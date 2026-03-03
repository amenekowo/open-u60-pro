use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::error::{Result, ZteError};

/// Wrapper around SSH/SCP subprocess calls to a remote device.
pub struct SshDevice {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub identity_file: Option<String>,
}

impl SshDevice {
    pub fn new(
        host: Option<String>,
        port: Option<u16>,
        user: Option<String>,
        identity_file: Option<String>,
    ) -> Self {
        Self {
            host: host.unwrap_or_else(|| "192.168.0.1".to_string()),
            port: port.unwrap_or(2222),
            user: user.unwrap_or_else(|| "root".to_string()),
            identity_file,
        }
    }

    /// Common SSH args: user@host, port, StrictHostKeyChecking, ControlMaster.
    fn ssh_args(&self) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            self.port.to_string(),
            "-o".to_string(),
            "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(),
            "UserKnownHostsFile=/dev/null".to_string(),
            "-o".to_string(),
            "LogLevel=ERROR".to_string(),
            "-o".to_string(),
            "ControlMaster=auto".to_string(),
            "-o".to_string(),
            format!(
                "ControlPath=/tmp/zte-ssh-{}@{}:{}",
                self.user, self.host, self.port
            ),
            "-o".to_string(),
            "ControlPersist=60".to_string(),
            "-o".to_string(),
            "ConnectTimeout=5".to_string(),
        ];
        if let Some(ref key) = self.identity_file {
            args.push("-i".to_string());
            args.push(key.clone());
        }
        args
    }

    /// Run a command via `ssh`, return stdout.
    pub fn shell(&self, cmd: &str, timeout_secs: u64) -> Result<String> {
        let mut ssh_args = self.ssh_args();
        ssh_args.push(format!("{}@{}", self.user, self.host));
        ssh_args.push(cmd.to_string());

        let mut child = Command::new("ssh")
            .args(&ssh_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ZteError::SshNotFound
                } else {
                    ZteError::Ssh(e.to_string())
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
                    let stderr_lower = stderr.to_lowercase();
                    if stderr_lower.contains("connection refused")
                        || stderr_lower.contains("no route to host")
                        || stderr_lower.contains("connection timed out")
                    {
                        return Err(ZteError::SshConnectionFailed {
                            host: self.host.clone(),
                            port: self.port,
                            reason: stderr.trim().to_string(),
                        });
                    }
                    return Err(ZteError::Ssh(format!(
                        "ssh command failed: {}",
                        stderr.trim()
                    )));
                }
                Ok(stdout)
            }
            Ok(None) => {
                child.kill().ok();
                Err(ZteError::SshTimeout {
                    cmd: cmd.to_string(),
                    timeout: timeout_secs,
                })
            }
            Err(e) => Err(ZteError::Ssh(e.to_string())),
        }
    }

    /// Push a local file to the device via scp.
    pub fn push(&self, local: &str, remote: &str) -> Result<String> {
        let mut args = Vec::new();
        args.push("-P".to_string());
        args.push(self.port.to_string());
        args.push("-o".to_string());
        args.push("StrictHostKeyChecking=no".to_string());
        args.push("-o".to_string());
        args.push("UserKnownHostsFile=/dev/null".to_string());
        args.push("-o".to_string());
        args.push("LogLevel=ERROR".to_string());
        args.push("-o".to_string());
        args.push(format!(
            "ControlPath=/tmp/zte-ssh-{}@{}:{}",
            self.user, self.host, self.port
        ));
        if let Some(ref key) = self.identity_file {
            args.push("-i".to_string());
            args.push(key.clone());
        }
        args.push(local.to_string());
        args.push(format!("{}@{}:{}", self.user, self.host, remote));

        let output = Command::new("scp")
            .args(&args)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ZteError::SshNotFound
                } else {
                    ZteError::Ssh(e.to_string())
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ZteError::Ssh(format!("scp push failed: {}", stderr.trim())));
        }
        Ok(format!("{local} -> {remote}"))
    }

    /// Pull a remote file from the device via scp.
    pub fn pull(&self, remote: &str, local: &str) -> Result<String> {
        let mut args = Vec::new();
        args.push("-P".to_string());
        args.push(self.port.to_string());
        args.push("-o".to_string());
        args.push("StrictHostKeyChecking=no".to_string());
        args.push("-o".to_string());
        args.push("UserKnownHostsFile=/dev/null".to_string());
        args.push("-o".to_string());
        args.push("LogLevel=ERROR".to_string());
        args.push("-o".to_string());
        args.push(format!(
            "ControlPath=/tmp/zte-ssh-{}@{}:{}",
            self.user, self.host, self.port
        ));
        if let Some(ref key) = self.identity_file {
            args.push("-i".to_string());
            args.push(key.clone());
        }
        // scp supports -r for directories
        args.push("-r".to_string());
        args.push(format!("{}@{}:{}", self.user, self.host, remote));
        args.push(local.to_string());

        let output = Command::new("scp")
            .args(&args)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ZteError::SshNotFound
                } else {
                    ZteError::Ssh(e.to_string())
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ZteError::Ssh(format!("scp pull failed: {}", stderr.trim())));
        }
        Ok(format!("{remote} -> {local}"))
    }

    /// Write content to a remote file by piping through SSH stdin.
    /// Avoids scp (not available on device) and base64 shell limits.
    pub fn write_content(&self, content: &[u8], remote_path: &str) -> Result<String> {
        let mut ssh_args = self.ssh_args();
        ssh_args.push(format!("{}@{}", self.user, self.host));
        ssh_args.push(format!("cat > {remote_path} && chmod +x {remote_path}"));

        let mut child = Command::new("ssh")
            .args(&ssh_args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ZteError::Ssh(e.to_string()))?;

        // Write content to stdin
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(content).map_err(|e| ZteError::Ssh(e.to_string()))?;
                // stdin is dropped here, closing the pipe
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|e| ZteError::Ssh(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ZteError::Ssh(format!(
                "write_content failed: {}",
                stderr.trim()
            )));
        }
        Ok(format!("wrote {} bytes to {remote_path}", content.len()))
    }

    /// Check if the device is reachable via SSH.
    pub fn is_connected(&self) -> bool {
        self.shell("echo ok", 5)
            .map(|out| out.trim() == "ok")
            .unwrap_or(false)
    }

    /// Poll until the device is reachable via SSH, or error on timeout.
    pub fn wait_for_device(&self, timeout_secs: u64) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        while Instant::now() < deadline {
            if self.is_connected() {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(1));
        }
        Err(ZteError::SshConnectionFailed {
            host: self.host.clone(),
            port: self.port,
            reason: format!("device not reachable after {timeout_secs}s"),
        })
    }

    /// Execute ubus call via `ssh`, return parsed JSON.
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
}
