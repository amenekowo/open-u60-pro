mod at_cmd;
mod auth;
mod cell;
mod device_ext;
pub mod doh;
mod handlers;
mod modem_ext;
mod network_ext;
mod router;
mod server;
mod sim;
mod sms;
mod system;
mod telephony;
mod ubus;
mod usb;
mod wifi;

use std::sync::Arc;

use handlers::AppState;

const DEFAULT_BIND: &str = "0.0.0.0:9090";
const DEFAULT_THREADS: usize = 2;

fn main() {
    let bind = std::env::var("ZTE_AGENT_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let threads: usize = std::env::var("ZTE_AGENT_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_THREADS);

    let state = Arc::new(AppState::new());

    // Set password from environment if provided
    if let Ok(pw) = std::env::var("ZTE_AGENT_PASSWORD") {
        state.auth.set_password(&pw);
        eprintln!("Auth enabled (password from ZTE_AGENT_PASSWORD)");
    } else {
        eprintln!("Warning: No ZTE_AGENT_PASSWORD set — auth disabled");
    }

    state.doh.auto_start();

    server::start(&bind, threads, state);
}
