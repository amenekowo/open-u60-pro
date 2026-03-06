use serde_json::{json, Value};

use crate::at_cmd::AtPort;
use crate::auth::AuthState;
use crate::system::{self, CpuTracker, SpeedTracker};
use crate::ubus;

pub struct AppState {
    pub auth: AuthState,
    pub cpu: CpuTracker,
    pub speed: SpeedTracker,
    pub at_port: AtPort,
    pub doh: std::sync::Arc<crate::doh::DohProxy>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            auth: AuthState::new(),
            cpu: CpuTracker::new(),
            speed: SpeedTracker::new(),
            at_port: AtPort::new(),
            doh: std::sync::Arc::new(crate::doh::DohProxy::new()),
        }
    }
}

/// POST /api/auth/login — body: {"password": "..."}
pub fn login(state: &AppState, body: &[u8]) -> (u16, Value) {
    let parsed: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return (400, json!({"ok": false, "error": "invalid JSON"})),
    };
    let password = match parsed["password"].as_str() {
        Some(p) => p,
        None => return (400, json!({"ok": false, "error": "missing 'password' field"})),
    };
    match state.auth.login(password) {
        Some(token) => (200, json!({"ok": true, "data": {"token": token}})),
        None => (401, json!({"ok": false, "error": "invalid password"})),
    }
}

/// GET /api/device
pub fn device(_state: &AppState) -> (u16, Value) {
    let info = system::read_device_info();
    (
        200,
        json!({"ok": true, "data": {
            "hostname": info.hostname,
            "uptime_secs": info.uptime_secs,
            "load_avg": info.load_avg,
            "kernel": info.kernel,
        }}),
    )
}

/// GET /api/battery
pub fn battery(_state: &AppState) -> (u16, Value) {
    match system::read_battery() {
        Some(b) => (200, json!({"ok": true, "data": b})),
        None => (
            503,
            json!({"ok": false, "error": "battery info not available"}),
        ),
    }
}

/// GET /api/cpu
pub fn cpu(state: &AppState) -> (u16, Value) {
    let usage = state.cpu.sample();
    (200, json!({"ok": true, "data": usage}))
}

/// GET /api/memory
pub fn memory(_state: &AppState) -> (u16, Value) {
    match system::read_meminfo() {
        Some(m) => (200, json!({"ok": true, "data": m})),
        None => (
            503,
            json!({"ok": false, "error": "memory info not available"}),
        ),
    }
}

/// GET /api/network/signal
pub fn network_signal(_state: &AppState) -> (u16, Value) {
    match ubus::call("zte_nwinfo_api", "nwinfo_get_netinfo", Some("{}")) {
        Ok(data) => (200, json!({"ok": true, "data": data})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

/// GET /api/network/traffic
pub fn network_traffic(_state: &AppState) -> (u16, Value) {
    let ifaces = system::read_network_traffic();
    (200, json!({"ok": true, "data": ifaces}))
}

/// GET /api/network/speed — server-computed speed with precise timing
pub fn network_speed(state: &AppState) -> (u16, Value) {
    let snap = state.speed.sample();
    (200, json!({"ok": true, "data": snap}))
}

/// GET /api/modem/status
pub fn modem_status(_state: &AppState) -> (u16, Value) {
    match ubus::uci_get("zte_nwinfo.sys_info.operate_mode") {
        Ok(mode) => (200, json!({"ok": true, "data": {"operate_mode": mode}})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}

/// POST /api/modem/online
pub fn modem_online(state: &AppState) -> (u16, Value) {
    use crate::at_cmd;
    match at_cmd::send(&state.at_port, "AT+CFUN=1", 8) {
        Ok(resp) if resp.contains("OK") => (200, json!({"ok": true, "data": {"status": "ok"}})),
        Ok(resp) => (500, json!({"ok": false, "error": "AT+CFUN=1 failed", "raw": resp})),
        Err(e) => (503, json!({"ok": false, "error": e})),
    }
}
