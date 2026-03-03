use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::{json, Value};

use zte_lib::device::DeviceShell;
use zte_lib::ubus::UbusClient;

use crate::cmd::ShellArgs;

const ACL_PATH: &str = "/usr/share/rpcd/acl.d/web.json";
const ACL_DIR: &str = "/usr/share/rpcd/acl.d";
/// Writable copy of the ACL directory (rootfs is read-only, no overlay on /usr).
const ACL_OVERLAY_DIR: &str = "/data/local/tmp/rpcd-acl.d";
/// Boot script that re-applies the bind-mount after reboot.
const ACL_INIT_SCRIPT: &str = "/data/local/tmp/acl_patch.sh";

/// Objects to add to the `web.read.ubus` ACL section.
const UNLOCK_OBJECTS: &[(&str, &[&str])] = &[
    ("network.device", &["*"]),
    ("network.interface.lan", &["*"]),
    ("network.interface.zte_wan", &["*"]),
    ("network.interface.zte_wan6", &["*"]),
    ("system", &["info", "board"]),
    ("file", &["read", "stat"]),
    ("luci-rpc", &["*"]),
    ("zwrt_bsp.battery", &["*"]),
    ("zwrt_bsp.charger", &["*"]),
    ("zwrt_bsp.thermal", &["*"]),
    ("zwrt_mc.device.manager", &["get_device_info", "set_device_info"]),
    ("zte-companion", &["battery_current", "cpu_usage"]),
];

/// Objects to add to the `unauthenticated.read.ubus` ACL section.
/// The ZTE `zwrt_web/web_login` creates sessions that only check the
/// `unauthenticated` ACL scope, NOT the `web` scope. Without these,
/// file.read, system.info, etc. are blocked for iOS/web sessions.
/// (Excludes `zwrt_bsp.*` — already covered by the `zwrt_*` wildcard.)
const UNAUTH_UNLOCK_OBJECTS: &[(&str, &[&str])] = &[
    ("file", &["read", "stat"]),
    ("system", &["info", "board"]),
    ("network.device", &["*"]),
    ("network.interface.lan", &["*"]),
    ("network.interface.zte_wan", &["*"]),
    ("network.interface.zte_wan6", &["*"]),
    ("luci-rpc", &["*"]),
    ("zwrt_mc.device.manager", &["get_device_info", "set_device_info"]),
    ("zte-companion", &["battery_current", "cpu_usage"]),
];

#[derive(Subcommand)]
pub enum Cmd {
    /// Show current ubus HTTP ACL
    Show {
        #[command(flatten)]
        shell: ShellArgs,
    },
    /// Patch ACL to unlock restricted objects (luci-rpc, network.*)
    Patch {
        #[command(flatten)]
        shell: ShellArgs,
    },
    /// Reset ACL to factory default (unmount overlay)
    Reset {
        #[command(flatten)]
        shell: ShellArgs,
    },
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Show { shell } => run_show(&shell),
        Cmd::Patch { shell } => run_patch(&shell),
        Cmd::Reset { shell } => run_reset(&shell),
    }
}

fn run_show(shell: &ShellArgs) -> Result<()> {
    let dev = shell.connect()?;
    println!("\n  {}\n", "ACL — Current ubus HTTP ACL".bold());

    // Check if bind-mount is active
    let mount_check = dev
        .shell(&format!("mount | grep '{ACL_DIR}'"), 5)
        .unwrap_or_default();
    let is_bind_mounted = !mount_check.trim().is_empty();

    // Check if it's a symlink or a real file
    let readlink = dev
        .shell(&format!("readlink {ACL_PATH} 2>/dev/null"), 5)
        .unwrap_or_default();
    let is_symlink = !readlink.trim().is_empty();

    if is_bind_mounted {
        println!("  {} patched (bind-mount from {ACL_OVERLAY_DIR})", "Type:".bold());
    } else if is_symlink {
        println!(
            "  {} factory symlink → {}",
            "Type:".bold(),
            readlink.trim().cyan()
        );
    } else {
        println!("  {} real file", "Type:".bold());
    }

    let raw = dev.shell(&format!("cat {ACL_PATH}"), 5)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        println!("  {} ACL file is empty or missing.", "!".yellow());
        return Ok(());
    }

    let acl: Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow::anyhow!("Failed to parse ACL JSON: {e}"))?;

    // Show which objects are in web.read.ubus
    if let Some(ubus_obj) = acl.pointer("/web/read/ubus") {
        println!("  {}", "Allowed ubus objects (web.read.ubus):".bold());
        if let Value::Object(map) = ubus_obj {
            for (obj_name, methods) in map {
                let methods_str = match methods {
                    Value::Array(arr) => arr
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    _ => format!("{methods}"),
                };
                let is_unlock_target = UNLOCK_OBJECTS.iter().any(|(name, _)| name == obj_name);
                if is_unlock_target {
                    println!("    {} [{}]", obj_name.green(), methods_str);
                } else {
                    println!("    {obj_name} [{methods_str}]");
                }
            }
        }
    } else {
        println!("  {} No web.read.ubus section found.", "!".yellow());
    }

    // Show unauthenticated.read.ubus status
    if let Some(ubus_obj) = acl.pointer("/unauthenticated/read/ubus") {
        println!(
            "\n  {}",
            "Allowed ubus objects (unauthenticated.read.ubus):".bold()
        );
        if let Value::Object(map) = ubus_obj {
            for (obj_name, methods) in map {
                let methods_str = match methods {
                    Value::Array(arr) => arr
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    _ => format!("{methods}"),
                };
                let is_unlock_target =
                    UNAUTH_UNLOCK_OBJECTS.iter().any(|(name, _)| name == obj_name);
                if is_unlock_target {
                    println!("    {} [{}]", obj_name.green(), methods_str);
                } else {
                    println!("    {obj_name} [{methods_str}]");
                }
            }
        }
    } else {
        println!(
            "\n  {} No unauthenticated.read.ubus section found.",
            "!".yellow()
        );
    }

    let missing_web = get_missing_for_scope(&acl, "/web/read/ubus", UNLOCK_OBJECTS);
    let missing_unauth =
        get_missing_for_scope(&acl, "/unauthenticated/read/ubus", UNAUTH_UNLOCK_OBJECTS);

    if missing_web.is_empty() && missing_unauth.is_empty() {
        println!(
            "\n  {}",
            "All target objects are already unlocked.".green()
        );
    } else {
        if !missing_web.is_empty() {
            println!("\n  {} Missing from web.read.ubus:", "!".yellow());
            for (obj, _) in &missing_web {
                println!("    {} {obj}", "-".red());
            }
        }
        if !missing_unauth.is_empty() {
            println!(
                "\n  {} Missing from unauthenticated.read.ubus:",
                "!".yellow()
            );
            for (obj, _) in &missing_unauth {
                println!("    {} {obj}", "-".red());
            }
        }
        println!("\n  Run {} to add them.", "zte acl patch".cyan());
    }

    println!();
    Ok(())
}

fn run_patch(shell: &ShellArgs) -> Result<()> {
    let password = shell.password.clone();
    let gateway = shell.gateway.clone();
    let dev = shell.connect()?;
    println!("\n  {}\n", "ACL — Patching ubus HTTP ACL".bold());

    // Read current ACL (follows symlink)
    let raw = dev.shell(&format!("cat {ACL_PATH}"), 5)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        anyhow::bail!("ACL file is empty or missing at {ACL_PATH}");
    }

    let mut acl: Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow::anyhow!("Failed to parse ACL JSON: {e}"))?;

    let missing_web = get_missing_for_scope(&acl, "/web/read/ubus", UNLOCK_OBJECTS);
    let missing_unauth =
        get_missing_for_scope(&acl, "/unauthenticated/read/ubus", UNAUTH_UNLOCK_OBJECTS);

    if missing_web.is_empty() && missing_unauth.is_empty() {
        println!(
            "  {}",
            "All target objects are already unlocked. Nothing to do.".green()
        );
        return Ok(());
    }

    // Patch web.read.ubus
    if !missing_web.is_empty() {
        println!("  Adding {} objects to web.read.ubus:", missing_web.len());
        for (obj, methods) in &missing_web {
            println!("    {} {obj} [{}]", "+".green(), methods.join(", "));
        }
        let ubus_obj = acl
            .pointer_mut("/web/read/ubus")
            .ok_or_else(|| anyhow::anyhow!("No web.read.ubus section in ACL"))?;
        if let Value::Object(map) = ubus_obj {
            for (obj_name, methods) in &missing_web {
                let method_values: Vec<Value> = methods.iter().map(|m| json!(m)).collect();
                map.insert(obj_name.to_string(), Value::Array(method_values));
            }
        }
    }

    // Patch unauthenticated.read.ubus
    if !missing_unauth.is_empty() {
        println!(
            "\n  Adding {} objects to unauthenticated.read.ubus:",
            missing_unauth.len()
        );
        for (obj, methods) in &missing_unauth {
            println!("    {} {obj} [{}]", "+".green(), methods.join(", "));
        }
        // Ensure unauthenticated.read.ubus exists
        if acl.pointer("/unauthenticated").is_none() {
            acl["unauthenticated"] = json!({});
        }
        if acl.pointer("/unauthenticated/read").is_none() {
            acl["unauthenticated"]["read"] = json!({});
        }
        if acl.pointer("/unauthenticated/read/ubus").is_none() {
            acl["unauthenticated"]["read"]["ubus"] = json!({});
        }
        let ubus_obj = acl
            .pointer_mut("/unauthenticated/read/ubus")
            .expect("just created unauthenticated.read.ubus");
        if let Value::Object(map) = ubus_obj {
            for (obj_name, methods) in &missing_unauth {
                let method_values: Vec<Value> = methods.iter().map(|m| json!(m)).collect();
                map.insert(obj_name.to_string(), Value::Array(method_values));
            }
        }
    }

    let patched_json = serde_json::to_string_pretty(&acl)?;

    // Strategy: rootfs is read-only with no overlay on /usr/share.
    // 1. Copy the entire ACL dir to a writable location
    // 2. Write the patched file there
    // 3. Bind-mount over the read-only dir
    println!("\n  Setting up writable overlay via bind-mount...");

    // Create writable copy of the ACL directory (dereference symlinks with -L)
    dev.shell(
        &format!("mkdir -p {ACL_OVERLAY_DIR} && cp -L {ACL_DIR}/* {ACL_OVERLAY_DIR}/ 2>/dev/null; true"),
        5,
    )?;

    // Remove any leftover symlink, then write patched JSON
    let b64 = base64_encode(patched_json.as_bytes());
    dev.shell(
        &format!("rm -f {ACL_OVERLAY_DIR}/web.json && echo '{b64}' | base64 -d > {ACL_OVERLAY_DIR}/web.json"),
        10,
    )?;

    // Unmount any existing bind-mount first
    dev.shell(&format!("umount {ACL_DIR} 2>/dev/null; true"), 5)?;

    // Bind-mount the writable directory over the read-only one
    dev.shell(
        &format!("mount --bind {ACL_OVERLAY_DIR} {ACL_DIR}"),
        5,
    )?;
    println!("  {} Bind-mounted {ACL_OVERLAY_DIR} → {ACL_DIR}", "OK".green());

    // Reload rpcd
    println!("  Reloading rpcd...");
    let reload_result = dev.shell("kill -HUP $(pidof rpcd)", 5);
    match reload_result {
        Ok(_) => println!("  {} rpcd reloaded.", "OK".green()),
        Err(e) => {
            println!(
                "  {} HUP failed ({}), trying restart...",
                "!".yellow(),
                e
            );
            let _ = dev.shell("/etc/init.d/rpcd restart", 10);
        }
    }

    // Brief pause for rpcd to reload
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Verify via HTTP
    if let Some(pw) = &password {
        println!("\n  Verifying via HTTP ubus call...");
        let mut client = UbusClient::new(gateway.as_deref(), 10);
        match client.login(pw) {
            Ok(_) => match client.call("network.device", "status", None) {
                Ok(_) => println!(
                    "  {} network.device.status is now accessible via HTTP!",
                    "OK".green().bold()
                ),
                Err(e) => println!("  {} Verification failed: {e}", "FAIL".red()),
            },
            Err(e) => println!(
                "  {} Could not login for verification: {e}",
                "!".yellow()
            ),
        }
    } else {
        println!(
            "\n  {} Use -p <password> to auto-verify, or run: {}",
            "Tip:".bold(),
            "zte probe --retry -p <password>".cyan()
        );
    }

    // Install boot persistence
    install_boot_persistence(&dev);

    println!();
    Ok(())
}

fn run_reset(shell: &ShellArgs) -> Result<()> {
    let dev = shell.connect()?;
    println!("\n  {}\n", "ACL — Resetting to factory default".bold());

    // Check if bind-mount is active
    let mount_check = dev
        .shell(&format!("mount | grep '{ACL_DIR}'"), 5)
        .unwrap_or_default();

    if mount_check.trim().is_empty() {
        println!(
            "  {} No bind-mount active. ACL is already at factory default.",
            "OK".green()
        );
        return Ok(());
    }

    // Unmount the bind-mount
    dev.shell(&format!("umount {ACL_DIR}"), 5)?;
    println!(
        "  {} Unmounted bind-mount, original ACL restored.",
        "OK".green()
    );

    // Clean up the writable copy
    dev.shell(&format!("rm -rf {ACL_OVERLAY_DIR}"), 5)?;
    println!("  {} Cleaned up {ACL_OVERLAY_DIR}", "OK".green());

    // Remove boot persistence
    remove_boot_persistence(&dev);

    // Reload rpcd
    println!("  Reloading rpcd...");
    let reload_result = dev.shell("kill -HUP $(pidof rpcd)", 5);
    match reload_result {
        Ok(_) => println!("  {} rpcd reloaded.", "OK".green()),
        Err(e) => {
            println!(
                "  {} HUP failed ({}), trying restart...",
                "!".yellow(),
                e
            );
            let _ = dev.shell("/etc/init.d/rpcd restart", 10);
        }
    }

    println!();
    Ok(())
}

/// Write a boot script and hook it into rc.local so the ACL bind-mount survives reboot.
fn install_boot_persistence(dev: &DeviceShell) {
    // Write the init script
    let script = "\
#!/bin/sh\n\
# Re-apply ACL bind-mount on boot\n\
if [ -d /data/local/tmp/rpcd-acl.d ]; then\n\
  mount | grep -q '/usr/share/rpcd/acl.d' || {\n\
    mount --bind /data/local/tmp/rpcd-acl.d /usr/share/rpcd/acl.d\n\
    kill -HUP $(pidof rpcd) 2>/dev/null\n\
  }\n\
fi\n";

    let b64 = base64_encode(script.as_bytes());
    let write_result = dev.shell(
        &format!("echo '{b64}' | base64 -d > {ACL_INIT_SCRIPT} && chmod +x {ACL_INIT_SCRIPT}"),
        5,
    );
    if write_result.is_err() {
        println!(
            "  {} Could not write boot script to {ACL_INIT_SCRIPT}",
            "!".yellow()
        );
        return;
    }
    println!("\n  Init script written to {}", ACL_INIT_SCRIPT.cyan());

    // Hook into rc.local (same pattern as ssh.rs)
    let check = dev
        .shell(
            &format!("grep -q '{ACL_INIT_SCRIPT}' /etc/rc.local 2>/dev/null && echo exists || true"),
            5,
        )
        .unwrap_or_default();
    if check.trim() == "exists" {
        println!("  rc.local already references the init script.");
    } else {
        let has_exit = dev
            .shell("grep -q '^exit 0' /etc/rc.local 2>/dev/null && echo yes || true", 5)
            .unwrap_or_default();
        let cmd = if has_exit.trim() == "yes" {
            format!("sed -i '/^exit 0/i {ACL_INIT_SCRIPT} &' /etc/rc.local 2>&1 || echo READONLY")
        } else {
            format!("echo \"{ACL_INIT_SCRIPT} &\" >> /etc/rc.local 2>&1 || echo READONLY")
        };
        let result = dev.shell(&cmd, 5).unwrap_or_default();
        if result.contains("READONLY") || result.contains("Read-only") {
            println!(
                "  {} Could not modify /etc/rc.local (read-only filesystem).",
                "!".yellow()
            );
            println!(
                "  {} Bind-mount will be lost on reboot. Re-run {} to re-apply.",
                "Note:".bold(),
                "zte acl patch".cyan()
            );
        } else {
            println!("  {} Boot persistence installed.", "OK".green());
        }
    }
}

/// Remove the boot script and its rc.local entry.
fn remove_boot_persistence(dev: &DeviceShell) {
    let _ = dev.shell(&format!("rm -f {ACL_INIT_SCRIPT}"), 5);

    let check = dev
        .shell(
            &format!("grep -q '{ACL_INIT_SCRIPT}' /etc/rc.local 2>/dev/null && echo exists || true"),
            5,
        )
        .unwrap_or_default();
    if check.trim() == "exists" {
        let result = dev
            .shell(
                &format!("sed -i '\\|{ACL_INIT_SCRIPT}|d' /etc/rc.local 2>&1 || echo READONLY"),
                5,
            )
            .unwrap_or_default();
        if result.contains("READONLY") || result.contains("Read-only") {
            println!(
                "  {} Could not remove entry from /etc/rc.local (read-only filesystem).",
                "!".yellow()
            );
        } else {
            println!("  {} Removed boot persistence.", "OK".green());
        }
    }
    let _ = dev.shell(&format!("rm -f {ACL_INIT_SCRIPT}"), 5);
}

/// Returns objects from `targets` that are missing from the given JSON pointer scope.
fn get_missing_for_scope(
    acl: &Value,
    pointer: &str,
    targets: &[(&str, &[&str])],
) -> Vec<(String, Vec<String>)> {
    let ubus_obj = match acl.pointer(pointer) {
        Some(Value::Object(map)) => map,
        _ => {
            return targets
                .iter()
                .map(|(name, methods)| {
                    (
                        name.to_string(),
                        methods.iter().map(|m| m.to_string()).collect(),
                    )
                })
                .collect()
        }
    };

    targets
        .iter()
        .filter(|(name, _)| !ubus_obj.contains_key(*name))
        .map(|(name, methods)| {
            (
                name.to_string(),
                methods.iter().map(|m| m.to_string()).collect(),
            )
        })
        .collect()
}

/// Simple base64 encoder (avoids adding a dependency for this one use).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
