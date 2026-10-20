#![allow(dead_code)]

use reqwest::{Client as Http, Method, StatusCode};
use serde_json::{Value, json};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn base() -> String {
    std::env::var("TEST_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

static RUN: OnceLock<String> = OnceLock::new();
fn run() -> &'static str {
    RUN.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let hex = format!("{nanos:x}");
        hex[hex.len() - 8..].to_string()
    })
}

static COUNTER: AtomicU64 = AtomicU64::new(0);
fn next() -> u64 {
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

pub fn uniq_name(prefix: &str) -> String {
    format!("{}{}{}", prefix, next(), run())
}
pub fn uniq_email(prefix: &str) -> String {
    format!("{}{}{}@e2e.test", prefix, next(), run())
}

pub struct Client {
    http: Http,
    pub token: Option<String>,
    pub user_id: i64,
    pub name: String,
    ip: String,
}

pub struct Resp {
    pub status: StatusCode,
    pub body: Value,
    pub text: String,
}

impl Resp {
    pub fn json(&self, ptr: &str) -> Value {
        self.body.pointer(ptr).cloned().unwrap_or(Value::Null)
    }
    pub fn ok(&self) -> bool {
        self.status.is_success()
    }
    pub fn expect_ok(self, ctx: &str) -> Self {
        assert!(
            self.ok(),
            "{ctx}: expected 2xx, got {}, {}",
            self.status,
            self.text
        );
        self
    }
}

impl Client {
    pub fn anon() -> Self {
        let n = next();
        // ~16M unique IPs per run: bans are keyed on IP and the old
        // single-octet scheme wrapped onto banned addresses past 254 clients.
        let seed = u64::from_str_radix(&run()[0..4], 16).unwrap_or(0);
        let k = (seed + n) % (254 * 254 * 254);
        Self {
            http: Http::new(),
            token: None,
            user_id: 0,
            name: String::new(),
            ip: format!(
                "10.{}.{}.{}",
                k / 64516 + 1,
                (k / 254) % 254 + 1,
                k % 254 + 1
            ),
        }
    }

    pub fn with_token(token: &str) -> Self {
        let mut c = Self::anon();
        c.token = Some(token.to_string());
        c
    }

    async fn req(&self, method: Method, path: &str, body: Option<Value>) -> Resp {
        let url = format!("{}{}", base(), path);
        let mut r = self
            .http
            .request(method, &url)
            .header("X-Forwarded-For", &self.ip)
            .header("Accept", "application/json");
        if let Some(t) = &self.token {
            r = r.bearer_auth(t);
        }
        if let Some(b) = body {
            r = r.json(&b);
        }
        let res = r.send().await.expect(&format!("request {url}"));
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        let body = serde_json::from_str(&text).unwrap_or(Value::Null);
        Resp { status, body, text }
    }

    pub async fn get(&self, path: &str) -> Resp {
        self.req(Method::GET, path, None).await
    }
    pub async fn post(&self, path: &str, body: Value) -> Resp {
        self.req(Method::POST, path, Some(body)).await
    }
    pub async fn put(&self, path: &str, body: Value) -> Resp {
        self.req(Method::PUT, path, Some(body)).await
    }
    pub async fn delete(&self, path: &str) -> Resp {
        self.req(Method::DELETE, path, None).await
    }
    pub async fn delete_body(&self, path: &str, body: Value) -> Resp {
        self.req(Method::DELETE, path, Some(body)).await
    }
    pub async fn post_empty(&self, path: &str) -> Resp {
        self.req(Method::POST, path, None).await
    }

    /// Raw-body POST with an extra header — used by the signed Stripe webhook.
    pub async fn post_raw(&self, path: &str, header: (&str, &str), body: String) -> Resp {
        let url = format!("{}{}", base(), path);
        let res = self
            .http
            .post(&url)
            .header("X-Forwarded-For", &self.ip)
            .header("Content-Type", "application/json")
            .header(header.0, header.1)
            .body(body)
            .send()
            .await
            .expect(&format!("request {url}"));
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        let body = serde_json::from_str(&text).unwrap_or(Value::Null);
        Resp { status, body, text }
    }

    pub async fn post_file(&self, path: &str, field: &str, filename: &str, bytes: &[u8]) -> Resp {
        let url = format!("{}{}", base(), path);
        let part = reqwest::multipart::Part::bytes(bytes.to_vec())
            .file_name(filename.to_string())
            .mime_str("image/png")
            .unwrap();
        let form = reqwest::multipart::Form::new().part(field.to_string(), part);
        let mut r = self
            .http
            .post(&url)
            .header("X-Forwarded-For", &self.ip)
            .header("Accept", "application/json")
            .multipart(form);
        if let Some(t) = &self.token {
            r = r.bearer_auth(t);
        }
        let res = r.send().await.expect(&format!("multipart {url}"));
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        let body = serde_json::from_str(&text).unwrap_or(Value::Null);
        Resp { status, body, text }
    }

    pub async fn register(prefix: &str) -> (Self, Value) {
        let c = Self::anon();
        let name = uniq_name(prefix);
        let resp = c
            .post(
                "/api/v1/auth/register",
                json!({
                    "name": name,
                    "email": uniq_email(prefix),
                    "password": "password123",
                    "password_confirmation": "password123",
                }),
            )
            .await;
        assert_eq!(resp.status, StatusCode::CREATED, "register: {}", resp.text);
        let mut c = c;
        c.token = Some(resp.json("/token").as_str().unwrap().to_string());
        c.user_id = resp.json("/user/id").as_i64().unwrap();
        c.name = name;
        (c, resp.body)
    }

    pub async fn guest() -> Self {
        let c = Self::anon();
        let resp = c.post("/api/v1/auth/guest", json!({})).await;
        assert_eq!(resp.status, StatusCode::CREATED, "guest: {}", resp.text);
        let mut c = c;
        c.token = Some(resp.json("/token").as_str().unwrap().to_string());
        c.user_id = resp.json("/user/id").as_i64().unwrap();
        c.name = resp.json("/user/name").as_str().unwrap().to_string();
        c
    }

    pub async fn login(&self, email: &str, password: &str) -> Resp {
        self.post(
            "/api/v1/auth/login",
            json!({"email": email, "password": password}),
        )
        .await
    }

    pub async fn admin() -> Self {
        let c = Self::anon();
        let r = c.login("e2e-admin@blindall.test", "password123").await;
        assert_eq!(r.status, StatusCode::OK, "admin login: {}", r.text);
        let mut c = c;
        c.token = Some(r.json("/token").as_str().unwrap().to_string());
        c.user_id = r.json("/user/id").as_i64().unwrap();
        c.name = "E2EAdmin".to_string();
        assert!(r.json("/user/is_administrator").as_bool().unwrap());
        c
    }
}

pub async fn game_fixture(owner: &Client, extra_answers: &[(&str, i64, f64)]) -> (i64, i64, i64) {
    let pl = owner
        .post("/api/v1/playlists", json!({"name": uniq_name("pl")}))
        .await;
    assert_eq!(pl.status, StatusCode::CREATED, "playlist: {}", pl.text);
    let playlist_id = pl.json("/id").as_i64().unwrap();

    let probe_url = format!("{}/healthz", base());
    let t = owner
        .post(
            &format!("/api/v1/playlists/{playlist_id}/tracks"),
            json!({
                "provider": "test",
                "provider_id": format!("e2e-{}", next()),
                "provider_url": "https://example.com/track/0",
                "artist_name": "E2E Artist",
                "track_name": "E2E Song",
                "preview_url": probe_url,
                "artwork_url": "https://example.com/cover.jpg",
            }),
        )
        .await;
    assert_eq!(t.status, StatusCode::CREATED, "track: {}", t.text);
    let track_id = t.json("/id").as_i64().unwrap();

    for (value, type_id, score) in extra_answers {
        let a = owner
            .post(
                &format!("/api/v1/tracks/{track_id}/answers"),
                json!({"answer_type_id": type_id, "value": value, "score": score}),
            )
            .await;
        assert!(
            a.status.is_success(),
            "answer {value}: {} {}",
            a.status,
            a.text
        );
    }

    let room_name = uniq_name("room");
    let r = owner
        .post(
            "/api/v1/rooms",
            json!({"name": room_name, "category_id": 4}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "room: {}", r.text);
    let room_id = r.json("/room/id").as_i64().unwrap();
    let attach = owner
        .post(
            &format!("/api/v1/rooms/{room_id}/playlists/attach"),
            json!({"playlist_id": playlist_id}),
        )
        .await;
    assert!(attach.ok(), "attach: {}", attach.text);
    visit_room(owner, room_id).await;
    (room_id, playlist_id, track_id)
}

pub async fn visit_room(c: &Client, room_id: i64) {
    c.get(&format!("/api/v1/rooms/{room_id}"))
        .await
        .expect_ok("visit room");
}
