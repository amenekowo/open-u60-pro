use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, TcpStream};
use std::os::unix::io::{FromRawFd as _, OwnedFd as _};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::error::{Result, ZteError};

const ANON_SESSION: &str = "00000000000000000000000000000000";

/// HTTP JSON-RPC 2.0 client for the ZTE ZWRT ubus API.
pub struct UbusClient {
    pub gateway: String,
    base_url: String,
    timeout: Duration,
    pub session: String,
    id_counter: AtomicU64,
    /// Interface index for IP_BOUND_IF on macOS (0 = no binding).
    iface_idx: u32,
    password: Option<String>,
}

impl UbusClient {
    pub fn new(gateway: Option<&str>, timeout: u64) -> Self {
        let gw = gateway
            .map(String::from)
            .unwrap_or_else(|| Self::detect_gateway());
        let base_url = format!("http://{}/ubus", gw);
        // On macOS with scoped routes, we need to bind sockets to the correct
        // interface via IP_BOUND_IF. Find the interface index for the gateway.
        let iface_idx = if let Ok(gw_ip) = gw.parse::<Ipv4Addr>() {
            Self::find_iface_for(gw_ip)
                .map(|(name, _ip)| Self::if_nametoindex(&name))
                .unwrap_or(0)
        } else {
            0
        };
        Self {
            gateway: gw,
            base_url,
            timeout: Duration::from_secs(timeout),
            session: ANON_SESSION.to_string(),
            id_counter: AtomicU64::new(0),
            iface_idx,
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

    /// Find the network interface and local IPv4 address on the same /24 as the gateway.
    fn find_iface_for(gateway: Ipv4Addr) -> Option<(String, Ipv4Addr)> {
        let gw = gateway.octets();
        // Try `ifconfig` (macOS / BSD) — interface name is the non-indented line
        if let Ok(output) = Command::new("ifconfig").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_iface = String::new();
            for line in stdout.lines() {
                if !line.starts_with('\t') && !line.starts_with(' ') {
                    // "en11: flags=8863<...> mtu 1500"
                    if let Some(name) = line.split(':').next() {
                        current_iface = name.to_string();
                    }
                } else if let Some(rest) = line.trim().strip_prefix("inet ") {
                    if let Some(ip_str) = rest.split_whitespace().next() {
                        if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                            let o = ip.octets();
                            if o[0] == gw[0] && o[1] == gw[1] && o[2] == gw[2] && ip != gateway {
                                return Some((current_iface, ip));
                            }
                        }
                    }
                }
            }
        }
        // Try `ip -4 addr` (Linux)
        if let Ok(output) = Command::new("ip").args(["-4", "addr"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_iface = String::new();
            for line in stdout.lines() {
                let trimmed = line.trim();
                // "2: en11: <...>"
                if !line.starts_with(' ') {
                    if let Some(rest) = line.split(':').nth(1) {
                        current_iface = rest.trim().to_string();
                    }
                } else if let Some(rest) = trimmed.strip_prefix("inet ") {
                    if let Some(cidr) = rest.split_whitespace().next() {
                        let ip_str = cidr.split('/').next().unwrap_or("");
                        if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                            let o = ip.octets();
                            if o[0] == gw[0] && o[1] == gw[1] && o[2] == gw[2] && ip != gateway {
                                return Some((current_iface, ip));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Convert interface name to index (for IP_BOUND_IF).
    fn if_nametoindex(name: &str) -> u32 {
        let c_name = std::ffi::CString::new(name).unwrap_or_default();
        // SAFETY: if_nametoindex is a safe POSIX function.
        let idx = unsafe { libc::if_nametoindex(c_name.as_ptr()) };
        idx
    }

    /// Create a TCP connection to the gateway, binding to the correct
    /// interface on macOS via IP_BOUND_IF if needed.
    fn tcp_connect(&self) -> std::io::Result<TcpStream> {
        let gw_ip: Ipv4Addr = self.gateway.parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, libc::IPPROTO_TCP);
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Bind to interface on macOS
            #[cfg(target_os = "macos")]
            if self.iface_idx > 0 {
                let val = self.iface_idx as libc::c_int;
                let ret = libc::setsockopt(
                    fd,
                    libc::IPPROTO_IP,
                    25, // IP_BOUND_IF
                    &val as *const _ as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
                if ret != 0 {
                    let err = std::io::Error::last_os_error();
                    libc::close(fd);
                    return Err(err);
                }
            }
            // Set nodelay
            let one: libc::c_int = 1;
            libc::setsockopt(
                fd, libc::IPPROTO_TCP, libc::TCP_NODELAY,
                &one as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            );
            // Connect
            let octets = gw_ip.octets();
            let mut addr: libc::sockaddr_in = std::mem::zeroed();
            addr.sin_family = libc::AF_INET as u8;
            addr.sin_port = 80u16.to_be();
            addr.sin_addr.s_addr = u32::from_ne_bytes(octets);
            let ret = libc::connect(
                fd,
                &addr as *const libc::sockaddr_in as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
            );
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(err);
            }
            Ok(TcpStream::from(std::os::unix::io::OwnedFd::from_raw_fd(fd)))
        }
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
        let path = format!("/ubus?t={}", Self::timestamp_ms());
        let body = if payload.is_array() {
            payload
        } else {
            json!([payload])
        };
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| ZteError::Ubus(format!("JSON serialize error: {e}")))?;

        let mut stream = self.tcp_connect()
            .map_err(|e| ZteError::Http(format!("TCP connect to {}: {e}", self.gateway)))?;

        // Send HTTP request
        let req = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            path, self.gateway, body_bytes.len()
        );
        stream.write_all(req.as_bytes())
            .map_err(|e| ZteError::Http(format!("write: {e}")))?;
        stream.write_all(&body_bytes)
            .map_err(|e| ZteError::Http(format!("write body: {e}")))?;

        // Read response
        let mut response = Vec::new();
        stream.read_to_end(&mut response)
            .map_err(|e| ZteError::Http(format!("read: {e}")))?;

        // Parse HTTP response — find body after \r\n\r\n
        let resp_str = String::from_utf8_lossy(&response);
        let body_start = resp_str.find("\r\n\r\n")
            .ok_or_else(|| ZteError::Http("malformed HTTP response".into()))?;
        let body_str = &resp_str[body_start + 4..];

        let data: Value = serde_json::from_str(body_str)
            .map_err(|e| ZteError::Http(format!("JSON parse error: {e}")))?;
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
