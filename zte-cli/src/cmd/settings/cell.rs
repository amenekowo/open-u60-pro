use anyhow::{bail, Result};
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;

use super::{confirm_guard, get_transport, print_kv, Transport, TransportArgs};

#[derive(Subcommand)]
pub enum Cmd {
    /// Show current serving cell and lock status
    Status(TransportArgs),
    /// Lock NR5G to a specific cell
    LockNr {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        pci: String,
        #[arg(long)]
        earfcn: String,
        #[arg(long)]
        band: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Lock LTE to a specific cell
    LockLte {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        pci: String,
        #[arg(long)]
        earfcn: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Scan neighbor cells
    Scan(TransportArgs),
    /// Reset all cell and band locks
    Unlock {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },

    // -- STC (Smart Tower Connect) --
    /// Show STC (Smart Tower Connect) cell whitelist config and status
    StcStatus(TransportArgs),
    /// Set STC collection parameters
    StcSet {
        #[command(flatten)]
        transport: TransportArgs,
        /// LTE collect timer max (minutes)
        #[arg(long)]
        lte_timer: Option<u32>,
        /// NR5G collect timer max (minutes)
        #[arg(long)]
        nr_timer: Option<u32>,
        /// LTE whitelist num max
        #[arg(long)]
        lte_max: Option<u32>,
        /// NR5G whitelist num max
        #[arg(long)]
        nr_max: Option<u32>,
        #[arg(long)]
        confirm: bool,
    },
    /// Enable STC cell lock
    StcEnable {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Disable STC cell lock
    StcDisable {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },
    /// Reset STC cell whitelist
    StcReset {
        #[command(flatten)]
        transport: TransportArgs,
        #[arg(long)]
        confirm: bool,
    },

    // -- Signal Quality Detection --
    /// Signal quality detection control
    #[command(subcommand)]
    SignalDetect(SignalDetectCmd),

    // -- DSDS --
    /// Show DSDS (dual-SIM slot switch) thresholds
    DsdsStatus(TransportArgs),
    /// Set DSDS thresholds (requires SSH or ADB)
    DsdsSet {
        #[command(flatten)]
        transport: TransportArgs,
        /// RSRP threshold 1
        #[arg(long)]
        rsrp1: Option<i32>,
        /// RSRP threshold 2
        #[arg(long)]
        rsrp2: Option<i32>,
        /// RSRP threshold 3
        #[arg(long)]
        rsrp3: Option<i32>,
        /// SNR threshold 1
        #[arg(long)]
        snr1: Option<i32>,
        /// SNR threshold 2
        #[arg(long)]
        snr2: Option<i32>,
        /// SNR threshold 3
        #[arg(long)]
        snr3: Option<i32>,
        /// Switch period in seconds
        #[arg(long)]
        period: Option<u32>,
        /// Switch timer
        #[arg(long)]
        timer: Option<u32>,
        #[arg(long)]
        confirm: bool,
    },

    // -- PS No-Service Timer --
    /// Show PS no-service check timer
    PsTimerStatus(TransportArgs),
    /// Set PS no-service check timer (requires SSH or ADB)
    PsTimerSet {
        #[command(flatten)]
        transport: TransportArgs,
        /// Timer value in seconds
        #[arg(long)]
        seconds: u32,
        #[arg(long)]
        confirm: bool,
    },

    // -- Sleep / Idle Timers --
    /// Show sleep and idle timer config
    SleepStatus(TransportArgs),
    /// Set sleep / idle timers (requires SSH or ADB for UCI, ubus for UFI)
    SleepSet {
        #[command(flatten)]
        transport: TransportArgs,
        /// System idle time (SysIdTime)
        #[arg(long)]
        idle_time: Option<u32>,
        /// Timer interval (timerInterval)
        #[arg(long)]
        interval: Option<u32>,
        /// UFI sleep time in seconds (via ubus)
        #[arg(long)]
        ufi_sleep: Option<u32>,
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
pub enum SignalDetectCmd {
    /// Start signal quality detection
    Start(TransportArgs),
    /// Stop signal quality detection
    Stop(TransportArgs),
    /// Show detection progress and quality results
    Status(TransportArgs),
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Status(args) => {
            let t = get_transport(&args)?;
            let nw = t.ubus_call("zte_nwinfo_api", "nwinfo_get_netinfo", None);
            print_kv(
                &nw,
                &[
                    "nr5g_pci", "nr5g_earfcn", "wan_active_band",
                    "lte_pci", "lte_earfcn", "lte_band",
                    "nr5g_sa_band_lock", "nr5g_nsa_band_lock",
                ],
                Some("Cell Status"),
            );
            let lock = t.ubus_call("zte_nwinfo_api", "nwinfo_get_sa_celllock", None);
            if lock != json!({}) {
                println!("\n{}", "SA Cell Lock:".bold());
                println!("{}", serde_json::to_string_pretty(&lock)?);
            }
            Ok(())
        }
        Cmd::LockNr { transport, pci, earfcn, band, confirm } => {
            confirm_guard(confirm, "lock NR5G cell")?;
            let t = get_transport(&transport)?;
            t.ubus_call(
                "zte_nwinfo_api",
                "nwinfo_lock_nr_cell",
                Some(&json!({
                    "lock_nr_pci": pci,
                    "lock_nr_earfcn": earfcn,
                    "lock_nr_cell_band": band,
                })),
            );
            println!("{}", format!("NR5G locked to PCI={pci} EARFCN={earfcn} band={band}").green());
            Ok(())
        }
        Cmd::LockLte { transport, pci, earfcn, confirm } => {
            confirm_guard(confirm, "lock LTE cell")?;
            let t = get_transport(&transport)?;
            t.ubus_call(
                "zte_nwinfo_api",
                "nwinfo_lock_lte_cell",
                Some(&json!({
                    "lock_lte_pci": pci,
                    "lock_lte_earfcn": earfcn,
                })),
            );
            println!("{}", format!("LTE locked to PCI={pci} EARFCN={earfcn}").green());
            Ok(())
        }
        Cmd::Scan(args) => {
            let t = get_transport(&args)?;
            println!("Initiating neighbor cell scan...");
            t.ubus_call("zte_nwinfo_api", "nwinfo_scan_nbr", None);
            std::thread::sleep(std::time::Duration::from_secs(3));
            let nr = t.ubus_call("zte_nwinfo_api", "nwinfo_get_nr5g_nbr_contents", None);
            let lte = t.ubus_call("zte_nwinfo_api", "nwinfo_get_lte_nbr_contents", None);
            if nr != json!({}) {
                println!("\n{}", "NR5G Neighbors:".bold());
                println!("{}", serde_json::to_string_pretty(&nr)?);
            }
            if lte != json!({}) {
                println!("\n{}", "LTE Neighbors:".bold());
                println!("{}", serde_json::to_string_pretty(&lte)?);
            }
            if nr == json!({}) && lte == json!({}) {
                println!("{}", "No neighbor cells found. Try again in a few seconds.".yellow());
            }
            Ok(())
        }
        Cmd::Unlock { transport, confirm } => {
            confirm_guard(confirm, "reset all cell/band locks")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zte_nwinfo_api", "nwinfo_reset_band_cell_setting", None);
            println!("{}", "All cell and band locks reset.".green());
            Ok(())
        }

        // -- STC --
        Cmd::StcStatus(args) => run_stc_status(&get_transport(&args)?),
        Cmd::StcSet { transport, lte_timer, nr_timer, lte_max, nr_max, confirm } => {
            confirm_guard(confirm, "set STC parameters")?;
            let t = get_transport(&transport)?;
            let mut params = serde_json::Map::new();
            if let Some(v) = lte_timer {
                params.insert("stc_lte_collect_timer".into(), json!(v.to_string()));
            }
            if let Some(v) = nr_timer {
                params.insert("stc_nrsa_collect_timer".into(), json!(v.to_string()));
            }
            if let Some(v) = lte_max {
                params.insert("stc_lte_white_list_num_max".into(), json!(v.to_string()));
            }
            if let Some(v) = nr_max {
                params.insert("stc_nrsa_white_list_num_max".into(), json!(v.to_string()));
            }
            if params.is_empty() {
                bail!("Provide at least one parameter to set (--lte-timer, --nr-timer, --lte-max, --nr-max)");
            }
            t.ubus_call(
                "zte_nwinfo_api",
                "nwinfo_set_stc_white_list_par",
                Some(&serde_json::Value::Object(params)),
            );
            println!("{}", "STC parameters updated.".green());
            Ok(())
        }
        Cmd::StcEnable { transport, confirm } => {
            confirm_guard(confirm, "enable STC cell lock")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zte_nwinfo_api", "nwinfo_stc_cell_lock_enable", None);
            println!("{}", "STC cell lock enabled.".green());
            Ok(())
        }
        Cmd::StcDisable { transport, confirm } => {
            confirm_guard(confirm, "disable STC cell lock")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zte_nwinfo_api", "nwinfo_stc_cell_lock_disable", None);
            println!("{}", "STC cell lock disabled.".green());
            Ok(())
        }
        Cmd::StcReset { transport, confirm } => {
            confirm_guard(confirm, "reset STC cell whitelist")?;
            let t = get_transport(&transport)?;
            t.ubus_call("zte_nwinfo_api", "nwinfo_stc_cell_lock_reset", None);
            println!("{}", "STC cell whitelist reset.".green());
            Ok(())
        }

        // -- Signal Detect --
        Cmd::SignalDetect(sub) => match sub {
            SignalDetectCmd::Start(args) => {
                let t = get_transport(&args)?;
                t.ubus_call("zte_nwinfo_api", "nwinfo_start_detect_signal_quality", None);
                println!("{}", "Signal quality detection started.".green());
                Ok(())
            }
            SignalDetectCmd::Stop(args) => {
                let t = get_transport(&args)?;
                t.ubus_call("zte_nwinfo_api", "nwinfo_end_detect_signal_quality", None);
                println!("{}", "Signal quality detection stopped.".green());
                Ok(())
            }
            SignalDetectCmd::Status(args) => {
                let t = get_transport(&args)?;
                let progress = t.ubus_call("zte_nwinfo_api", "nwinfo_get_progress_and_quality", None);
                if progress != json!({}) {
                    println!("{}", "Signal Detection Progress:".bold());
                    println!("{}", serde_json::to_string_pretty(&progress)?);
                }
                let recorder = t.ubus_call("zte_nwinfo_api", "nwinfo_get_detect_quality_recorder", None);
                if recorder != json!({}) {
                    println!("\n{}", "Detection Quality Recorder:".bold());
                    println!("{}", serde_json::to_string_pretty(&recorder)?);
                }
                if progress == json!({}) && recorder == json!({}) {
                    println!("{}", "No signal detection data available.".yellow());
                }
                Ok(())
            }
        },

        // -- DSDS --
        Cmd::DsdsStatus(args) => run_dsds_status(&get_transport(&args)?),
        Cmd::DsdsSet { transport, rsrp1, rsrp2, rsrp3, snr1, snr2, snr3, period, timer, confirm } => {
            confirm_guard(confirm, "set DSDS thresholds")?;
            let t = get_transport(&transport)?;
            let mut cmds: Vec<String> = Vec::new();
            if let Some(v) = rsrp1 { cmds.push(format!("uci set zte_nwinfo.dsds.rsrp_threshold1='{v}'")); }
            if let Some(v) = rsrp2 { cmds.push(format!("uci set zte_nwinfo.dsds.rsrp_threshold2='{v}'")); }
            if let Some(v) = rsrp3 { cmds.push(format!("uci set zte_nwinfo.dsds.rsrp_threshold3='{v}'")); }
            if let Some(v) = snr1 { cmds.push(format!("uci set zte_nwinfo.dsds.snr_threshold1='{v}'")); }
            if let Some(v) = snr2 { cmds.push(format!("uci set zte_nwinfo.dsds.snr_threshold2='{v}'")); }
            if let Some(v) = snr3 { cmds.push(format!("uci set zte_nwinfo.dsds.snr_threshold3='{v}'")); }
            if let Some(v) = period { cmds.push(format!("uci set zte_nwinfo.dsds.switch_period='{v}'")); }
            if let Some(v) = timer { cmds.push(format!("uci set zte_nwinfo.dsds.switch_timer='{v}'")); }
            if cmds.is_empty() {
                bail!("Provide at least one parameter to set");
            }
            cmds.push("uci commit zte_nwinfo".into());
            t.shell(&cmds.join(" && "))?;
            println!("{}", "DSDS thresholds updated.".green());
            Ok(())
        }

        // -- PS Timer --
        Cmd::PsTimerStatus(args) => run_ps_timer_status(&get_transport(&args)?),
        Cmd::PsTimerSet { transport, seconds, confirm } => {
            confirm_guard(confirm, "set PS no-service timer")?;
            let t = get_transport(&transport)?;
            t.shell(&format!(
                "uci set zte_nwinfo.sys_info.ps_no_srv_check_timer_cfg='{seconds}' && uci commit zte_nwinfo"
            ))?;
            println!("{}", format!("PS no-service timer set to {seconds}s.").green());
            Ok(())
        }

        // -- Sleep --
        Cmd::SleepStatus(args) => run_sleep_status(&get_transport(&args)?),
        Cmd::SleepSet { transport, idle_time, interval, ufi_sleep, confirm } => {
            confirm_guard(confirm, "set sleep timers")?;
            let t = get_transport(&transport)?;
            let mut uci_cmds: Vec<String> = Vec::new();
            if let Some(v) = idle_time {
                uci_cmds.push(format!("uci set zwrt_sleep.ztmp_time.SysIdTime='{v}'"));
            }
            if let Some(v) = interval {
                uci_cmds.push(format!("uci set zwrt_sleep.ztmp_interval.timerInterval='{v}'"));
            }
            if !uci_cmds.is_empty() {
                uci_cmds.push("uci commit zwrt_sleep".into());
                t.shell(&uci_cmds.join(" && "))?;
                println!("{}", "Sleep UCI settings updated.".green());
            }
            if let Some(v) = ufi_sleep {
                t.ubus_call(
                    "zwrt_zte_sleep_faw.wakelock",
                    "set_ufi_sleep",
                    Some(&json!({ "ufiSleepTime": v.to_string() })),
                );
                println!("{}", format!("UFI sleep time set to {v}s.").green());
            }
            if idle_time.is_none() && interval.is_none() && ufi_sleep.is_none() {
                bail!("Provide at least one parameter (--idle-time, --interval, --ufi-sleep)");
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// STC status helper
// ---------------------------------------------------------------------------

fn run_stc_status(t: &Transport) -> Result<()> {
    let cfg = t.ubus_call("zte_nwinfo_api", "nwinfo_get_stc_white_list_par", None);
    if cfg != json!({}) {
        print_kv(
            &cfg,
            &[
                "cell_white_list_enable_flag",
                "stc_lte_collect_timer_max",
                "stc_nrsa_collect_timer_max",
                "stc_lte_white_list_num_max",
                "stc_nrsa_white_list_num_max",
                "stc_delayed_start_timer",
                "stc_delayed_start_flag",
            ],
            Some("STC Configuration"),
        );
    }
    let status = t.ubus_call("zte_nwinfo_api", "nwinfo_get_stc_white_list_status", None);
    if status != json!({}) {
        println!();
        print_kv(
            &status,
            &[
                "stc_lte_collect_timer",
                "stc_nrsa_collect_timer",
                "stc_lte_white_list_num",
                "stc_nrsa_white_list_num",
                "stc_run_time",
            ],
            Some("STC Runtime Status"),
        );
    }
    if cfg == json!({}) && status == json!({}) {
        println!("{}", "No STC data available.".yellow());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// DSDS status helper
// ---------------------------------------------------------------------------

fn run_dsds_status(t: &Transport) -> Result<()> {
    let out = t.shell("uci show zte_nwinfo.dsds 2>/dev/null")?;
    if out.trim().is_empty() {
        println!("{}", "No DSDS config found (zte_nwinfo.dsds section missing).".yellow());
        return Ok(());
    }
    println!("{}", "DSDS Thresholds:".bold());
    for line in out.lines() {
        // Format: zte_nwinfo.dsds.key='value'
        if let Some(kv) = line.strip_prefix("zte_nwinfo.dsds.") {
            let parts: Vec<&str> = kv.splitn(2, '=').collect();
            if parts.len() == 2 {
                let key = parts[0];
                let val = parts[1].trim_matches('\'');
                println!("  {:<20} {}", key, val);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// PS timer status helper
// ---------------------------------------------------------------------------

fn run_ps_timer_status(t: &Transport) -> Result<()> {
    let out = t.shell("uci get zte_nwinfo.sys_info.ps_no_srv_check_timer_cfg 2>/dev/null")?;
    let val = out.trim();
    if val.is_empty() {
        println!("{}", "PS no-service timer not set.".yellow());
    } else {
        println!("{} {}s", "PS no-service check timer:".bold(), val);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Sleep status helper
// ---------------------------------------------------------------------------

fn run_sleep_status(t: &Transport) -> Result<()> {
    let idle = t.shell("uci get zwrt_sleep.ztmp_time.SysIdTime 2>/dev/null").unwrap_or_default();
    let interval = t.shell("uci get zwrt_sleep.ztmp_interval.timerInterval 2>/dev/null").unwrap_or_default();

    println!("{}", "Sleep / Idle Timers:".bold());
    println!("  {:<20} {}", "SysIdTime", if idle.trim().is_empty() { "--" } else { idle.trim() });
    println!("  {:<20} {}", "timerInterval", if interval.trim().is_empty() { "--" } else { interval.trim() });

    // Also try to get UFI sleep info via ubus
    let ufi = t.ubus_call("zwrt_zte_sleep_faw.wakelock", "get_ufi_sleep", None);
    if ufi != json!({}) {
        println!("\n{}", "UFI Sleep:".bold());
        println!("{}", serde_json::to_string_pretty(&ufi)?);
    }
    Ok(())
}
