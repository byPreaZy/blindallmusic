use leptos::prelude::{ArcRwSignal, GetUntracked, Set};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, MessageEvent, WebSocket};

pub type EventCallback = Rc<dyn Fn(&Value)>;
pub type PresenceCallback = Rc<dyn Fn(&str, &[Value])>;

#[derive(Clone)]
pub struct Realtime(Rc<Inner>);

struct Inner {
    ws: RefCell<Option<WebSocket>>,
    channels: RefCell<HashSet<String>>,
    listeners: RefCell<HashMap<(String, String), Vec<EventCallback>>>,
    presence: RefCell<HashMap<String, Vec<PresenceCallback>>>,
    reconnect_hooks: RefCell<Vec<Rc<dyn Fn()>>>,
    pub conn_state: ArcRwSignal<&'static str>,
    last_rx: Cell<f64>,
    generation: Cell<u64>,
    ever_connected: Cell<bool>,
    attempts: Cell<u32>,
    stopped: Cell<bool>,
    // Pending reconnect timer — dropping the handle cancels it, so
    // scheduling again replaces instead of stacking attempts.
    reconnect_timer: RefCell<Option<gloo_timers::callback::Timeout>>,
}

fn ws_url(ticket: &str) -> String {
    let loc = web_sys::window().unwrap().location();
    let host = loc.host().unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let proto = if loc.protocol().unwrap_or_default() == "https:" {
        "wss"
    } else {
        "ws"
    };
    // Ticket one-time (60s) obtenu via POST /ws-ticket : pas de bearer dans l'URL
    format!("{proto}://{host}/ws?ticket={ticket}")
}

impl Realtime {
    pub fn connect(token: &str) -> Self {
        let _ = token; // auth lue depuis stored_token() à chaque fetch de ticket
        let rt = Self(Rc::new(Inner {
            ws: RefCell::new(None),
            channels: RefCell::new(HashSet::new()),
            listeners: RefCell::new(HashMap::new()),
            presence: RefCell::new(HashMap::new()),
            reconnect_hooks: RefCell::new(Vec::new()),
            conn_state: ArcRwSignal::new("reconnecting"),
            last_rx: Cell::new(js_sys::Date::now()),
            generation: Cell::new(0),
            ever_connected: Cell::new(false),
            attempts: Cell::new(0),
            stopped: Cell::new(false),
            reconnect_timer: RefCell::new(None),
        }));
        rt.open();
        rt
    }

    fn open(&self) {
        if self.0.stopped.get() {
            return;
        }
        let inner = self.0.clone();
        let generation = inner.generation.get() + 1;
        inner.generation.set(generation);

        // Ticket one-time (60 s) via POST /ws-ticket : jamais de bearer dans l'URL.
        let rt = self.clone();
        leptos::task::spawn_local(async move {
            match crate::api::post_empty::<Value>("/api/v1/ws-ticket").await {
                Ok(v) => match v["ticket"].as_str() {
                    Some(t) if rt.0.generation.get() == generation => rt.wire(t, generation),
                    _ => {}
                },
                Err(e) => {
                    // 401/403 = logged out or token revoked: retrying is pointless.
                    if matches!(e.status, 401 | 403) {
                        rt.0.stopped.set(true);
                        rt.0.conn_state.set("offline");
                        return;
                    }
                    leptos::logging::warn!("ws ticket failed {:?}", e.message);
                    rt.schedule_reconnect();
                }
            }
        });
    }

    fn wire(&self, ticket: &str, generation: u64) {
        let inner = self.0.clone();
        let url = ws_url(ticket);
        let ws = match WebSocket::new(&url) {
            Ok(w) => w,
            Err(e) => {
                leptos::logging::error!("ws connect failed {:?}", e);
                self.schedule_reconnect();
                return;
            }
        };
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        *inner.ws.borrow_mut() = Some(ws.clone());

        {
            let inner = inner.clone();
            let ws2 = ws.clone();
            let onopen = Closure::<dyn FnMut()>::new(move || {
                inner.conn_state.set("connected");
                inner.attempts.set(0);
                inner.last_rx.set(js_sys::Date::now());
                let channels: Vec<String> = inner.channels.borrow().iter().cloned().collect();
                for ch in channels {
                    Self::send_raw(&ws2, &json!({"op": "subscribe", "channel": ch}));
                }
                if inner.ever_connected.replace(true) {
                    let hooks = inner.reconnect_hooks.borrow().clone();
                    for cb in hooks {
                        cb();
                    }
                }
            });
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            onopen.forget();
        }

        {
            let inner = inner.clone();
            let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                inner.last_rx.set(js_sys::Date::now());
                let Some(text) = e.data().as_string() else {
                    return;
                };
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    return;
                };
                match v["op"].as_str() {
                    Some("event") => {
                        let channel = v["channel"].as_str().unwrap_or_default();
                        let event = v["event"].as_str().unwrap_or_default();
                        let data = &v["data"];
                        for key in [
                            (channel.to_string(), event.to_string()),
                            (channel.to_string(), "*".to_string()),
                        ] {
                            // Snapshot the vec before dispatch: a callback may
                            // listen()/leave() (mutate this map) while handling.
                            let cbs = inner.listeners.borrow().get(&key).cloned();
                            if let Some(cbs) = cbs {
                                for cb in cbs {
                                    cb(data);
                                }
                            }
                        }
                    }
                    Some("presence") => {
                        let channel = v["channel"].as_str().unwrap_or_default();
                        let kind = v["kind"].as_str().unwrap_or_default();
                        let users: Vec<Value> = v["users"].as_array().cloned().unwrap_or_default();
                        let cbs = inner.presence.borrow().get(channel).cloned();
                        if let Some(cbs) = cbs {
                            for cb in cbs {
                                cb(kind, &users);
                            }
                        }
                    }
                    Some("error") => {
                        leptos::logging::warn!("ws error op {}", v["code"].as_str().unwrap_or("?"));
                    }
                    _ => {} // ack / pong
                }
            });
            ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget();
        }

        {
            let rt = self.clone();
            let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |_e: CloseEvent| {
                if rt.0.generation.get() != generation {
                    return; // a newer socket already replaced us
                }
                rt.0.conn_state.set("reconnecting");
                rt.schedule_reconnect();
            });
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            onclose.forget();
        }
    }

    fn schedule_reconnect(&self) {
        if self.0.stopped.get() {
            return;
        }
        // Exponential backoff with jitter: 1s, 2s, 4s… capped at 30s so a dead
        // server isn't hammered and a subway dropout doesn't drain the radio.
        let attempt = self.0.attempts.get();
        self.0.attempts.set(attempt.saturating_add(1));
        let base = 1_000u64.saturating_mul(1 << attempt.min(5));
        let jitter = (js_sys::Math::random() * 500.0) as u64;
        let delay = base.min(30_000) + jitter;
        let rt = self.clone();
        // Drop any previously scheduled attempt before installing the new
        // one — multiple close/error bursts must not stack timers.
        self.0.reconnect_timer.borrow_mut().take();
        self.0.reconnect_timer.replace(Some(
            gloo_timers::callback::Timeout::new(delay as u32, move || {
                if !rt.0.stopped.get() && rt.0.conn_state.get_untracked() != "connected" {
                    rt.open();
                }
            }),
        ));
    }

    fn send_raw(ws: &WebSocket, msg: &Value) {
        if ws.ready_state() == WebSocket::OPEN {
            let _ = ws.send_with_str(&msg.to_string());
        }
    }

    fn send(&self, msg: &Value) {
        if let Some(ws) = self.0.ws.borrow().as_ref() {
            Self::send_raw(ws, msg);
        }
    }

    pub fn join(&self, channel: &str) {
        self.0.channels.borrow_mut().insert(channel.to_string());
        self.send(&json!({"op": "subscribe", "channel": channel}));
    }

    pub fn subscribe(&self, channel: &str) {
        self.join(channel);
    }

    pub fn leave(&self, channel: &str) {
        self.0.channels.borrow_mut().remove(channel);
        self.0
            .listeners
            .borrow_mut()
            .retain(|(c, _), _| c != channel);
        self.0.presence.borrow_mut().remove(channel);
        self.send(&json!({"op": "unsubscribe", "channel": channel}));
    }

    pub fn listen(&self, channel: &str, event: &str, cb: EventCallback) {
        self.0
            .listeners
            .borrow_mut()
            .entry((channel.to_string(), event.to_string()))
            .or_default()
            .push(cb);
    }

    pub fn on_presence(&self, channel: &str, cb: PresenceCallback) {
        self.0
            .presence
            .borrow_mut()
            .entry(channel.to_string())
            .or_default()
            .push(cb);
    }

    pub fn on_reconnect(&self, cb: Rc<dyn Fn()>) {
        self.0.reconnect_hooks.borrow_mut().push(cb);
    }

    pub fn conn_state(&self) -> ArcRwSignal<&'static str> {
        self.0.conn_state.clone()
    }

    pub fn start_heartbeat(&self) -> gloo_timers::callback::Interval {
        let rt = self.clone();
        gloo_timers::callback::Interval::new(25_000, move || {
            // Dead sockets don't always fire `close` — in a tunnel the TCP
            // timeout can take minutes. No inbound traffic for ~2 missed
            // pongs → force-close, the close handler drives the reconnect.
            if rt.0.conn_state.get_untracked() == "connected"
                && js_sys::Date::now() - rt.0.last_rx.get() > 50_000.0
            {
                rt.0.conn_state.set("reconnecting");
                if let Some(ws) = rt.0.ws.borrow().as_ref() {
                    let _ = ws.close();
                }
            }
            rt.send(&json!({"op": "heartbeat", "ts": js_sys::Date::now() as i64}));
        })
    }
}
