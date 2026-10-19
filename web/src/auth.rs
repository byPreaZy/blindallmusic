use crate::api;
use crate::types::{AuthResponse, User};
use leptos::prelude::*;
use serde::Serialize;

#[derive(Clone, Copy)]
pub struct Auth {
    pub user: RwSignal<Option<User>>,
    pub ready: RwSignal<bool>,
}

impl Auth {
    pub fn provide() -> Self {
        let auth = Self {
            user: RwSignal::new(None),
            ready: RwSignal::new(false),
        };
        if api::stored_token().is_some() {
            leptos::task::spawn_local(async move {
                match api::get::<User>("/api/v1/auth/me").await {
                    Ok(u) => auth.user.set(Some(u)),
                    Err(e) => {
                        if e.status == 401 {
                            api::store_token(None);
                        } else if e.body["error"].as_str() == Some("banned") {
                            if let Some(w) = web_sys::window() {
                                if w.location().pathname().ok().as_deref() != Some("/user/banned") {
                                    let _ = w.location().set_href("/user/banned");
                                }
                            }
                        }
                        auth.user.set(None);
                    }
                }
                auth.ready.set(true);
            });
        } else {
            auth.ready.set(true);
        }
        provide_context(auth);
        auth
    }

    pub fn is_authed(&self) -> bool {
        self.user.get().is_some()
    }

    pub async fn login(
        &self,
        email: &str,
        password: &str,
        remember: bool,
    ) -> Result<(), api::ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            email: &'a str,
            password: &'a str,
            remember: bool,
        }
        let resp: AuthResponse = api::post(
            "/api/v1/auth/login",
            &Body {
                email,
                password,
                remember,
            },
        )
        .await?;
        api::store_token(Some(&resp.token));
        self.user.set(Some(resp.user));
        Ok(())
    }

    pub async fn register(
        &self,
        name: &str,
        email: &str,
        password: &str,
        password_confirmation: &str,
    ) -> Result<(), api::ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            email: &'a str,
            password: &'a str,
            password_confirmation: &'a str,
        }
        let resp: AuthResponse = api::post(
            "/api/v1/auth/register",
            &Body {
                name,
                email,
                password,
                password_confirmation,
            },
        )
        .await?;
        api::store_token(Some(&resp.token));
        self.user.set(Some(resp.user));
        Ok(())
    }

    pub fn complete_oauth(&self, resp: crate::types::AuthResponse) {
        api::store_token(Some(&resp.token));
        self.user.set(Some(resp.user));
    }

    pub fn logout(&self) {
        let user_sig = self.user;
        leptos::task::spawn_local(async move {
            let _ = api::post_empty::<serde_json::Value>("/api/v1/auth/logout").await;
            api::store_token(None);
            user_sig.set(None);
        });
    }

    pub fn guest_to(&self, target: &'static str) {
        let user_sig = self.user;
        leptos::task::spawn_local(async move {
            let _ = api::post_empty::<serde_json::Value>("/api/v1/auth/logout").await;
            api::store_token(None);
            user_sig.set(None);
            if let Some(win) = web_sys::window() {
                let _ = win.location().set_href(target);
            }
        });
    }
}

pub fn use_auth() -> Auth {
    expect_context::<Auth>()
}

pub fn require_registered(auth: Auth) {
    Effect::new(move |_| {
        if !auth.ready.get() {
            return;
        }
        let nav = leptos_router::hooks::use_navigate();
        match auth.user.get() {
            None => nav("/login", Default::default()),
            Some(u) if u.is_guest => nav("/", Default::default()),
            _ => {}
        }
    });
}
