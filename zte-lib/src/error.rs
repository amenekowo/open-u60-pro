use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZteError {
    #[error("ADB error: {0}")]
    Adb(String),

    #[error("ADB not found in PATH. Install Android platform-tools.")]
    AdbNotFound,

    #[error("ADB timed out after {timeout}s: {cmd}")]
    AdbTimeout { cmd: String, timeout: u64 },

    #[error("No ADB device found after {0}s. Is USB debugging enabled and cable connected?")]
    AdbNoDevice(u64),

    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("SSH not found in PATH. Install OpenSSH.")]
    SshNotFound,

    #[error("SSH timed out after {timeout}s: {cmd}")]
    SshTimeout { cmd: String, timeout: u64 },

    #[error("SSH connection failed to {host}:{port} — {reason}")]
    SshConnectionFailed {
        host: String,
        port: u16,
        reason: String,
    },

    #[error("ubus error: {0}")]
    Ubus(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("AT command error: {0}")]
    At(String),

    #[error("No modem serial device found")]
    AtNoDevice,

    #[error("ZTE config error: {0}")]
    Config(String),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ZteError>;
