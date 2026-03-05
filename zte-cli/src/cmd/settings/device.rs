use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use anyhow::Result;
use chrono::Local;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::{Cell, Color, Table};
use serde_json::json;

use super::{confirm_guard, get_transport, print_kv, TransportArgs};

const CHARGE_POLICIES: &[(u8, u8, &str)] = &[
    (0, 20, "Extreme conservation"),
    (10, 40, "Low maintenance"),
    (39, 60, "Default"),
    (40, 77, "Extended"),
    (60, 90, "High"),
    (80, 100, "Maximum"),
];

#[derive(Subcommand)]
pub enum Cmd {
    /// Show device info (model, firmware, IMEI, SIM)
    Info(TransportArgs),
    /// Reboot the device
    Reboot {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Factory reset the device
    FactoryReset {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Set USB mode (debug/normal)
    Usb {
        #[command(flatten)]
        transport: TransportArgs,
        /// debug or normal
        mode: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Show airplane mode status (ONLINE or LPM)
    AirplaneStatus(TransportArgs),
    /// Enable airplane mode (turn off cellular radio)
    AirplaneOn {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Disable airplane mode (restore cellular radio)
    AirplaneOff {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Show fast-boot status
    FastBoot(TransportArgs),
    /// Enable fast-boot (suspend-to-RAM instead of full shutdown)
    FastBootOn {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Disable fast-boot (use full shutdown)
    FastBootOff {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Show battery and thermal status
    Battery(TransportArgs),
    /// Show charge limit policy and wall mode status
    ChargeLimit(TransportArgs),
    /// Set charge limit policy mode (0-5)
    ChargeLimitSet {
        #[command(flatten)]
        transport: TransportArgs,
        /// Policy mode 0-5 (0=0-20%, 1=10-40%, 2=39-60%, 3=40-77%, 4=60-90%, 5=80-100%)
        mode: u8,
        #[arg(long)]
        confirm: bool,
    },
    /// Log battery status to CSV for charge policy verification
    BatteryLog {
        #[command(flatten)]
        transport: TransportArgs,
        /// Log file path
        #[arg(long, default_value = "logs/charge-policy.csv")]
        output: String,
    },
    /// Toggle wall mode (direct power supply)
    WallMode {
        #[command(flatten)]
        transport: TransportArgs,
        /// on or off
        state: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Analyze battery log CSV to verify wall mode / charge policy effectiveness
    BatteryLogAnalyze {
        /// Log file path to analyze
        #[arg(long, default_value = "logs/charge-policy.csv")]
        input: String,
    },
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Info(args) => {
            let t = get_transport(&args)?;
            let info = t.ubus_call("zwrt_bsp.info", "get_device_info", None);
            print_kv(
                &info,
                &[
                    "model", "firmware_version", "hardware_version",
                    "imei", "sim_status", "sim_iccid", "mac_addr",
                ],
                Some("Device Info"),
            );
            Ok(())
        }
        Cmd::Reboot { transport, confirm } => {
            confirm_guard(confirm, "reboot the device")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_bsp.power", "reboot", None);
            println!("{}", "Device is rebooting...".green());
            Ok(())
        }
        Cmd::FactoryReset { transport, confirm } => {
            confirm_guard(confirm, "factory reset the device")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_bsp.power", "factory_reset", None);
            println!("{}", "Factory reset initiated. Device will reboot.".green());
            Ok(())
        }
        Cmd::Usb { transport, mode, confirm } => {
            confirm_guard(confirm, &format!("set USB mode to {mode}"))?;
            let t = get_transport(&transport)?;
            t.ubus_call("zwrt_bsp.usb", "set", Some(&json!({"mode": mode})));
            println!("{}", format!("USB mode set to {mode}.").green());
            Ok(())
        }
        Cmd::AirplaneStatus(args) => {
            let t = get_transport(&args)?;
            let comp = t.ubus_call("zte-companion", "modem_status", None);
            let mode = comp.get("operate_mode").and_then(|v| v.as_str()).map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let nw = t.ubus_call("zte_nwinfo_api", "nwinfo_get_netinfo", None);
                    nw.get("operate_mode").and_then(|v| v.as_str())
                        .unwrap_or("unknown").to_string()
                });
            if mode == "ONLINE" {
                println!("{}", "Airplane mode: OFF (radio ONLINE)".green());
            } else if mode == "LPM" || mode == "LOW_POWER" {
                println!("{}", "Airplane mode: ON (radio in LPM)".yellow());
            } else {
                println!("Airplane mode: {} (operate_mode={})", "unknown".yellow(), mode);
            }
            Ok(())
        }
        Cmd::AirplaneOn { transport, confirm } => {
            confirm_guard(confirm, "enable airplane mode (cellular radio off)")?;
            let t = get_transport(&transport)?;
            t.ubus_call(
                "zte_nwinfo_api",
                "nwinfo_set_mode",
                Some(&json!({"operate_mode": "LPM"})),
            );
            println!("{}", "Airplane mode enabled — cellular radio is OFF.".green());
            Ok(())
        }
        Cmd::AirplaneOff { transport, confirm } => {
            confirm_guard(confirm, "disable airplane mode (cellular radio on)")?;
            let t = get_transport(&transport)?;
            // Try companion modem_online with retry (matches iOS behavior)
            let mut succeeded = false;
            for attempt in 1..=2 {
                let result = t.ubus_call("zte-companion", "modem_online", None);
                if result.get("error").is_none() && !result.is_null() {
                    succeeded = true;
                    break;
                }
                if attempt == 1 {
                    println!("First attempt failed, retrying in 3s...");
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
            }
            if succeeded {
                println!("{}", "Airplane mode disabled — signal recovering...".green());
            } else {
                println!("{}", "Companion modem_online failed. Rebooting device (firmware bug workaround)...".yellow());
                t.ubus_call("zwrt_bsp.power", "reboot", None);
                println!("{}", "Device is rebooting...".green());
            }
            Ok(())
        }
        Cmd::FastBoot(args) => {
            let t = get_transport(&args)?;
            let info = t.ubus_call(
                "zwrt_mc.device.manager",
                "get_device_info",
                Some(&json!({"deviceInfoList": ["quicken_power_on"]})),
            );
            let val = info.get("quicken_power_on").and_then(|v| v.as_str()).unwrap_or("0");
            if val == "1" {
                println!("{}", "Fast boot: enabled (suspend-to-RAM)".green());
            } else {
                println!("{}", "Fast boot: disabled (full shutdown)".yellow());
            }
            Ok(())
        }
        Cmd::FastBootOn { transport, confirm } => {
            confirm_guard(confirm, "enable fast boot (suspend-to-RAM instead of full shutdown)")?;
            let t = get_transport(&transport)?;
            t.ubus_call(
                "zwrt_mc.device.manager",
                "set_device_info",
                Some(&json!({"deviceInfoList": {"quicken_power_on": "1"}})),
            );
            println!("{}", "Fast boot enabled.".green());
            Ok(())
        }
        Cmd::FastBootOff { transport, confirm } => {
            confirm_guard(confirm, "disable fast boot (use full shutdown)")?;
            let t = get_transport(&transport)?;
            t.ubus_call(
                "zwrt_mc.device.manager",
                "set_device_info",
                Some(&json!({"deviceInfoList": {"quicken_power_on": "0"}})),
            );
            println!("{}", "Fast boot disabled.".green());
            Ok(())
        }
        Cmd::Battery(args) => {
            let t = get_transport(&args)?;
            let bat = t.ubus_call("zwrt_bsp.battery", "list", None);
            print_kv(
                &bat,
                &[
                    "battery_capacity", "battery_temperature",
                    "battery_voltage", "battery_mode",
                    "battery_time_to_full", "battery_time_to_empty",
                    "charge_status", "charge_type",
                ],
                Some("Battery Status"),
            );
            let thermal = t.ubus_call("zwrt_bsp.thermal", "get_cpu_temp", None);
            print_kv(&thermal, &["cpuss_temp"], Some("Thermal"));
            Ok(())
        }
        Cmd::ChargeLimit(args) => charge_limit(args),
        Cmd::ChargeLimitSet { transport, mode, confirm } => {
            charge_limit_set(transport, mode, confirm)
        }
        Cmd::BatteryLog { transport, output } => battery_log(transport, output),
        Cmd::WallMode { transport, state, confirm } => wall_mode(transport, state, confirm),
        Cmd::BatteryLogAnalyze { input } => battery_log_analyze(input),
    }
}

fn battery_log(args: TransportArgs, output: String) -> Result<()> {
    let t = get_transport(&args)?;

    let mode_str = t
        .shell("cat /sys/class/power_supply/battery_zte/ui_chg_policy_mode 2>/dev/null")?;
    let policy_mode = mode_str.trim().to_string();

    let bat = t.ubus_call("zwrt_bsp.battery", "list", None);
    let charger = t.ubus_call("zwrt_bsp.charger", "list", None);
    let current = t.ubus_call("zte-companion", "battery_current", None);

    let val = |data: &serde_json::Value, key: &str| -> String {
        data.get(key)
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    };

    let capacity = val(&bat, "battery_capacity");
    let temperature = val(&bat, "battery_temperature");
    let time_to_empty = val(&bat, "battery_time_to_empty");
    let charge_raw = val(&charger, "charge_status");
    let charge_status = match charge_raw.as_str() {
        "1" => "Charging",
        "2" => "Discharging",
        "3" => "Not charging",
        "4" => "Full",
        other => other,
    };
    let wall_mode = val(&charger, "direct_power_supply_mode");
    let charger_connected = val(&charger, "charger_connect");

    // Current/voltage/power from zte-companion battery_current (μA/μV → mA/mV)
    let current_ua: i64 = current.get("current_now")
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0);
    let voltage_uv: i64 = current.get("voltage_now")
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        // Fallback to battery_voltage from bat response (already in mV)
        .unwrap_or_else(|| {
            bat.get("battery_voltage")
                .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                .map(|mv| mv * 1000) // convert mV back to μV for uniform handling
                .unwrap_or(0)
        });
    let current_ma = current_ua / 1000;
    let voltage_mv = voltage_uv / 1000;
    let power_mw = (current_ma * voltage_mv) / 1000;

    let current_ma_str = current_ma.to_string();
    let voltage_mv_str = voltage_mv.to_string();
    let power_mw_str = power_mw.to_string();

    let timestamp = Local::now().to_rfc3339();

    // Write CSV
    let path = Path::new(&output);
    let needs_header = !path.exists();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    if needs_header {
        writeln!(file, "timestamp,capacity,temperature,charge_status,wall_mode,policy_mode,charger,time_to_empty,current_ma,voltage_mv,power_mw")?;
    }
    writeln!(
        file,
        "{timestamp},{capacity},{temperature},{charge_status},{wall_mode},{policy_mode},{charger_connected},{time_to_empty},{current_ma_str},{voltage_mv_str},{power_mw_str}"
    )?;

    // Print to stdout
    let mut table = Table::new();
    table.set_header(vec!["Battery Log", ""]);
    table.add_row(vec!["Timestamp", &timestamp]);
    table.add_row(vec!["Capacity", &format!("{capacity}%")]);
    table.add_row(vec!["Temperature", &format!("{temperature}°C")]);
    table.add_row(vec!["Charge status", charge_status]);
    table.add_row(vec!["Wall mode", &wall_mode]);
    table.add_row(vec!["Policy mode", &policy_mode]);
    table.add_row(vec!["Charger", &charger_connected]);
    table.add_row(vec!["Time to empty", &format!("{time_to_empty} min")]);
    table.add_row(vec!["Current", &format!("{current_ma_str} mA")]);
    table.add_row(vec!["Voltage", &format!("{voltage_mv_str} mV")]);
    table.add_row(vec!["Power", &format!("{power_mw_str} mW")]);
    println!("{table}");
    println!("{}", format!("Logged to {output}").green());

    Ok(())
}

fn charge_limit(args: TransportArgs) -> Result<()> {
    let t = get_transport(&args)?;

    let mode_str = t
        .shell("cat /sys/class/power_supply/battery_zte/ui_chg_policy_mode 2>/dev/null")?;
    let mode: usize = mode_str.trim().parse().unwrap_or(0);

    let bat = t.ubus_call("zwrt_bsp.battery", "list", None);
    let charger = t.ubus_call("zwrt_bsp.charger", "list", None);

    let val_str = |data: &serde_json::Value, key: &str| -> String {
        data.get(key)
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                other => other.to_string(),
            })
            .unwrap_or_else(|| "--".into())
    };
    let capacity = val_str(&bat, "battery_capacity");
    let charge_raw = val_str(&charger, "charge_status");
    let charge_status = match charge_raw.as_str() {
        "1" => "Charging".to_string(),
        "2" => "Discharging".to_string(),
        "3" => "Not charging".to_string(),
        "4" => "Full".to_string(),
        other => other.to_string(),
    };
    let wall_mode = val_str(&charger, "direct_power_supply_mode");

    let (start, stop, desc) = if mode < CHARGE_POLICIES.len() {
        let p = &CHARGE_POLICIES[mode];
        (p.0, p.1, p.2)
    } else {
        (0, 0, "Unknown")
    };

    let mut table = Table::new();
    table.set_header(vec!["Charge Policy", ""]);
    table.add_row(vec!["Policy mode", &format!("{mode}")]);
    table.add_row(vec!["Target range", &format!("{start}-{stop}%")]);
    table.add_row(vec!["Description", desc]);
    table.add_row(vec!["Battery", &format!("{capacity}%")]);
    table.add_row(vec!["Charging", &charge_status]);
    table.add_row(vec!["Wall mode", &wall_mode]);
    println!("{table}");

    // Show all available policies
    let mut policy_table = Table::new();
    policy_table.set_header(vec!["Mode", "Range", "Description"]);
    for (i, (start, stop, desc)) in CHARGE_POLICIES.iter().enumerate() {
        let marker = if i == mode { " <--" } else { "" };
        policy_table.add_row(vec![
            &format!("{i}"),
            &format!("{start}-{stop}%"),
            &format!("{desc}{marker}"),
        ]);
    }
    println!("{policy_table}");

    Ok(())
}

fn charge_limit_set(args: TransportArgs, mode: u8, confirm: bool) -> Result<()> {
    if mode > 5 {
        anyhow::bail!("Policy mode must be 0-5");
    }
    let (start, stop, desc) = {
        let p = &CHARGE_POLICIES[mode as usize];
        (p.0, p.1, p.2)
    };

    confirm_guard(confirm, &format!("set charge policy to mode {mode} ({desc}, {start}-{stop}%)"))?;

    let t = get_transport(&args)?;
    t.shell(&format!(
        "echo {mode} > /sys/class/power_supply/battery_zte/ui_chg_policy_mode"
    ))?;

    println!(
        "{}",
        format!("Charge policy set to mode {mode}: {desc} ({start}-{stop}%).").green()
    );
    Ok(())
}

fn battery_log_analyze(input: String) -> Result<()> {
    let path = Path::new(&input);
    if !path.exists() {
        anyhow::bail!("Log file not found: {input}");
    }

    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

    struct Sample {
        timestamp: String,
        capacity: i64,
        temperature: i64,
        charge_status: String,
        wall_mode: String,
        policy_mode: u8,
        charger: i64,
        current_ma: Option<i64>,
        #[allow(dead_code)]
        voltage_mv: Option<i64>,
        power_mw: Option<i64>,
    }

    let mut samples: Vec<Sample> = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || i == 0 {
            continue; // skip header
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 8 {
            continue;
        }
        samples.push(Sample {
            timestamp: cols[0].to_string(),
            capacity: cols[1].parse().unwrap_or(0),
            temperature: cols[2].parse().unwrap_or(0),
            charge_status: cols[3].to_string(),
            wall_mode: cols[4].to_string(),
            policy_mode: cols[5].parse().unwrap_or(0),
            charger: cols[6].parse().unwrap_or(0),
            current_ma: cols.get(8).and_then(|s| s.parse().ok()),
            voltage_mv: cols.get(9).and_then(|s| s.parse().ok()),
            power_mw: cols.get(10).and_then(|s| s.parse().ok()),
        });
    }

    if samples.is_empty() {
        anyhow::bail!("No data samples found in {input}");
    }

    let n = samples.len();
    let first = &samples[0];
    let last = &samples[n - 1];

    // Time span
    let t_start = &first.timestamp;
    let t_end = &last.timestamp;
    let duration_min = chrono::DateTime::parse_from_rfc3339(t_end)
        .and_then(|end| {
            chrono::DateTime::parse_from_rfc3339(t_start)
                .map(|start| (end - start).num_minutes())
        })
        .unwrap_or(0);

    // SOC trend
    let soc_start = first.capacity;
    let soc_end = last.capacity;
    let soc_delta = soc_end - soc_start;
    let soc_rate = if duration_min > 0 {
        (soc_delta as f64 / duration_min as f64) * 60.0
    } else {
        0.0
    };

    // Charge status distribution
    let mut charging = 0u32;
    let mut discharging = 0u32;
    let mut not_charging = 0u32;
    let mut full = 0u32;
    for s in &samples {
        match s.charge_status.as_str() {
            "Charging" => charging += 1,
            "Discharging" => discharging += 1,
            "Not charging" => not_charging += 1,
            "Full" => full += 1,
            _ => {}
        }
    }

    // Current/power stats (only from samples that have them)
    let currents: Vec<i64> = samples.iter().filter_map(|s| s.current_ma).collect();
    let powers: Vec<i64> = samples.iter().filter_map(|s| s.power_mw).filter(|&p| p != 0).collect();

    let (avg_current, min_current, max_current) = if !currents.is_empty() {
        let sum: i64 = currents.iter().sum();
        let avg = sum / currents.len() as i64;
        let min = *currents.iter().min().unwrap();
        let max = *currents.iter().max().unwrap();
        (Some(avg), Some(min), Some(max))
    } else {
        (None, None, None)
    };

    let avg_power = if !powers.is_empty() {
        Some(powers.iter().sum::<i64>() / powers.len() as i64)
    } else {
        None
    };

    // Temperature
    let temp_min = samples.iter().map(|s| s.temperature).min().unwrap_or(0);
    let temp_max = samples.iter().map(|s| s.temperature).max().unwrap_or(0);

    // Wall mode / charger consistency
    let wall_enabled = samples.iter().all(|s| s.wall_mode == "enable");
    let charger_connected = samples.iter().all(|s| s.charger == 1);
    let policy_mode = first.policy_mode;
    let policy_consistent = samples.iter().all(|s| s.policy_mode == policy_mode);
    let (policy_floor, policy_ceil, policy_desc) = if (policy_mode as usize) < CHARGE_POLICIES.len() {
        let p = &CHARGE_POLICIES[policy_mode as usize];
        (p.0, p.1, p.2)
    } else {
        (0, 0, "Unknown")
    };

    // Verdict
    let all_discharging = discharging == n as u32;
    let current_always_negative = currents.iter().all(|&c| c < 0);
    let soc_declining = soc_delta < 0;
    let below_policy_floor = soc_end < policy_floor as i64;

    let wall_mode_working = !all_discharging && !current_always_negative && !soc_declining;

    // === Output ===

    // Overview table
    let mut overview = Table::new();
    overview.set_header(vec!["Log Overview", ""]);
    overview.add_row(vec!["File", &input]);
    overview.add_row(vec!["Samples", &n.to_string()]);
    overview.add_row(vec!["Time span", &format!("{duration_min} min ({t_start} to {t_end})")]);
    overview.add_row(vec!["Wall mode", &format!("{}", if wall_enabled { "enable (all samples)" } else { "MIXED" })]);
    overview.add_row(vec!["Policy mode", &format!("{policy_mode} ({policy_desc}, {policy_floor}-{policy_ceil}%)")]);
    overview.add_row(vec!["Charger", &format!("{}", if charger_connected { "connected (all samples)" } else { "INTERMITTENT" })]);
    println!("{overview}");

    // SOC analysis
    let mut soc_table = Table::new();
    soc_table.set_header(vec!["SOC Analysis", ""]);
    soc_table.add_row(vec![
        Cell::new("SOC trend"),
        Cell::new(format!("{soc_start}% -> {soc_end}% ({soc_delta:+}%)"))
            .fg(if soc_declining { Color::Red } else { Color::Green }),
    ]);
    soc_table.add_row(vec![
        Cell::new("Rate"),
        Cell::new(format!("{soc_rate:+.1}%/hr")),
    ]);
    if below_policy_floor {
        soc_table.add_row(vec![
            Cell::new("Policy violation"),
            Cell::new(format!("SOC {soc_end}% is below policy floor {policy_floor}%"))
                .fg(Color::Red),
        ]);
    }
    println!("{soc_table}");

    // Charge status distribution
    let mut status_table = Table::new();
    status_table.set_header(vec!["Charge Status", "Count", "%"]);
    let pct = |count: u32| format!("{:.0}%", count as f64 / n as f64 * 100.0);
    if charging > 0 { status_table.add_row(vec!["Charging", &charging.to_string(), &pct(charging)]); }
    if discharging > 0 {
        status_table.add_row(vec![
            &format!("Discharging{}", if all_discharging { " (!)" } else { "" }),
            &discharging.to_string(),
            &pct(discharging),
        ]);
    }
    if not_charging > 0 { status_table.add_row(vec!["Not charging", &not_charging.to_string(), &pct(not_charging)]); }
    if full > 0 { status_table.add_row(vec!["Full", &full.to_string(), &pct(full)]); }
    println!("{status_table}");

    // Current/power stats
    if avg_current.is_some() {
        let mut elec_table = Table::new();
        elec_table.set_header(vec!["Electrical", ""]);
        elec_table.add_row(vec!["Avg current", &format!("{} mA", avg_current.unwrap())]);
        elec_table.add_row(vec!["Current range", &format!("{} to {} mA", min_current.unwrap(), max_current.unwrap())]);
        if let Some(ap) = avg_power {
            elec_table.add_row(vec!["Avg power", &format!("{ap} mW")]);
        }
        elec_table.add_row(vec!["Temperature", &format!("{temp_min}-{temp_max} C")]);
        println!("{elec_table}");
    }

    // Verdict
    println!();
    if wall_mode_working {
        println!("{}", "VERDICT: Wall mode appears to be WORKING".green().bold());
        if charging > 0 {
            println!("  Battery is charging — direct power supply may be active.");
        }
        if !soc_declining {
            println!("  SOC is stable or increasing.");
        }
    } else {
        println!("{}", "VERDICT: Wall mode is NOT WORKING".red().bold());
        println!();
        println!("{}",  "Evidence:".bold());

        if all_discharging {
            println!("  {} charge_status = \"Discharging\" on ALL {} samples", "x".red(), n);
        }
        if current_always_negative {
            println!("  {} Current always negative (drawing from battery)", "x".red());
        }
        if soc_declining {
            println!("  {} SOC declined {soc_start}% -> {soc_end}% ({soc_rate:+.1}%/hr)", "x".red());
        }
        if below_policy_floor {
            println!("  {} SOC {soc_end}% below policy floor {policy_floor}%  (no charge triggered)", "x".red());
        }

        println!();
        println!("{}", "The device is running entirely off battery despite charger + wall mode enabled.".yellow());

        println!();
        println!("{}", "Possible causes:".bold());
        println!("  1. ubus set was accepted but firmware didn't activate bypass");
        println!("  2. Charger wattage insufficient for direct power supply mode");
        println!("  3. Firmware bug - setting is cosmetic, not acted on by charge controller");
        println!("  4. Hardware doesn't truly support battery bypass on this board");

        println!();
        println!("{}", "Recommended next steps:".bold());
        println!("  1. Re-read charger status:  zte settings device charge-limit");
        println!("  2. Toggle wall mode off/on:  zte settings device wall-mode off --confirm");
        println!("     then:                     zte settings device wall-mode on --confirm");
        println!("  3. Try a higher-wattage charger (>10W)");
        println!("  4. Compare discharge rate with wall mode disabled");
        if !policy_consistent {
            println!("  5. Policy mode changed during logging - re-run with consistent settings");
        }
    }

    Ok(())
}

fn wall_mode(args: TransportArgs, state: String, confirm: bool) -> Result<()> {
    let on = match state.as_str() {
        "on" | "1" | "true" | "enable" => true,
        "off" | "0" | "false" | "disable" => false,
        _ => anyhow::bail!("State must be 'on' or 'off'"),
    };

    let label = if on { "enable" } else { "disable" };
    confirm_guard(confirm, &format!("{label} wall mode (direct power supply)"))?;

    let t = get_transport(&args)?;
    let mode_val = if on { "enable" } else { "disable" };
    let uci_val = if on { "1" } else { "0" };

    t.ubus_call(
        "zwrt_bsp.charger",
        "set",
        Some(&json!({"direct_power_supply_mode": mode_val})),
    );
    t.shell(&format!(
        "uci set zwrt_deviceui.Device.direct_power_mode_switch={uci_val} && uci commit zwrt_deviceui"
    ))?;

    println!(
        "{}",
        format!("Wall mode {label}d.").green()
    );
    Ok(())
}
