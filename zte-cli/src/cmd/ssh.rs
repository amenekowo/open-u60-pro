use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use colored::Colorize;

use zte_lib::adb::AdbDevice;

pub(crate) const DROPBEAR_REMOTE: &str = "/data/local/tmp/dropbear";
pub(crate) const DROPBEAR_KEY_DIR: &str = "/etc/dropbear";
pub(crate) const DROPBEAR_HOST_KEY: &str = "/etc/dropbear/dropbear_rsa_host_key";
pub(crate) const INIT_SCRIPT: &str = "/data/local/tmp/start_ssh.sh";
pub(crate) const AUTH_KEYS_PATH: &str = "/etc/dropbear/authorized_keys";
const DROPBEAR_DOWNLOAD_URL: &str =
    "https://downloads.openwrt.org/releases/23.05.4/targets/armsr/armv8/packages/dropbear_2022.82-6_aarch64_generic.ipk";

fn cache_dir() -> PathBuf {
    dirs_next().join("dropbear-aarch64")
}

fn dirs_next() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache/zte-toolkit")
}

#[derive(ClapArgs)]
pub struct Args {
    /// SSH listen port on the device
    #[arg(long, default_value_t = 22)]
    port: u16,

    /// Path to SSH public key to push for key-based auth
    #[arg(long)]
    key: Option<String>,

    /// Path to a local dropbear binary to push
    #[arg(long)]
    push_binary: Option<String>,

    /// Actually apply changes (dry-run without this flag)
    #[arg(long)]
    confirm: bool,
}

pub fn run(args: Args) -> Result<()> {
    let dry_run = !args.confirm;
    if dry_run {
        println!(
            "{} -- pass {} to apply changes.\n",
            "DRY RUN".bold().yellow(),
            "--confirm".bold()
        );
    }

    // Step 1: Check ADB
    println!("{} Checking ADB connection...", "Step 1:".bold());
    let adb = AdbDevice::new(None);
    if !adb.is_connected() {
        anyhow::bail!("No ADB device connected. Run `zte adb-enable` first.");
    }
    println!("  ADB device connected.");

    // Step 2: Detect existing SSH binary
    println!("{} Detecting SSH binary on device...", "Step 2:".bold());
    let existing = detect_dropbear(&adb);
    let mut need_push = false;
    let dropbear_path;

    if let Some(ref path) = existing {
        println!("  Found: {}", path.green());
        dropbear_path = path.clone();
    } else {
        println!("  No SSH binary found on device.");
        need_push = true;
        dropbear_path = DROPBEAR_REMOTE.to_string();
    }

    // Step 3: Obtain and push binary
    if need_push {
        println!("{} Obtaining dropbear binary...", "Step 3:".bold());
        if let Some(ref path) = args.push_binary {
            println!("  Using user-provided binary: {}", path.cyan());
            if dry_run {
                println!("  {}", format!("Would push binary to {DROPBEAR_REMOTE}").yellow());
            } else {
                push_binary(&adb, path)?;
            }
        } else if dry_run {
            println!(
                "  {}",
                "Would download and push dropbear binary.".yellow()
            );
        } else {
            let local = download_dropbear()?;
            push_binary(&adb, &local)?;
        }
    } else {
        println!("{} Binary already present, skipping push.", "Step 3:".bold());
    }

    // Step 4: Generate host keys and start
    println!(
        "{} Generating host keys and starting dropbear...",
        "Step 4:".bold()
    );
    if dry_run {
        println!("  {}", format!("Would generate host keys at {DROPBEAR_HOST_KEY}").yellow());
        println!("  {}", format!("Would start dropbear on port {}", args.port).yellow());
    } else {
        generate_host_keys(&adb, &dropbear_path)?;
        start_dropbear(&adb, &dropbear_path, args.port)?;
    }

    // Step 5: Persistence
    println!("{} Setting up persistence...", "Step 5:".bold());
    if dry_run {
        println!("  {}", format!("Would create init script at {INIT_SCRIPT}").yellow());
    } else {
        if let Err(e) = create_init_script(&adb, &dropbear_path, args.port) {
            println!("  {}", format!("Persistence setup failed: {e}").yellow());
        }
    }

    // Step 6: SSH key auth
    if let Some(ref key_path) = args.key {
        println!("{} Installing SSH public key...", "Step 6:".bold());
        if dry_run {
            println!("  {}", format!("Would push {key_path} to {AUTH_KEYS_PATH}").yellow());
        } else {
            push_ssh_key(&adb, key_path)?;
        }
    } else {
        println!("{} No --key provided, skipping SSH key setup.", "Step 6:".bold());
    }

    // Step 7: Verify
    println!("{} Verification...", "Step 7:".bold());
    if dry_run {
        println!("  {}", "Skipping verification in dry-run mode.".yellow());
    } else {
        verify_ssh(&adb, args.port);
    }

    // Step 8: Connection instructions
    println!();
    let device_ip = if dry_run {
        None
    } else {
        get_device_ip(&adb)
    };
    let ip_display = device_ip.as_deref().unwrap_or("<device-ip>");
    println!("{}", "Connection instructions:".bold());
    println!("  ssh root@{ip_display} -p {}", args.port);
    if device_ip.is_none() {
        println!("  (Replace <device-ip> with the device's LAN/WAN IP address)");
    }
    println!();
    if dry_run {
        println!(
            "{} Re-run with {} to apply.",
            "No changes were made.".bold().yellow(),
            "--confirm".bold()
        );
    } else {
        println!("{}", "Done. SSH access is enabled on the device.".bold().green());
    }
    Ok(())
}

pub(crate) fn detect_dropbear(adb: &AdbDevice) -> Option<String> {
    for name in ["dropbear", "sshd"] {
        if let Ok(out) = adb.shell(&format!("which {name} 2>/dev/null"), 5) {
            let out = out.trim().to_string();
            if !out.is_empty() {
                return Some(out);
            }
        }
    }
    if let Ok(out) = adb.shell(&format!("test -x {DROPBEAR_REMOTE} && echo found"), 5) {
        if out.trim() == "found" {
            return Some(DROPBEAR_REMOTE.to_string());
        }
    }
    None
}

pub(crate) fn download_dropbear() -> Result<String> {
    let cached = cache_dir();
    if cached.exists() {
        println!(
            "  Using cached binary: {}",
            cached.display().to_string().cyan()
        );
        return Ok(cached.display().to_string());
    }
    println!("  Downloading dropbear from OpenWrt packages...");
    let cache_parent = cached
        .parent()
        .context("invalid cache path")?;
    fs::create_dir_all(cache_parent)?;

    let ipk_path = cache_parent.join("dropbear.ipk");

    // Download the ipk
    let output = Command::new("curl")
        .args(["-fSL", "-o"])
        .arg(&ipk_path)
        .arg(DROPBEAR_DOWNLOAD_URL)
        .output()
        .context("curl not found")?;
    if !output.status.success() {
        anyhow::bail!(
            "Failed to download dropbear. Provide one with --push-binary PATH"
        );
    }

    // Extract dropbear binary from ipk (ipk is a tar.gz containing data.tar.gz)
    let extract_dir = cache_parent.join("dropbear-extract");
    fs::create_dir_all(&extract_dir)?;

    let status = Command::new("tar")
        .args(["xzf"])
        .arg(&ipk_path)
        .arg("-C")
        .arg(&extract_dir)
        .status()
        .context("tar not found")?;
    if !status.success() {
        // Try ar extraction as fallback (some ipk formats)
        Command::new("ar")
            .arg("x")
            .arg(&ipk_path)
            .current_dir(&extract_dir)
            .status()
            .context("ar not found")?;
    }

    let data_tar = extract_dir.join("data.tar.gz");
    if data_tar.exists() {
        Command::new("tar")
            .args(["xzf"])
            .arg(&data_tar)
            .arg("-C")
            .arg(&extract_dir)
            .status()?;
    }

    let extracted_bin = extract_dir.join("usr/sbin/dropbear");
    if !extracted_bin.exists() {
        let _ = fs::remove_dir_all(&extract_dir);
        let _ = fs::remove_file(&ipk_path);
        anyhow::bail!(
            "Failed to extract dropbear from ipk. Provide one with --push-binary PATH"
        );
    }

    fs::copy(&extracted_bin, &cached)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&cached, fs::Permissions::from_mode(0o755))?;
    }

    // Also cache dropbearkey if present
    let extracted_keygen = extract_dir.join("usr/bin/dropbearkey");
    if extracted_keygen.exists() {
        let keygen_cached = cache_parent.join("dropbearkey-aarch64");
        fs::copy(&extracted_keygen, &keygen_cached)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&keygen_cached, fs::Permissions::from_mode(0o755))?;
        }
    }

    // Cleanup
    let _ = fs::remove_dir_all(&extract_dir);
    let _ = fs::remove_file(&ipk_path);

    println!("  Saved to {}", cached.display().to_string().cyan());
    Ok(cached.display().to_string())
}

pub(crate) fn push_binary(adb: &AdbDevice, local_path: &str) -> Result<()> {
    println!(
        "  Pushing {} -> {}",
        local_path.cyan(),
        DROPBEAR_REMOTE.cyan()
    );
    adb.push(local_path, DROPBEAR_REMOTE)?;
    adb.shell(&format!("chmod +x {DROPBEAR_REMOTE}"), 5)?;
    println!("  Binary pushed and marked executable.");

    // Also push dropbearkey if available alongside the binary
    let keygen_cached = dirs_next().join("dropbearkey-aarch64");
    if keygen_cached.exists() {
        let remote = "/data/local/tmp/dropbearkey";
        println!("  Pushing dropbearkey -> {}", remote.cyan());
        adb.push(&keygen_cached.display().to_string(), remote)?;
        adb.shell(&format!("chmod +x {remote}"), 5)?;
    }

    Ok(())
}

pub(crate) fn generate_host_keys(adb: &AdbDevice, dropbear_path: &str) -> Result<()> {
    adb.shell(
        &format!("mkdir -p {DROPBEAR_KEY_DIR} && chmod 700 {DROPBEAR_KEY_DIR}"),
        5,
    )?;

    // Try to find dropbearkey (system or pushed)
    let keygen = adb
        .shell(
            "which dropbearkey 2>/dev/null || \
             (test -x /data/local/tmp/dropbearkey && echo /data/local/tmp/dropbearkey)",
            5,
        )
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(kg) = keygen {
        println!("  Generating host keys with {}...", kg.cyan());
        adb.shell(
            &format!("{kg} -t rsa -f {DROPBEAR_HOST_KEY} 2>/dev/null"),
            15,
        )?;
        let ed25519_key = format!("{DROPBEAR_KEY_DIR}/dropbear_ed25519_host_key");
        adb.shell(
            &format!("{kg} -t ed25519 -f {ed25519_key} 2>/dev/null"),
            15,
        )?;
    } else {
        println!("  Generating host keys via dropbear -R on first start...");
        adb.shell(&format!("{dropbear_path} -R 2>/dev/null || true"), 10)?;
    }
    Ok(())
}

pub(crate) fn start_dropbear(adb: &AdbDevice, dropbear_path: &str, port: u16) -> Result<()> {
    adb.shell("killall dropbear 2>/dev/null || true", 5)?;

    // Ensure /root is writable (bind mount from /data if root fs is read-only)
    setup_writable_root(adb);

    println!("  Starting dropbear on port {}...", port.to_string().cyan());
    adb.shell(
        &format!("{dropbear_path} -p 0.0.0.0:{port} -R -E"),
        10,
    )?;

    // Give dropbear a moment to fork
    std::thread::sleep(std::time::Duration::from_secs(1));

    let out = adb
        .shell("pidof dropbear 2>/dev/null || true", 5)
        .unwrap_or_default();
    let pid = out.trim();
    if !pid.is_empty() {
        println!("  Dropbear running (PID {pid}).");
    } else {
        println!(
            "  {}",
            "Warning: could not confirm dropbear is running.".yellow()
        );
    }
    Ok(())
}

pub(crate) fn setup_writable_root(adb: &AdbDevice) {
    // On ZTE devices, /root is on read-only rootfs.  Bind-mount a writable
    // directory from /data so that login shells get a proper HOME.
    let already = adb
        .shell("mount | grep 'on /root ' | grep -v /dev/root", 5)
        .unwrap_or_default();
    if !already.trim().is_empty() {
        return; // already mounted
    }
    let _ = adb.shell(
        "mkdir -p /data/local/tmp/root-home && \
         mount -o bind /data/local/tmp/root-home /root && \
         chmod 700 /root",
        5,
    );
}

pub(crate) fn create_init_script(adb: &AdbDevice, dropbear_path: &str, port: u16) -> Result<()> {
    let script = format!(
        "#!/bin/sh\\n\
         # Start dropbear SSH on boot\\n\
         mkdir -p {DROPBEAR_KEY_DIR}\\n\
         chmod 700 {DROPBEAR_KEY_DIR}\\n\
         # Bind-mount writable /root from /data\\n\
         mount | grep -q 'on /root ' || {{\\n\
           mkdir -p /data/local/tmp/root-home\\n\
           mount -o bind /data/local/tmp/root-home /root\\n\
           chmod 700 /root\\n\
         }}\\n\
         {dropbear_path} -p 0.0.0.0:{port} -R -E\\n"
    );
    adb.shell(
        &format!("echo -e \"{script}\" > {INIT_SCRIPT}"),
        5,
    )?;
    adb.shell(&format!("chmod +x {INIT_SCRIPT}"), 5)?;
    println!("  Init script written to {}", INIT_SCRIPT.cyan());

    // Try to hook into rc.local
    let check = adb
        .shell(
            &format!("grep -q '{INIT_SCRIPT}' /etc/rc.local 2>/dev/null && echo exists || true"),
            5,
        )
        .unwrap_or_default();
    if check.trim() == "exists" {
        println!("  rc.local already references the init script.");
    } else {
        // Insert before `exit 0` so the line is actually reached on boot.
        // If there's no `exit 0`, fall back to appending.
        let has_exit = adb
            .shell("grep -q '^exit 0' /etc/rc.local 2>/dev/null && echo yes || true", 5)
            .unwrap_or_default();
        let cmd = if has_exit.trim() == "yes" {
            format!(
                "sed -i '/^exit 0/i {INIT_SCRIPT} &' /etc/rc.local 2>&1 || echo READONLY"
            )
        } else {
            format!(
                "echo \"{INIT_SCRIPT} &\" >> /etc/rc.local 2>&1 || echo READONLY"
            )
        };
        let result = adb.shell(&cmd, 5).unwrap_or_default();
        if result.contains("READONLY") || result.contains("Read-only") {
            println!(
                "  {}",
                format!("Could not modify /etc/rc.local (read-only filesystem).").yellow()
            );
        } else {
            println!("  Added init script to {}", "/etc/rc.local".cyan());
        }
    }
    Ok(())
}

pub(crate) fn push_ssh_key(adb: &AdbDevice, key_path: &str) -> Result<()> {
    let key_path = shellexpand_tilde(key_path);
    let pubkey = fs::read_to_string(&key_path)
        .with_context(|| format!("SSH public key not found: {key_path}"))?;
    let pubkey = pubkey.trim();
    println!("  Pushing public key to {}...", AUTH_KEYS_PATH.cyan());

    // OpenWrt dropbear reads keys from /etc/dropbear/authorized_keys
    adb.shell(
        &format!("mkdir -p {DROPBEAR_KEY_DIR} && chmod 700 {DROPBEAR_KEY_DIR}"),
        5,
    )?;
    let existing = adb
        .shell(&format!("cat {AUTH_KEYS_PATH} 2>/dev/null || true"), 5)
        .unwrap_or_default();
    if existing.contains(pubkey) {
        println!("  Key already present on device.");
    } else {
        let escaped = pubkey.replace('"', "\\\"");
        adb.shell(
            &format!("echo \"{escaped}\" >> {AUTH_KEYS_PATH}"),
            5,
        )?;
        adb.shell(&format!("chmod 600 {AUTH_KEYS_PATH}"), 5)?;
        println!("  Public key installed.");
    }
    Ok(())
}

pub(crate) fn verify_ssh(adb: &AdbDevice, port: u16) {
    println!("  Verifying SSH connectivity...");
    let ip = get_device_ip(adb).unwrap_or_else(|| "192.168.0.1".to_string());
    let target = format!("root@{ip}");
    let result = Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "ConnectTimeout=5",
            "-o", "BatchMode=yes",
            "-p", &port.to_string(),
            &target,
            "echo", "ssh_ok",
        ])
        .output();
    match result {
        Ok(output) if String::from_utf8_lossy(&output.stdout).contains("ssh_ok") => {
            println!("  {}", "SSH connection verified!".bold().green());
        }
        _ => {
            println!(
                "  {}",
                "Could not verify SSH automatically (try manually).".yellow()
            );
        }
    }
}

pub(crate) fn get_device_ip(adb: &AdbDevice) -> Option<String> {
    let out = adb
        .shell("ip route get 1.1.1.1 2>/dev/null | head -1", 5)
        .ok()?;
    if out.contains("src") {
        let parts: Vec<&str> = out.split("src").collect();
        if parts.len() >= 2 {
            let ip = parts[1].trim().split_whitespace().next()?;
            return Some(ip.to_string());
        }
    }
    for iface in ["br0", "br-lan", "wlan0", "eth0"] {
        if let Ok(out) = adb.shell(&format!("ip addr show {iface} 2>/dev/null"), 5) {
            if out.contains("inet ") {
                for token in out.split_whitespace() {
                    if token.contains('/') && token.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        return Some(token.split('/').next()?.to_string());
                    }
                }
            }
        }
    }
    None
}

fn shellexpand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    }
}
