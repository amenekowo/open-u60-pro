use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::error::{Result, ZteError};

const ANON_SESSION: &str = "00000000000000000000000000000000";

/// HTTP JSON-RPC 2.0 client for the ZTE ZWRT ubus API.
pub struct UbusClient {
    pub gateway: String,
    base_url: String,
    timeout: u64,
    pub session: String,
    id_counter: AtomicU64,
    client: Client,
    password: Option<String>,
}

impl UbusClient {
    pub fn new(gateway: Option<&str>, timeout: u64) -> Self {
        let gw = gateway
            .map(String::from)
            .unwrap_or_else(|| Self::detect_gateway());
        let base_url = format!("http://{}/ubus/", gw);
        Self {
            gateway: gw,
            base_url,
            timeout,
            session: ANON_SESSION.to_string(),
            id_counter: AtomicU64::new(0),
            client: Client::builder()
                .no_proxy()
                .connect_timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("Failed to build HTTP client"),
            password: None,
        }
    }

    /// Auto-detect gateway IP from the default route.
    fn detect_gateway() -> String {
        // Try Linux: ip route show default
        if let Ok(output) = Command::new("ip")
            .args(["route", "show", "default"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(idx) = parts.iter().position(|&p| p == "via") {
                    if let Some(gw) = parts.get(idx + 1) {
                        return gw.to_string();
                    }
                }
            }
        }
        // macOS fallback: route -n get default
        if let Ok(output) = Command::new("route")
            .args(["-n", "get", "default"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("gateway:") {
                    if let Some(gw) = line.split("gateway:").nth(1) {
                        let gw = gw.trim();
                        if !gw.is_empty() {
                            return gw.to_string();
                        }
                    }
                }
            }
        }
        "192.168.0.1".to_string()
    }

    fn next_id(&self) -> u64 {
        self.id_counter.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn post(&self, payload: Value) -> Result<Vec<Value>> {
        let url = format!("{}?t={}", self.base_url, Self::timestamp_ms());
        let body = if payload.is_array() {
            payload
        } else {
            json!([payload])
        };
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(self.timeout))
            .send()?;
        resp.error_for_status_ref()?;
        let data: Value = resp.json()?;
        match data {
            Value::Array(arr) => Ok(arr),
            other => Ok(vec![other]),
        }
    }

    fn rpc(&self, ubus_method: &str, params: Value) -> Result<Value> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": ubus_method,
            "params": params,
        });
        let results = self.post(payload)?;
        if results.is_empty() {
            return Err(ZteError::Ubus("Empty response from ubus".into()));
        }
        let result = &results[0];
        if result.get("error").is_some() {
            return Err(ZteError::Ubus(format!("ubus error: {}", result["error"])));
        }
        Ok(result
            .get("result")
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// Fetch the login salt via `zwrt_web / web_login_info`.
    pub fn get_salt(&self, retries: u32) -> Result<String> {
        let mut last_error = None;
        for attempt in 1..=retries {
            match self.rpc(
                "call",
                json!([ANON_SESSION, "zwrt_web", "web_login_info", {}]),
            ) {
                Ok(result) => {
                    if let Value::Array(arr) = &result {
                        if arr.len() >= 2 {
                            if let Value::Object(info) = &arr[1] {
                                let salt = info
                                    .get("zte_web_sault")
                                    .or_else(|| info.get("salt"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                if !salt.is_empty() {
                                    return Ok(salt.to_string());
                                }
                            }
                        }
                    }
                    last_error = Some(ZteError::Auth(format!(
                        "No salt in response (attempt {attempt}): {result}"
                    )));
                }
                Err(e) => last_error = Some(e),
            }
            if attempt < retries {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
        Err(last_error.unwrap_or_else(|| {
            ZteError::Auth(format!("Failed to fetch salt after {retries} attempts"))
        }))
    }

    /// Double-SHA256 hash: UPPER(SHA256(UPPER(SHA256(password)) + salt)).
    pub fn hash_password(password: &str, salt: &str) -> String {
        let first = format!("{:X}", Sha256::digest(password.as_bytes()));
        let combined = format!("{first}{salt}");
        format!("{:X}", Sha256::digest(combined.as_bytes()))
    }

    /// Authenticate and store the session token.
    pub fn login(&mut self, password: &str) -> Result<String> {
        self.password = Some(password.to_string());
        let salt = self.get_salt(3)?;
        let hashed = Self::hash_password(password, &salt);
        let result = self.rpc(
            "call",
            json!([ANON_SESSION, "zwrt_web", "web_login", { "password": hashed }]),
        )?;
        if let Value::Array(arr) = &result {
            if arr.len() >= 2 {
                if let Value::Object(info) = &arr[1] {
                    if let Some(session) = info
                        .get("ubus_rpc_session")
                        .and_then(|v| v.as_str())
                    {
                        if !session.is_empty() && session != ANON_SESSION {
                            self.session = session.to_string();
                            return Ok(session.to_string());
                        }
                    }
                }
            }
        }
        Err(ZteError::Auth(format!(
            "Login failed: unexpected response: {result}"
        )))
    }

    /// Re-authenticate using the stored password.
    pub fn relogin(&mut self) -> Result<String> {
        let password = self
            .password
            .clone()
            .ok_or_else(|| ZteError::Auth("No stored password for relogin".into()))?;
        self.login(&password)
    }

    /// Make an authenticated ubus call.
    pub fn call(&self, obj: &str, method: &str, params: Option<&Value>) -> Result<Value> {
        if self.session == ANON_SESSION {
            return Err(ZteError::Auth("Not logged in. Call login() first.".into()));
        }
        let p = params.cloned().unwrap_or(json!({}));
        let result = self.rpc("call", json!([self.session, obj, method, p]))?;
        if let Value::Array(arr) = &result {
            if !arr.is_empty() {
                if let Some(code) = arr[0].as_u64() {
                    if code != 0 {
                        return Err(ZteError::Ubus(format!(
                            "ubus call failed (code {code}): {obj}.{method}"
                        )));
                    }
                }
            }
            if arr.len() >= 2 {
                return Ok(arr[1].clone());
            }
            return Ok(Value::Null);
        }
        Ok(result)
    }

    /// Make an unauthenticated ubus call (anonymous session).
    pub fn call_anon(&self, obj: &str, method: &str, params: Option<&Value>) -> Result<Value> {
        let p = params.cloned().unwrap_or(json!({}));
        let result = self.rpc("call", json!([ANON_SESSION, obj, method, p]))?;
        if let Value::Array(arr) = &result {
            if arr.len() >= 2 {
                return Ok(arr[1].clone());
            }
            return Ok(Value::Null);
        }
        Ok(result)
    }

    /// Check if session is authenticated.
    pub fn is_authenticated(&self) -> bool {
        self.session != ANON_SESSION
    }

    /// Return the current session ID.
    pub fn session(&self) -> &str {
        &self.session
    }

    /// List ubus objects/methods (authenticated).
    pub fn list(&self, pattern: Option<&str>) -> Result<Value> {
        let pat = pattern.unwrap_or("*");
        let result = self.rpc("list", json!([self.session, pat]))?;
        match &result {
            Value::Object(_) => Ok(result),
            Value::Array(arr) if !arr.is_empty() && arr[0].is_object() => Ok(arr[0].clone()),
            _ => Ok(json!({})),
        }
    }

    /// List ubus objects/methods (anonymous session).
    pub fn list_anon(&self, pattern: Option<&str>) -> Result<Value> {
        let pat = pattern.unwrap_or("*");
        let result = self.rpc("list", json!([ANON_SESSION, pat]))?;
        match &result {
            Value::Object(_) => Ok(result),
            Value::Array(arr) if !arr.is_empty() && arr[0].is_object() => Ok(arr[0].clone()),
            _ => Ok(json!({})),
        }
    }
}
