use serde_json::Value;

use crate::adb::AdbDevice;
use crate::device::DeviceShell;
use crate::error::{Result, ZteError};
use crate::ubus::UbusClient;

/// Dual ADB/SSH/HTTP transport for ubus calls.
pub enum Transport<'a> {
    Shell(&'a DeviceShell),
    Http(&'a UbusClient),
}

impl<'a> Transport<'a> {
    /// Execute a ubus call through the appropriate transport.
    pub fn ubus_call(
        &self,
        obj: &str,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Value> {
        match self {
            Transport::Shell(dev) => dev.ubus_call(obj, method, params, 10),
            Transport::Http(client) => client.call(obj, method, params),
        }
    }

    /// Returns true if this is a shell (ADB/SSH) transport.
    pub fn is_shell(&self) -> bool {
        matches!(self, Transport::Shell(_))
    }

    /// Require shell transport, returning an error for HTTP.
    pub fn require_shell(&self, action: &str) -> Result<&DeviceShell> {
        match self {
            Transport::Shell(dev) => Ok(dev),
            Transport::Http(_) => Err(ZteError::Ubus(format!(
                "{action} requires shell access (ADB/SSH) — not available in WiFi mode."
            ))),
        }
    }

    /// Require ADB specifically (e.g., for `zte ssh` install).
    pub fn require_adb(&self, action: &str) -> Result<&AdbDevice> {
        match self {
            Transport::Shell(dev) => dev.require_adb(action),
            Transport::Http(_) => Err(ZteError::Ubus(format!(
                "{action} requires ADB (USB) — not available in WiFi mode."
            ))),
        }
    }
}
