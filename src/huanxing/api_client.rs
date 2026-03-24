//! HTTP client for the HuanXing backend API.
//!
//! Three authentication modes:
//! 1. **Agent Key** — `X-Agent-Key` header for `agent/` routes
//! 2. **User Token** — `Authorization: Bearer {token}` for `app/` routes
//! 3. **Public** — no auth for `open/` and `auth/` routes

use anyhow::{Context, Result};
use serde_json::Value;

/// HuanXing backend API client.
#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    agent_key: String,
    server_id: String,
}

impl ApiClient {
    /// Create a new API client.
    pub fn new(base_url: &str, agent_key: &str, server_id: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            agent_key: agent_key.to_string(),
            server_id: server_id.to_string(),
        }
    }

    // ── Agent Key authenticated requests ──────────────

    /// POST with Agent Key auth.
    pub async fn agent_post(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .headers(self.agent_headers())
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {path}"))?;
        self.handle_response(resp, "POST", path).await
    }

    /// GET with Agent Key auth.
    pub async fn agent_get(&self, path: &str, params: &[(&str, &str)]) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .headers(self.agent_headers())
            .query(params)
            .send()
            .await
            .with_context(|| format!("GET {path}"))?;
        self.handle_response(resp, "GET", path).await
    }

    /// POST multipart form with Agent Key auth.
    pub async fn agent_post_multipart(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);

        let mut h = reqwest::header::HeaderMap::new();
        if let Ok(v) = reqwest::header::HeaderValue::from_str(&self.agent_key) {
            h.insert("X-Agent-Key", v);
        }
        if let Ok(v) = reqwest::header::HeaderValue::from_str(&self.server_id) {
            h.insert("X-Server-Id", v);
        }
        h.insert("X-App-Code", reqwest::header::HeaderValue::from_static("huanxing"));

        let resp = self
            .client
            .post(&url)
            .headers(h)
            .query(query)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("POST multipart {path}"))?;
        self.handle_response(resp, "POST", path).await
    }

    /// PUT with Agent Key auth.
    pub async fn agent_put(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .put(&url)
            .headers(self.agent_headers())
            .json(body)
            .send()
            .await
            .with_context(|| format!("PUT {path}"))?;
        self.handle_response(resp, "PUT", path).await
    }

    /// DELETE with Agent Key auth.
    pub async fn agent_delete(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .delete(&url)
            .headers(self.agent_headers())
            .send()
            .await
            .with_context(|| format!("DELETE {path}"))?;
        self.handle_response(resp, "DELETE", path).await
    }

    // ── Agent Key + X-User-Id requests ─────────────────

    /// Headers with both Agent Key and X-User-Id.
    fn agent_user_headers(&self, user_uuid: &str) -> reqwest::header::HeaderMap {
        let mut h = self.agent_headers();
        if let Ok(v) = reqwest::header::HeaderValue::from_str(user_uuid) {
            h.insert("X-User-Id", v);
        }
        h
    }

    /// POST with Agent Key + X-User-Id auth (for per-user resource APIs).
    pub async fn agent_post_as_user(
        &self,
        path: &str,
        body: &Value,
        user_uuid: &str,
    ) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .headers(self.agent_user_headers(user_uuid))
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {path}"))?;
        self.handle_response(resp, "POST", path).await
    }

    /// GET with Agent Key + X-User-Id auth.
    pub async fn agent_get_as_user(
        &self,
        path: &str,
        params: &[(&str, &str)],
        user_uuid: &str,
    ) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .headers(self.agent_user_headers(user_uuid))
            .query(params)
            .send()
            .await
            .with_context(|| format!("GET {path}"))?;
        self.handle_response(resp, "GET", path).await
    }

    /// PUT with Agent Key + X-User-Id auth.
    pub async fn agent_put_as_user(
        &self,
        path: &str,
        body: &Value,
        user_uuid: &str,
    ) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .put(&url)
            .headers(self.agent_user_headers(user_uuid))
            .json(body)
            .send()
            .await
            .with_context(|| format!("PUT {path}"))?;
        self.handle_response(resp, "PUT", path).await
    }

    /// DELETE with Agent Key + X-User-Id auth.
    pub async fn agent_delete_as_user(&self, path: &str, user_uuid: &str) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .delete(&url)
            .headers(self.agent_user_headers(user_uuid))
            .send()
            .await
            .with_context(|| format!("DELETE {path}"))?;
        self.handle_response(resp, "DELETE", path).await
    }

    // ── Public requests (no auth) ─────────────────────

    /// POST to public endpoint (no auth).
    pub async fn open_post(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-App-Code", "huanxing")
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {path}"))?;
        self.handle_response(resp, "POST", path).await
    }

    // ── User Token authenticated requests ─────────────

    /// GET with user Bearer token.
    pub async fn user_get(
        &self,
        path: &str,
        token: &str,
        params: &[(&str, &str)],
    ) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .headers(self.user_headers(token))
            .query(params)
            .send()
            .await
            .with_context(|| format!("GET {path}"))?;
        self.handle_response(resp, "GET", path).await
    }

    /// POST with user Bearer token.
    pub async fn user_post(&self, path: &str, token: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .headers(self.user_headers(token))
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {path}"))?;
        self.handle_response(resp, "POST", path).await
    }

    /// PUT with user Bearer token.
    pub async fn user_put(&self, path: &str, token: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .put(&url)
            .headers(self.user_headers(token))
            .json(body)
            .send()
            .await
            .with_context(|| format!("PUT {path}"))?;
        self.handle_response(resp, "PUT", path).await
    }

    /// DELETE with user Bearer token.
    pub async fn user_delete(&self, path: &str, token: &str) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .delete(&url)
            .headers(self.user_headers(token))
            .send()
            .await
            .with_context(|| format!("DELETE {path}"))?;
        self.handle_response(resp, "DELETE", path).await
    }

    // ── Internal helpers ──────────────────────────────

    fn agent_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::HeaderValue;
        let mut h = reqwest::header::HeaderMap::new();
        h.insert("Content-Type", HeaderValue::from_static("application/json"));
        if let Ok(v) = HeaderValue::from_str(&self.agent_key) {
            h.insert("X-Agent-Key", v);
        }
        if let Ok(v) = HeaderValue::from_str(&self.server_id) {
            h.insert("X-Server-Id", v);
        }
        h.insert("X-App-Code", HeaderValue::from_static("huanxing"));
        h
    }

    fn user_headers(&self, token: &str) -> reqwest::header::HeaderMap {
        use reqwest::header::HeaderValue;
        let mut h = reqwest::header::HeaderMap::new();
        h.insert("Content-Type", HeaderValue::from_static("application/json"));
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {token}")) {
            h.insert("Authorization", v);
        }
        h.insert("X-App-Code", HeaderValue::from_static("huanxing"));
        h
    }

    async fn handle_response(
        &self,
        resp: reqwest::Response,
        method: &str,
        path: &str,
    ) -> Result<Value> {
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "API 请求失败: {method} {path} → {status}{body_suffix}",
                body_suffix = if body.is_empty() {
                    String::new()
                } else {
                    format!(" | {body}")
                }
            );
        }
        resp.json::<Value>()
            .await
            .with_context(|| format!("Failed to parse JSON from {method} {path}"))
    }
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient")
            .field("base_url", &self.base_url)
            .field("server_id", &self.server_id)
            .field("agent_key", &"[redacted]")
            .finish()
    }
}
