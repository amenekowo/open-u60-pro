use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use colored::Colorize;
use serde_json::json;

use zte_lib::adb::AdbDevice;
use zte_lib::ubus::UbusClient;

#[derive(ClapArgs)]
pub struct Args {
    /// Gateway IP address (auto-detected if omitted)
    #[arg(long)]
    gateway: Option<String>,

    /// Admin password (prompted interactively if omitted)
    #[arg(long)]
    password: Option<String>,
}

pub fn run(args: Args) -> Result<()> {
    // Step 1: Connect to gateway
    println!("{} Connecting to gateway...", "Step 1:".bold());
    let mut client = UbusClient::new(args.gateway.as_deref(), 10);
    println!("  Gateway: {}", client.gateway.cyan());

    // Step 2: Get password & authenticate
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
    println!("{} Waiting for ADB device (up to 10s)...", "Step 4:".bold());
    println!("  Make sure USB cable is connected to the device.");
    std::thread::sleep(std::time::Duration::from_secs(2));
    let adb = AdbDevice::new(None);
    match adb.wait_for_device(10) {
        Ok(()) => {
            println!("{}", "ADB device detected!".bold().green());
            for (serial, state) in AdbDevice::get_devices() {
                println!("  {serial}  ({state})");
            }
        }
        Err(_) => {
            println!(
                "{}\n  This is normal if the USB cable is not connected.\n  Connect the cable and run: {}",
                "No ADB device detected yet.".yellow(),
                "adb devices".bold()
            );
        }
    }

    println!();
    println!("{} ADB debug mode is enabled on the gateway.", "Done.".bold());
    println!("Connect via: {}", "adb shell".bold().cyan());
    Ok(())
}
