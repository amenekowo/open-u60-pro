use std::fs;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::ubus;

const STORAGE_PATH: &str = "/data/local/tmp/charge_limit.json";
const POLL_ACTIVE_SECS: u64 = 60;
const POLL_IDLE_SECS: u64 = 300;
const DEFAULT_HYSTERESIS: u8 = 5;

const SYSFS_CAPACITY: &str = "/sys/class/power_supply/battery/capacity";
const SYSFS_STATUS: &str = "/sys/class/power_supply/battery/status";

fn default_hysteresis() -> u8 {
    DEFAULT_HYSTERESIS
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    enabled: bool,
    limit: u8,
    #[serde(default = "default_hysteresis")]
    hysteresis: u8,
}

struct LimitState {
    enabled: bool,
    limit: u8,
    hysteresis: u8,
    manual_override: bool,
}

pub struct ChargeLimitEnforcer {
    inner: Mutex<LimitState>,
}

/// Check if charging is currently stopped via ubus (ground truth).
/// `direct_power_supply_mode: "enable"` = charging STOPPED (inverted naming).
fn is_charging_stopped() -> bool {
    ubus::call("zwrt_bsp.charger", "list", Some("{}"))
        .ok()
        .and_then(|v| {
            v["direct_power_supply_mode"]
                .as_str()
                .map(|s| s == "enable")
        })
        .unwrap_or(false)
}

/// Set charging state via ubus (inverted: "enable" = stop, "disable" = start)
fn set_charging(allow: bool) {
    let mode = if allow { "disable" } else { "enable" };
    let params = format!(r#"{{"direct_power_supply_mode":"{mode}"}}"#);
    match ubus::call("zwrt_bsp.charger", "set", Some(&params)) {
        Ok(_) => {
            // Verify the change took effect
            let stopped = is_charging_stopped();
            let expected_stopped = !allow;
            if stopped != expected_stopped {
                eprintln!(
                    "charge_limit: WARNING set_charging({}) succeeded but state mismatch: stopped={}",
                    allow, stopped
                );
            }
        }
        Err(e) => eprintln!("charge_limit: ERROR set_charging({}) failed: {}", allow, e),
    }
}

impl ChargeLimitEnforcer {
    pub fn new() -> Self {
        let persisted = fs::read_to_string(STORAGE_PATH)
            .ok()
            .and_then(|s| serde_json::from_str::<Persisted>(&s).ok());

        let (enabled, limit, hysteresis) = match persisted {
            Some(p) => (p.enabled, p.limit.clamp(50, 100), p.hysteresis.clamp(1, 20)),
            None => (false, 100, DEFAULT_HYSTERESIS),
        };

        ChargeLimitEnforcer {
            inner: Mutex::new(LimitState {
                enabled,
                limit,
                hysteresis,
                manual_override: false,
            }),
        }
    }

    pub fn start(self: &Arc<Self>) {
        let enforcer = Arc::clone(self);
        std::thread::spawn(move || {
            eprintln!(
                "Charge limit enforcer started ({}s/{}s active/idle)",
                POLL_ACTIVE_SECS, POLL_IDLE_SECS
            );
            loop {
                let interval = enforcer.tick();
                std::thread::sleep(std::time::Duration::from_secs(interval));
            }
        });
    }

    fn tick(&self) -> u64 {
        let state = self.inner.lock().unwrap();
        if !state.enabled || state.manual_override {
            return POLL_IDLE_SECS;
        }

        let limit = state.limit;
        let hysteresis = state.hysteresis;
        drop(state);

        self.enforce(limit, hysteresis);
        POLL_ACTIVE_SECS
    }

    /// Core enforcement logic: read capacity and stop/resume charging as needed.
    fn enforce(&self, limit: u8, hysteresis: u8) {
        let stopped = is_charging_stopped();

        let status = fs::read_to_string(SYSFS_STATUS).unwrap_or_default();
        if status.trim() == "Discharging" && !stopped {
            eprintln!("charge_limit: on battery (not plugged in), skipping");
            return;
        }

        let capacity: u8 = fs::read_to_string(SYSFS_CAPACITY)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        eprintln!(
            "charge_limit: tick capacity={}% limit={}% stopped={}",
            capacity, limit, stopped
        );

        if capacity >= limit && !stopped {
            set_charging(false);
            eprintln!(
                "charge_limit: capacity {}% >= limit {}%, stopping charge",
                capacity, limit
            );
        } else if capacity <= limit.saturating_sub(hysteresis) && stopped {
            set_charging(true);
            eprintln!(
                "charge_limit: capacity {}% <= {}%, resuming charge",
                capacity,
                limit.saturating_sub(hysteresis)
            );
        }
    }

    pub fn get(&self) -> (bool, u8, u8, bool) {
        let state = self.inner.lock().unwrap();
        (state.enabled, state.limit, state.hysteresis, state.manual_override)
    }

    pub fn set(&self, enabled: bool, limit: u8, hysteresis: u8) -> Result<(), String> {
        if limit < 50 || limit > 100 {
            return Err("limit must be 50-100".into());
        }
        if hysteresis < 1 || hysteresis > 20 {
            return Err("hysteresis must be 1-20".into());
        }
        let mut state = self.inner.lock().unwrap();
        state.enabled = enabled;
        state.limit = limit;
        state.hysteresis = hysteresis;
        state.manual_override = false;
        self.save_locked(&state);
        drop(state);

        if enabled {
            eprintln!(
                "charge_limit: set enabled={} limit={}% hysteresis={}%, enforcing immediately",
                enabled, limit, hysteresis
            );
            self.enforce(limit, hysteresis);
        } else {
            eprintln!("charge_limit: disabled, resuming charge");
            set_charging(true);
        }

        Ok(())
    }

    pub fn set_manual_override(&self, override_on: bool) {
        let mut state = self.inner.lock().unwrap();
        state.manual_override = override_on;
    }

    fn save_locked(&self, state: &LimitState) {
        let p = Persisted {
            enabled: state.enabled,
            limit: state.limit,
            hysteresis: state.hysteresis,
        };
        if let Ok(json) = serde_json::to_string(&p) {
            let _ = fs::write(STORAGE_PATH, json);
        }
    }
}
