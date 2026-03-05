use std::path::Path;

use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::Table;

use super::{confirm_guard, get_transport, Transport, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show current display settings (brightness, timeout, wakeup, night mode, panel status)
    Info(TransportArgs),
    /// Set brightness level (0-5)
    Brightness {
        #[command(flatten)]
        transport: TransportArgs,
        /// Brightness level 0-5 (0 = dimmest, 5 = brightest)
        level: u8,
        #[arg(long)]
        confirm: bool,
    },
    /// Set screen timeout interval in seconds
    Timeout {
        #[command(flatten)]
        transport: TransportArgs,
        /// Timeout in seconds (0 = never)
        seconds: u32,
        #[arg(long)]
        confirm: bool,
    },
    /// Toggle periodic screen wakeup
    Wakeup {
        #[command(flatten)]
        transport: TransportArgs,
        /// on or off
        state: String,
        /// Wakeup interval in seconds
        #[arg(long)]
        interval: Option<u32>,
        /// How long the screen stays on during wakeup, in seconds
        #[arg(long)]
        duration: Option<u32>,
        #[arg(long)]
        confirm: bool,
    },
    /// Toggle LED night mode
    NightMode {
        #[command(flatten)]
        transport: TransportArgs,
        /// on or off
        state: String,
        /// Start time (HH:MM)
        #[arg(long)]
        start: Option<String>,
        /// End time (HH:MM)
        #[arg(long)]
        end: Option<String>,
        #[arg(long)]
        confirm: bool,
    },
    /// Turn screen on immediately (set backlight to max)
    On(TransportArgs),
    /// Turn screen off immediately (set backlight to 0)
    Off(TransportArgs),
    /// Capture the display to a local PNG file
    Screenshot {
        #[command(flatten)]
        transport: TransportArgs,
        /// Output file path
        #[arg(long, short, default_value = "screenshot.png")]
        output: String,
    },
    /// Push a PNG file to the device UI directory (/usr/ui/)
    PushAsset {
        #[command(flatten)]
        transport: TransportArgs,
        /// Local PNG file to push
        file: String,
        /// Target path relative to /usr/ui/ (e.g. skin/signal0.png)
        target: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Override a UI text string in a language INI file
    SetText {
        #[command(flatten)]
        transport: TransportArgs,
        /// INI key name (e.g. TUFormIDLE_buttonText_Settings)
        key: String,
        /// New text value
        value: String,
        /// Language file to modify
        #[arg(long, default_value = "English")]
        lang: String,
        #[arg(long)]
        confirm: bool,
    },
    /// List UI assets on device under /usr/ui/
    ListAssets {
        #[command(flatten)]
        transport: TransportArgs,
        /// Directory under /usr/ui/ to list (skin, anim, fonts, language)
        #[arg(default_value = "skin")]
        dir: String,
    },
    /// Restore a previously backed-up asset from /usr/ui/<target>.bak
    RestoreAsset {
        #[command(flatten)]
        transport: TransportArgs,
        /// Target path relative to /usr/ui/ (e.g. skin/signal0.png)
        target: String,
        #[arg(long)]
        confirm: bool,
    },
}

/// Brightness level (0–5) mapped to sysfs value (0–255).
fn level_to_sysfs(level: u8) -> u8 {
    match level {
        0 => 0,
        1 => 51,
        2 => 102,
        3 => 153,
        4 => 204,
        5 => 255,
        _ => 255,
    }
}

/// Map sysfs brightness (0–255) back to level (0–5).
fn sysfs_to_level(sysfs: u8) -> u8 {
    match sysfs {
        0 => 0,
        1..=51 => 1,
        52..=102 => 2,
        103..=153 => 3,
        154..=204 => 4,
        _ => 5,
    }
}

fn remount_rw(t: &Transport) -> Result<()> {
    t.shell("mount -o remount,rw /")?;
    Ok(())
}

fn remount_ro(t: &Transport) -> Result<()> {
    t.shell("mount -o remount,ro /")?;
    Ok(())
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Info(args) => info(args),
        Cmd::Brightness { transport, level, confirm } => brightness(transport, level, confirm),
        Cmd::Timeout { transport, seconds, confirm } => timeout(transport, seconds, confirm),
        Cmd::Wakeup { transport, state, interval, duration, confirm } => {
            wakeup(transport, state, interval, duration, confirm)
        }
        Cmd::NightMode { transport, state, start, end, confirm } => {
            night_mode(transport, state, start, end, confirm)
        }
        Cmd::On(args) => screen_on(args),
        Cmd::Off(args) => screen_off(args),
        Cmd::Screenshot { transport, output } => screenshot(transport, output),
        Cmd::PushAsset { transport, file, target, confirm } => {
            push_asset(transport, file, target, confirm)
        }
        Cmd::SetText { transport, key, value, lang, confirm } => {
            set_text(transport, key, value, lang, confirm)
        }
        Cmd::ListAssets { transport, dir } => list_assets(transport, dir),
        Cmd::RestoreAsset { transport, target, confirm } => {
            restore_asset(transport, target, confirm)
        }
    }
}

fn info(args: TransportArgs) -> Result<()> {
    let t = get_transport(&args)?;

    // Read UCI display settings
    let uci_brightness = t.shell("uci get zwrt_deviceui.Screen.brightness 2>/dev/null")?;
    let uci_timeout = t.shell("uci get zwrt_deviceui.Screen.screen_show_interval 2>/dev/null")?;
    let uci_wakeup = t.shell("uci get zwrt_deviceui.Screen.screen_wakeup 2>/dev/null")?;
    let uci_wakeup_interval =
        t.shell("uci get zwrt_deviceui.Screen.screen_wakeup_interval 2>/dev/null")?;
    let uci_wakeup_show =
        t.shell("uci get zwrt_deviceui.Screen.screen_wakeup_show_time 2>/dev/null")?;
    let uci_pin_enabled =
        t.shell("uci get zwrt_deviceui.Screen.Lock_Screen_PIN_Enable_State 2>/dev/null")?;

    // Read night mode settings
    let night_switch = t.shell("uci get zwrt_led.nightmode.switch 2>/dev/null")?;
    let night_start = t.shell("uci get zwrt_led.nightmode.starttime 2>/dev/null")?;
    let night_end = t.shell("uci get zwrt_led.nightmode.endtime 2>/dev/null")?;
    let night_active = t.shell("uci get zwrt_led.nightmode.is_nightmode 2>/dev/null")?;

    // Read live sysfs values
    let sysfs_brightness = t.shell("cat /sys/class/leds/led:lcd/brightness 2>/dev/null")?;
    let panel_on = t.shell("cat /sys/kernel/debug/qpic_display/is_panel_on 2>/dev/null")?;

    let trim = |s: String| s.trim().to_string();

    let mut table = Table::new();
    table.set_header(vec!["Display Settings", ""]);

    table.add_row(vec!["Brightness (UCI level)", &trim(uci_brightness)]);
    table.add_row(vec!["Brightness (sysfs raw)", &trim(sysfs_brightness.clone())]);

    // Show mapped level from sysfs
    if let Ok(raw) = trim(sysfs_brightness).parse::<u8>() {
        table.add_row(vec![
            "Brightness (actual level)",
            &format!("{}", sysfs_to_level(raw)),
        ]);
    }

    table.add_row(vec!["Screen timeout (sec)", &trim(uci_timeout)]);
    table.add_row(vec![
        "Panel on",
        &trim(panel_on).replace('1', "yes").replace('0', "no"),
    ]);
    table.add_row(vec![
        "PIN lock enabled",
        &trim(uci_pin_enabled)
            .replace('1', "yes")
            .replace('0', "no"),
    ]);

    println!("{table}");

    // Wakeup
    let mut wakeup_table = Table::new();
    wakeup_table.set_header(vec!["Periodic Wakeup", ""]);
    wakeup_table.add_row(vec![
        "Enabled",
        &trim(uci_wakeup).replace('1', "yes").replace('0', "no"),
    ]);
    wakeup_table.add_row(vec!["Interval (sec)", &trim(uci_wakeup_interval)]);
    wakeup_table.add_row(vec!["Show duration (sec)", &trim(uci_wakeup_show)]);
    println!("{wakeup_table}");

    // Night mode
    let mut night_table = Table::new();
    night_table.set_header(vec!["Night Mode", ""]);
    night_table.add_row(vec![
        "Enabled",
        &trim(night_switch).replace('1', "yes").replace('0', "no"),
    ]);
    night_table.add_row(vec!["Start time", &trim(night_start)]);
    night_table.add_row(vec!["End time", &trim(night_end)]);
    night_table.add_row(vec![
        "Currently active",
        &trim(night_active).replace('1', "yes").replace('0', "no"),
    ]);
    println!("{night_table}");

    Ok(())
}

fn brightness(args: TransportArgs, level: u8, confirm: bool) -> Result<()> {
    if level > 5 {
        anyhow::bail!("Brightness level must be 0-5");
    }
    confirm_guard(confirm, &format!("set brightness to {level}"))?;

    let t = get_transport(&args)?;
    let sysfs_val = level_to_sysfs(level);

    // Set UCI value (persistent)
    t.shell(&format!(
        "uci set zwrt_deviceui.Screen.brightness={level} && uci commit zwrt_deviceui"
    ))?;

    // Set sysfs value (immediate)
    t.shell(&format!(
        "echo {sysfs_val} > /sys/class/leds/led:lcd/brightness"
    ))?;

    println!(
        "{}",
        format!("Brightness set to level {level} (sysfs={sysfs_val}).").green()
    );
    Ok(())
}

fn timeout(args: TransportArgs, seconds: u32, confirm: bool) -> Result<()> {
    confirm_guard(confirm, &format!("set screen timeout to {seconds}s"))?;

    let t = get_transport(&args)?;
    t.shell(&format!(
        "uci set zwrt_deviceui.Screen.screen_show_interval={seconds} && uci commit zwrt_deviceui"
    ))?;

    println!(
        "{}",
        format!("Screen timeout set to {seconds} seconds.").green()
    );
    Ok(())
}

fn wakeup(
    args: TransportArgs,
    state: String,
    interval: Option<u32>,
    duration: Option<u32>,
    confirm: bool,
) -> Result<()> {
    let on = match state.as_str() {
        "on" | "1" | "true" => true,
        "off" | "0" | "false" => false,
        _ => anyhow::bail!("State must be 'on' or 'off'"),
    };

    confirm_guard(
        confirm,
        &format!("set periodic wakeup to {}", if on { "on" } else { "off" }),
    )?;

    let t = get_transport(&args)?;
    let val = if on { "1" } else { "0" };
    let mut cmds = format!("uci set zwrt_deviceui.Screen.screen_wakeup={val}");
    if let Some(i) = interval {
        cmds.push_str(&format!(
            " && uci set zwrt_deviceui.Screen.screen_wakeup_interval={i}"
        ));
    }
    if let Some(d) = duration {
        cmds.push_str(&format!(
            " && uci set zwrt_deviceui.Screen.screen_wakeup_show_time={d}"
        ));
    }
    cmds.push_str(" && uci commit zwrt_deviceui");
    t.shell(&cmds)?;

    let mut msg = format!("Periodic wakeup {}.", if on { "enabled" } else { "disabled" });
    if let Some(i) = interval {
        msg.push_str(&format!(" Interval: {i}s."));
    }
    if let Some(d) = duration {
        msg.push_str(&format!(" Duration: {d}s."));
    }
    println!("{}", msg.green());
    Ok(())
}

fn night_mode(
    args: TransportArgs,
    state: String,
    start: Option<String>,
    end: Option<String>,
    confirm: bool,
) -> Result<()> {
    let on = match state.as_str() {
        "on" | "1" | "true" => true,
        "off" | "0" | "false" => false,
        _ => anyhow::bail!("State must be 'on' or 'off'"),
    };

    confirm_guard(
        confirm,
        &format!(
            "set night mode to {}",
            if on { "on" } else { "off" }
        ),
    )?;

    let t = get_transport(&args)?;
    let val = if on { "1" } else { "0" };
    let mut cmds = format!("uci set zwrt_led.nightmode.switch={val}");
    if let Some(ref s) = start {
        cmds.push_str(&format!(" && uci set zwrt_led.nightmode.starttime={s}"));
    }
    if let Some(ref e) = end {
        cmds.push_str(&format!(" && uci set zwrt_led.nightmode.endtime={e}"));
    }
    cmds.push_str(" && uci commit zwrt_led");
    t.shell(&cmds)?;

    let mut msg = format!(
        "Night mode {}.",
        if on { "enabled" } else { "disabled" }
    );
    if let Some(s) = start {
        msg.push_str(&format!(" Start: {s}."));
    }
    if let Some(e) = end {
        msg.push_str(&format!(" End: {e}."));
    }
    println!("{}", msg.green());
    Ok(())
}

fn screen_on(args: TransportArgs) -> Result<()> {
    let t = get_transport(&args)?;

    // Read current UCI brightness to determine the right sysfs value
    let uci_level = t
        .shell("uci get zwrt_deviceui.Screen.brightness 2>/dev/null")?
        .trim()
        .parse::<u8>()
        .unwrap_or(5);
    let sysfs_val = level_to_sysfs(uci_level);

    t.shell(&format!(
        "echo {sysfs_val} > /sys/class/leds/led:lcd/brightness"
    ))?;

    println!(
        "{}",
        format!("Screen on (brightness level {uci_level}, sysfs={sysfs_val}).").green()
    );
    Ok(())
}

fn screen_off(args: TransportArgs) -> Result<()> {
    let t = get_transport(&args)?;
    t.shell("echo 0 > /sys/class/leds/led:lcd/brightness")?;
    println!("{}", "Screen off.".green());
    Ok(())
}

// ---------------------------------------------------------------------------
// Display content commands
// ---------------------------------------------------------------------------

fn screenshot(args: TransportArgs, output: String) -> Result<()> {
    let t = get_transport(&args)?;

    // Try SIGUSR1 to zte_topsw_devui first
    let pid = t.shell("pidof zte_topsw_devui 2>/dev/null")?.trim().to_string();
    let mut got_screenshot = false;

    if !pid.is_empty() {
        // Try SIGUSR1
        println!("Sending SIGUSR1 to zte_topsw_devui (pid {pid})...");
        let _ = t.shell(&format!("kill -USR1 {pid}"));
        std::thread::sleep(std::time::Duration::from_secs(2));

        let check = t.shell("test -f /cache/fb.png && echo exists")?.trim().to_string();
        if check == "exists" {
            got_screenshot = true;
        } else {
            // Try SIGUSR2
            println!("SIGUSR1 didn't produce /cache/fb.png, trying SIGUSR2...");
            let _ = t.shell(&format!("kill -USR2 {pid}"));
            std::thread::sleep(std::time::Duration::from_secs(2));

            let check = t.shell("test -f /cache/fb.png && echo exists")?.trim().to_string();
            if check == "exists" {
                got_screenshot = true;
            }
        }
    }

    if got_screenshot {
        t.0.pull("/cache/fb.png", &output)?;
        let _ = t.shell("rm -f /cache/fb.png");
        println!("{}", format!("Screenshot saved to {output}").green());
    } else {
        // Fallback: raw framebuffer dump
        println!("No signal-based screenshot available, dumping raw framebuffer...");
        // 320x480 RGB565 = 307200 bytes
        t.shell(
            "dd if=/dev/fb0 of=/tmp/fb.raw bs=307200 count=1 2>/dev/null || \
             cat /dev/graphics/fb0 > /tmp/fb.raw 2>/dev/null || \
             echo 'fb_fail'"
        )?;

        let check = t.shell("test -s /tmp/fb.raw && echo exists")?.trim().to_string();
        if check == "exists" {
            let raw_output = output.replace(".png", ".raw");
            t.0.pull("/tmp/fb.raw", &raw_output)?;
            let _ = t.shell("rm -f /tmp/fb.raw");
            println!(
                "{}",
                format!("Raw framebuffer saved to {raw_output} (RGB565 320x480)").yellow()
            );
            println!(
                "{}",
                "Convert with: ffmpeg -f rawvideo -pix_fmt rgb565le -s 320x480 -i fb.raw screenshot.png"
                    .dimmed()
            );
        } else {
            anyhow::bail!("Could not capture screenshot via signal or framebuffer");
        }
    }

    Ok(())
}

fn push_asset(args: TransportArgs, file: String, target: String, confirm: bool) -> Result<()> {
    // Validate local file exists and is PNG
    let path = Path::new(&file);
    if !path.exists() {
        anyhow::bail!("Local file not found: {file}");
    }
    let header = std::fs::read(path).map(|b| b.get(..4).map(|s| s.to_vec()))?;
    if header.as_deref() != Some(&[0x89, 0x50, 0x4E, 0x47]) {
        anyhow::bail!("File does not appear to be a PNG (bad magic bytes)");
    }

    // Sanitize target to prevent path traversal
    if target.contains("..") {
        anyhow::bail!("Target path must not contain '..'");
    }

    let remote_path = format!("/usr/ui/{target}");
    confirm_guard(confirm, &format!("push {file} -> {remote_path}"))?;

    let t = get_transport(&args)?;

    remount_rw(&t)?;

    // Back up original if no backup exists yet
    let has_bak = t
        .shell(&format!("test -f {remote_path}.bak && echo exists"))?
        .trim()
        .to_string();
    if has_bak != "exists" {
        let has_orig = t
            .shell(&format!("test -f {remote_path} && echo exists"))?
            .trim()
            .to_string();
        if has_orig == "exists" {
            t.shell(&format!("cp {remote_path} {remote_path}.bak"))?;
            println!("Backed up original to {remote_path}.bak");
        }
    }

    t.0.push(&file, &remote_path)?;

    remount_ro(&t)?;

    println!("{}", format!("Pushed {file} -> {remote_path}").green());
    println!(
        "{}",
        "Restart the UI daemon for changes to take effect: killall zte_topsw_devui".dimmed()
    );
    Ok(())
}

fn set_text(
    args: TransportArgs,
    key: String,
    value: String,
    lang: String,
    confirm: bool,
) -> Result<()> {
    // Sanitize inputs for safe sed usage
    if key.contains('/') || key.contains('\'') || key.contains('\n') {
        anyhow::bail!("Key must not contain /, ', or newlines");
    }
    if value.contains('\'') || value.contains('\n') {
        anyhow::bail!("Value must not contain ' or newlines");
    }
    if lang.contains("..") || lang.contains('/') {
        anyhow::bail!("Language name must not contain '..' or '/'");
    }

    let ini_path = format!("/usr/ui/language/{lang}.ini");
    confirm_guard(confirm, &format!("set {key}=\"{value}\" in {ini_path}"))?;

    let t = get_transport(&args)?;

    // Check the language file exists
    let exists = t
        .shell(&format!("test -f {ini_path} && echo exists"))?
        .trim()
        .to_string();
    if exists != "exists" {
        anyhow::bail!("Language file not found: {ini_path}");
    }

    remount_rw(&t)?;

    // Back up original if no backup exists yet
    let has_bak = t
        .shell(&format!("test -f {ini_path}.bak && echo exists"))?
        .trim()
        .to_string();
    if has_bak != "exists" {
        t.shell(&format!("cp {ini_path} {ini_path}.bak"))?;
        println!("Backed up original to {ini_path}.bak");
    }

    // Use sed to replace the key's value
    t.shell(&format!(
        "sed -i 's/^{key}=.*/{key}=\"{value}\"/' {ini_path}"
    ))?;

    remount_ro(&t)?;

    println!(
        "{}",
        format!("Set {key}=\"{value}\" in {ini_path}").green()
    );
    println!(
        "{}",
        "Restart the UI daemon for changes to take effect: killall zte_topsw_devui".dimmed()
    );
    Ok(())
}

fn list_assets(args: TransportArgs, dir: String) -> Result<()> {
    if dir.contains("..") {
        anyhow::bail!("Directory must not contain '..'");
    }

    let t = get_transport(&args)?;
    let remote_dir = format!("/usr/ui/{dir}");

    let output = t.shell(&format!("ls -la {remote_dir}/ 2>&1"))?;
    println!("{}", format!("Contents of {remote_dir}/").bold());
    println!("{output}");
    Ok(())
}

fn restore_asset(args: TransportArgs, target: String, confirm: bool) -> Result<()> {
    if target.contains("..") {
        anyhow::bail!("Target path must not contain '..'");
    }

    let remote_path = format!("/usr/ui/{target}");
    let bak_path = format!("{remote_path}.bak");
    confirm_guard(confirm, &format!("restore {bak_path} -> {remote_path}"))?;

    let t = get_transport(&args)?;

    let has_bak = t
        .shell(&format!("test -f {bak_path} && echo exists"))?
        .trim()
        .to_string();
    if has_bak != "exists" {
        anyhow::bail!("No backup found at {bak_path}");
    }

    remount_rw(&t)?;
    t.shell(&format!("cp {bak_path} {remote_path}"))?;
    remount_ro(&t)?;

    println!("{}", format!("Restored {bak_path} -> {remote_path}").green());
    println!(
        "{}",
        "Restart the UI daemon for changes to take effect: killall zte_topsw_devui".dimmed()
    );
    Ok(())
}
