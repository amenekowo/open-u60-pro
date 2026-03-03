pub mod acl;
pub mod adb_enable;
pub mod backup;
pub mod companion;
pub mod explore;
pub mod monitor;
pub mod network;
pub mod probe;
pub mod settings;
pub mod setup;
pub mod ssh;

use anyhow::Result;
use clap::Subcommand;

use zte_lib::adb::AdbDevice;
use zte_lib::device::DeviceShell;
use zte_lib::ssh::SshDevice;
use zte_lib::ubus::UbusClient;

/// Shared CLI args for device connection (HTTP default, --ssh or --adb fallback).
#[derive(clap::Args, Clone)]
pub struct ShellArgs {
    /// Use ADB (USB) transport
    #[arg(long, global = true)]
    pub adb: bool,

    /// Use SSH transport instead of HTTP
    #[arg(long, global = true)]
    pub ssh: bool,

    /// ADB device serial (only with --adb)
    #[arg(short, long, global = true)]
    pub device: Option<String>,

    /// SSH host (default: 192.168.0.1)
    #[arg(long, global = true)]
    pub ssh_host: Option<String>,

    /// SSH port (default: 2222)
    #[arg(long, global = true, default_value_t = 2222)]
    pub ssh_port: u16,

    /// SSH identity file
    #[arg(long, global = true)]
    pub ssh_key: Option<String>,

    /// Router gateway IP for HTTP (default: auto-detect)
    #[arg(short, long, global = true)]
    pub gateway: Option<String>,

    /// Router admin password for HTTP
    #[arg(short, long, global = true)]
    pub password: Option<String>,
}

impl ShellArgs {
    /// Connect to device: HTTP (default), --ssh, or --adb.
    pub fn connect(&self) -> Result<DeviceShell> {
        if self.adb {
            let adb = AdbDevice::new(self.device.clone());
            if !adb.is_connected() {
                anyhow::bail!("No ADB device connected.");
            }
            Ok(DeviceShell::Adb(adb))
        } else if self.ssh {
            let ssh = SshDevice::new(
                self.ssh_host.clone(),
                Some(self.ssh_port),
                None,
                self.ssh_key.clone(),
            );
            if !ssh.is_connected() {
                anyhow::bail!(
                    "Cannot connect via SSH to {}:{}. Is dropbear running?",
                    ssh.host,
                    ssh.port,
                );
            }
            Ok(DeviceShell::Ssh(ssh))
        } else {
            // Default: HTTP/ubus
            let mut client = UbusClient::new(self.gateway.as_deref(), 10);
            let password = match &self.password {
                Some(p) => p.clone(),
                None => rpassword::prompt_password("Router admin password: ")?,
            };
            client
                .login(&password)
                .map_err(|e| anyhow::anyhow!("HTTP login failed: {e}"))?;
            Ok(DeviceShell::Http(client))
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage ubus HTTP ACL (unlock restricted API methods)
    #[command(subcommand)]
    Acl(acl::Cmd),

    /// Enable ADB (USB debug) mode on the device
    AdbEnable(adb_enable::Args),

    /// Deploy/manage zte-companion rpcd plugin (cpu_usage, battery_current)
    #[command(subcommand)]
    Companion(companion::Cmd),

    /// Collect device information and save a report
    Explore(explore::Args),

    /// All-in-one setup: enable ADB, install SSH, push keys
    Setup(setup::Args),

    /// Enable SSH (dropbear) access via ADB
    Ssh(ssh::Args),

    /// Network tools: DNS, TTL, band lock, firewall, telemetry
    #[command(subcommand)]
    Network(network::Cmd),

    /// Config backup, decrypt, view, and restore
    #[command(subcommand)]
    Backup(backup::Cmd),

    /// Live signal monitoring TUI dashboard
    Monitor(monitor::Args),

    /// Advanced device settings (100+ ubus endpoints)
    #[command(subcommand)]
    Settings(settings::Cmd),

    /// Enumerate and test ubus HTTP API endpoints
    Probe(probe::Args),
}

pub fn run(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Acl(cmd) => acl::run(cmd),
        Commands::AdbEnable(args) => adb_enable::run(args),
        Commands::Companion(cmd) => companion::run(cmd),
        Commands::Explore(args) => explore::run(args),
        Commands::Setup(args) => setup::run(args),
        Commands::Ssh(args) => ssh::run(args),
        Commands::Network(cmd) => network::run(cmd),
        Commands::Backup(cmd) => backup::run(cmd),
        Commands::Monitor(args) => monitor::run(args),
        Commands::Settings(cmd) => settings::run(cmd),
        Commands::Probe(args) => probe::run(args),
    }
}
