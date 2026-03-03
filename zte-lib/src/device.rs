use serde_json::{json, Value};

use crate::adb::AdbDevice;
use crate::error::{Result, ZteError};
use crate::ssh::SshDevice;
use crate::ubus::UbusClient;

/// Unified device shell — delegates to ADB, SSH, or HTTP transport.
pub enum DeviceShell {
    Adb(AdbDevice),
    Ssh(SshDevice),
    Http(UbusClient),
}

impl DeviceShell {
    /// Run a shell command on the device.
    pub fn shell(&self, cmd: &str, timeout_secs: u64) -> Result<String> {
        match self {
            DeviceShell::Adb(adb) => adb.shell(cmd, timeout_secs),
            DeviceShell::Ssh(ssh) => ssh.shell(cmd, timeout_secs),
            DeviceShell::Http(_) => Err(ZteError::Ubus(
                "Shell access requires --ssh or --adb (not available over HTTP)".into(),
            )),
        }
    }

    /// Push a local file to the device.
    pub fn push(&self, local: &str, remote: &str) -> Result<String> {
        match self {
            DeviceShell::Adb(adb) => adb.push(local, remote),
            DeviceShell::Ssh(ssh) => ssh.push(local, remote),
            DeviceShell::Http(_) => Err(ZteError::Ubus(
                "File push requires --ssh or --adb (not available over HTTP)".into(),
            )),
        }
    }

    /// Pull a remote file from the device.
    pub fn pull(&self, remote: &str, local: &str) -> Result<String> {
        match self {
            DeviceShell::Adb(adb) => adb.pull(remote, local),
            DeviceShell::Ssh(ssh) => ssh.pull(remote, local),
            DeviceShell::Http(_) => Err(ZteError::Ubus(
                "File pull requires --ssh or --adb (not available over HTTP)".into(),
            )),
        }
    }

    /// Write content directly to a remote file (avoids shell argument limits).
    /// SSH: pipes via stdin. ADB: writes temp file + adb push.
    pub fn write_content(&self, content: &[u8], remote_path: &str) -> Result<String> {
        match self {
            DeviceShell::Ssh(ssh) => ssh.write_content(content, remote_path),
            DeviceShell::Adb(adb) => {
                let tmp = "/tmp/zte-write-content.tmp";
                std::fs::write(tmp, content)
                    .map_err(|e| ZteError::Adb(format!("write temp file: {e}")))?;
                let result = adb.push(tmp, remote_path);
                let _ = std::fs::remove_file(tmp);
                result
            }
            DeviceShell::Http(_) => Err(ZteError::Ubus(
                "File write requires --ssh or --adb (not available over HTTP)".into(),
            )),
        }
    }

    /// Check if the device is connected/reachable.
    pub fn is_connected(&self) -> bool {
        match self {
            DeviceShell::Adb(adb) => adb.is_connected(),
            DeviceShell::Ssh(ssh) => ssh.is_connected(),
            DeviceShell::Http(client) => client.is_authenticated(),
        }
    }

    /// Wait until the device is connected/reachable.
    pub fn wait_for_device(&self, timeout_secs: u64) -> Result<()> {
        match self {
            DeviceShell::Adb(adb) => adb.wait_for_device(timeout_secs),
            DeviceShell::Ssh(ssh) => ssh.wait_for_device(timeout_secs),
            DeviceShell::Http(client) => {
                if client.is_authenticated() {
                    Ok(())
                } else {
                    Err(ZteError::Auth("HTTP client not authenticated".into()))
                }
            }
        }
    }

    /// Execute ubus call, return parsed JSON.
    pub fn ubus_call(
        &self,
        obj: &str,
        method: &str,
        params: Option<&Value>,
        timeout_secs: u64,
    ) -> Result<Value> {
        match self {
            DeviceShell::Adb(adb) => adb.ubus_call(obj, method, params, timeout_secs),
            DeviceShell::Ssh(ssh) => ssh.ubus_call(obj, method, params, timeout_secs),
            DeviceShell::Http(client) => client.call(obj, method, params),
        }
    }

    /// Convenience ubus call: 3-arg, returns `json!({})` on error.
    pub fn ubus_call_quiet(&self, obj: &str, method: &str, params: Option<&Value>) -> Value {
        self.ubus_call(obj, method, params, 10).unwrap_or(json!({}))
    }

    /// Require ADB transport, returning an error if not ADB.
    pub fn require_adb(&self, action: &str) -> Result<&AdbDevice> {
        match self {
            DeviceShell::Adb(adb) => Ok(adb),
            _ => Err(ZteError::Adb(format!(
                "{action} requires ADB (USB) — not available over {}.",
                self.transport_name()
            ))),
        }
    }

    /// Human-readable transport name.
    pub fn transport_name(&self) -> &'static str {
        match self {
            DeviceShell::Adb(_) => "ADB",
            DeviceShell::Ssh(_) => "SSH",
            DeviceShell::Http(_) => "HTTP",
        }
    }
}
