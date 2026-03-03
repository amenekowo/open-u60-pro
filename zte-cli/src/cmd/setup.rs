use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use colored::Colorize;
use serde_json::json;

use zte_lib::adb::AdbDevice;
use zte_lib::ubus::UbusClient;

use super::ssh;

#[derive(ClapArgs)]
pub struct Args {
    /// Gateway IP address (default: 192.168.0.1)
    #[arg(long)]
    gateway: Option<String>,

    /// Admin password (prompted interactively if omitted)
    #[arg(long)]
    password: Option<String>,

    /// SSH listen port on the device
    #[arg(long, default_value_t = 2222)]
    port: u16,

    /// Path to SSH public key to push for key-based auth
    #[arg(long)]
    key: Option<String>,

    /// Path to a local dropbear binary to push instead of downloading
    #[arg(long)]
    push_binary: Option<String>,
}

pub fn run(args: Args) -> Result<()> {
    // Step 1: Connect to gateway
    println!("{} Connecting to gateway...", "Step 1:".bold());
    let mut client = UbusClient::new(args.gateway.as_deref(), 10);
    println!("  Gateway: {}", client.gateway.cyan());

    // Step 2: Authenticate
    let password = match args.password {
        Some(p) => p,
        None => rpassword::prompt_password("Enter admin password: ")
            .context("Failed to read password")?,
    };
    println!("{} Logging in...", "Step 2:".bold());
    client
        .login(&password)
        .map_err(|e| anyhow::anyhow!("Login failed: {e}"))?;
    println!("  Authenticated successfully.");

    // Step 3: Enable USB debug mode
    println!("{} Enabling USB debug mode...", "Step 3:".bold());
    client
        .call("zwrt_bsp.usb", "set", Some(&json!({"mode": "debug"})))
        .map_err(|e| anyhow::anyhow!("Failed to set USB debug mode: {e}"))?;
    println!("  USB mode set to {}.", "debug".green());

    // Step 4: Wait for ADB device
    println!("{} Waiting for ADB device...", "Step 4:".bold());
    println!("  Make sure USB cable is connected to the device.");
    std::thread::sleep(std::time::Duration::from_secs(2));
    let adb = AdbDevice::new(None);
    adb.wait_for_device(15)
        .map_err(|_| anyhow::anyhow!("ADB device not detected. Check USB cable and retry."))?;
    println!("  {}", "ADB device connected.".green());

    // Step 5: Detect or push dropbear binary
    println!("{} Detecting SSH binary on device...", "Step 5:".bold());
    let dropbear_path;
    if let Some(path) = ssh::detect_dropbear(&adb) {
        println!("  Found: {}", path.green());
        dropbear_path = path;
    } else {
        println!("  No SSH binary found, installing...");
        if let Some(ref local) = args.push_binary {
            println!("  Using user-provided binary: {}", local.cyan());
            ssh::push_binary(&adb, local)?;
        } else {
            let local = ssh::download_dropbear()?;
            ssh::push_binary(&adb, &local)?;
        }
        dropbear_path = ssh::DROPBEAR_REMOTE.to_string();
    }

    // Step 6: Generate host keys and start dropbear
    println!(
        "{} Generating host keys and starting dropbear...",
        "Step 6:".bold()
    );
    ssh::generate_host_keys(&adb, &dropbear_path)?;
    ssh::start_dropbear(&adb, &dropbear_path, args.port)?;

    // Step 7: Persistence
    println!("{} Setting up boot persistence...", "Step 7:".bold());
    if let Err(e) = ssh::create_init_script(&adb, &dropbear_path, args.port) {
        println!(
            "  {}",
            format!("Persistence setup failed: {e}").yellow()
        );
    }

    // Step 8: SSH key
    if let Some(ref key_path) = args.key {
        println!("{} Installing SSH public key...", "Step 8:".bold());
        ssh::push_ssh_key(&adb, key_path)?;
    } else {
        println!(
            "{} No --key provided, skipping SSH key setup.",
            "Step 8:".bold()
        );
    }

    // Step 9: Verify
    println!("{} Verifying SSH...", "Step 9:".bold());
    ssh::verify_ssh(&adb, args.port);

    // Connection instructions
    println!();
    let device_ip = ssh::get_device_ip(&adb);
    let ip_display = device_ip.as_deref().unwrap_or("<device-ip>");
    println!("{}", "Connection instructions:".bold());
    println!("  ssh root@{ip_display} -p {}", args.port);
    if device_ip.is_none() {
        println!("  (Replace <device-ip> with the device's LAN/WAN IP address)");
    }
    println!();
    println!(
        "{}",
        "Done. SSH access is enabled on the device.".bold().green()
    );
    Ok(())
}
