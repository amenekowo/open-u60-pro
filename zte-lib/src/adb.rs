use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::error::{Result, ZteError};

/// Wrapper around ADB subprocess calls.
pub struct AdbDevice {
    serial: Option<String>,
}

impl AdbDevice {
    pub fn new(serial: Option<String>) -> Self {
        Self { serial }
    }

    fn base_cmd(&self) -> Command {
        let mut cmd = Command::new("adb");
        if let Some(ref s) = self.serial {
            cmd.args(["-s", s]);
        }
        cmd
    }

    /// Run a command via `adb shell`, return stdout.
    pub fn shell(&self, cmd: &str, timeout_secs: u64) -> Result<String> {
        let mut child = self
            .base_cmd()
            .args(["shell", cmd])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ZteError::AdbNotFound
                } else {
                    ZteError::Adb(e.to_string())
                }
            })?;

        let timeout = Duration::from_secs(timeout_secs);
        use wait_timeout::ChildExt;
        match child.wait_timeout(timeout) {
            Ok(Some(status)) => {
                let stdout = {
                    use std::io::Read;
                    let mut s = String::new();
                    if let Some(mut out) = child.stdout.take() {
                        out.read_to_string(&mut s).ok();
                    }
                    s
                };
                let stderr = {
                    use std::io::Read;
                    let mut s = String::new();
                    if let Some(mut err) = child.stderr.take() {
                        err.read_to_string(&mut s).ok();
                    }
                    s
                };
                if !status.success() && !stderr.trim().is_empty() {
                    return Err(ZteError::Adb(format!(
                        "adb shell failed: {}",
                        stderr.trim()
                    )));
                }
                Ok(stdout)
            }
            Ok(None) => {
                child.kill().ok();
                Err(ZteError::AdbTimeout {
                    cmd: cmd.to_string(),
                    timeout: timeout_secs,
                })
            }
            Err(e) => Err(ZteError::Adb(e.to_string())),
        }
    }

    /// Push a local file to the device.
    pub fn push(&self, local: &str, remote: &str) -> Result<String> {
        let output = self
            .base_cmd()
            .args(["push", local, remote])
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ZteError::AdbNotFound
                } else {
                    ZteError::Adb(e.to_string())
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ZteError::Adb(format!("adb push failed: {}", stderr.trim())));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Pull a remote file from the device.
    pub fn pull(&self, remote: &str, local: &str) -> Result<String> {
        let output = self
            .base_cmd()
            .args(["pull", remote, local])
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ZteError::AdbNotFound
                } else {
                    ZteError::Adb(e.to_string())
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ZteError::Adb(format!("adb pull failed: {}", stderr.trim())));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if a device is connected via ADB.
    pub fn is_connected(&self) -> bool {
        let output = match self.base_cmd().args(["devices"]).output() {
            Ok(o) => o,
            Err(_) => return false,
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == "device" {
                if self.serial.is_none() || Some(parts[0]) == self.serial.as_deref() {
                    return true;
                }
            }
        }
        false
    }

    /// Poll until a device appears, or error on timeout.
    pub fn wait_for_device(&self, timeout_secs: u64) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        while Instant::now() < deadline {
            if self.is_connected() {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(1));
        }
        Err(ZteError::AdbNoDevice(timeout_secs))
    }

    /// Execute ubus call via `adb shell`, return parsed JSON.
    pub fn ubus_call(
        &self,
        obj: &str,
        method: &str,
        params: Option<&Value>,
        timeout_secs: u64,
    ) -> Result<Value> {
        let params_str = match params {
            Some(v) => serde_json::to_string(v)?,
            None => "{}".to_string(),
        };
        let raw = self.shell(
            &format!("ubus call {obj} {method} '{params_str}' 2>/dev/null"),
            timeout_secs,
        )?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Value::Object(serde_json::Map::new()));
        }
        match serde_json::from_str(trimmed) {
            Ok(v) => Ok(v),
            Err(_) => {
                let mut map = serde_json::Map::new();
                map.insert("_raw".to_string(), Value::String(trimmed.to_string()));
                Ok(Value::Object(map))
            }
        }
    }

    /// Return list of (serial, state) pairs.
    pub fn get_devices() -> Vec<(String, String)> {
        let output = match Command::new("adb").arg("devices").output() {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                devices.push((parts[0].to_string(), parts[1].to_string()));
            }
        }
        devices
    }
}
