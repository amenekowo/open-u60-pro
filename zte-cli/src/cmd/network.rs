use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use comfy_table::Table;
use serde_json::{json, Value};

use zte_lib::device::DeviceShell;

use crate::cmd::ShellArgs;

const ZTE_TELEMETRY_DOMAINS: &[&str] = &[
    "iot.zte.com.cn",
    "mifi.zte.com.cn",
    "update.zte.com.cn",
    "cpe.zte.com.cn",
    "fota.zte.com.cn",
    "push.zte.com.cn",
    "log.zte.com.cn",
    "report.zte.com.cn",
    "cloud.zte.com.cn",
    "ztedevices.com",
    "www.ztedevices.com",
    "support.ztedevices.com",
];

const ZTE_IP_RANGES: &[&str] = &[
    "120.197.0.0/16",
    "183.230.0.0/16",
    "221.176.0.0/16",
    "117.169.0.0/16",
];

#[derive(Subcommand)]
pub enum Cmd {
    /// Custom DNS configuration
    Dns {
        /// Shell connection args (SSH default, --adb for USB)
        #[command(flatten)]
        shell: ShellArgs,
        /// Show current DNS configuration
        #[arg(long)]
        show: bool,
        /// Set primary DNS server
        #[arg(long)]
        set: Option<String>,
        /// Secondary DNS server (used with --set)
        #[arg(long)]
        secondary: Option<String>,
        /// Confirm destructive operations
        #[arg(long)]
        confirm: bool,
    },
    /// TTL masking via iptables mangle rules
    Ttl {
        /// Shell connection args (SSH default, --adb for USB)
        #[command(flatten)]
        shell: ShellArgs,
        /// Set TTL/HL value for all traffic
        #[arg(long)]
        set: Option<u32>,
        /// Flush mangle table TTL/HL rules
        #[arg(long)]
        reset: bool,
        /// Show current mangle table rules
        #[arg(long)]
        status: bool,
        /// Confirm destructive operations
        #[arg(long)]
        confirm: bool,
    },
    /// Band locking via ubus API
    Band {
        /// Shell connection args (SSH default, --adb for USB)
        #[command(flatten)]
        shell: ShellArgs,
        /// List current band configuration
        #[arg(long)]
        list: bool,
        /// Lock NR5G bands (comma-separated)
        #[arg(long = "lock")]
        lock_nr: Option<String>,
        /// Lock LTE bands (comma-separated)
        #[arg(long)]
        lock_lte: Option<String>,
        /// Reset to all bands
        #[arg(long)]
        unlock_all: bool,
        /// Show serving cell and locked bands
        #[arg(long)]
        status: bool,
        /// Confirm destructive operations
        #[arg(long)]
        confirm: bool,
    },
    /// Firewall rule management via iptables
    Firewall {
        /// Shell connection args (SSH default, --adb for USB)
        #[command(flatten)]
        shell: ShellArgs,
        /// Show current firewall rules
        #[arg(long)]
        show: bool,
        /// Block outbound traffic to an IP
        #[arg(long)]
        block_outbound: Option<String>,
        /// Allow inbound TCP traffic on a port
        #[arg(long)]
        allow_port: Option<u16>,
        /// Confirm destructive operations
        #[arg(long)]
        confirm: bool,
    },
    /// Disable ZTE phone-home telemetry
    Telemetry {
        /// Shell connection args (SSH default, --adb for USB)
        #[command(flatten)]
        shell: ShellArgs,
        /// Scan outbound connections and ZTE domains
        #[arg(long)]
        scan: bool,
        /// Block ZTE telemetry domains and IPs
        #[arg(long)]
        disable: bool,
        /// Check if telemetry blocking is active
        #[arg(long)]
        status: bool,
        /// Confirm destructive operations
        #[arg(long)]
        confirm: bool,
    },
}

fn get_device(shell: &ShellArgs) -> Result<DeviceShell> {
    shell.connect()
}

fn ubus_call(dev: &DeviceShell, obj: &str, method: &str, params: Option<&str>) -> Value {
    let params_str = params.unwrap_or("{}");
    match dev.shell(
        &format!("ubus call {obj} {method} '{params_str}' 2>/dev/null"),
        10,
    ) {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return json!({});
            }
            serde_json::from_str(trimmed).unwrap_or(json!({"_raw": trimmed}))
        }
        Err(_) => json!({}),
    }
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Dns {
            shell,
            show,
            set,
            secondary,
            confirm,
        } => run_dns(&shell, show, set, secondary, confirm),
        Cmd::Ttl {
            shell,
            set,
            reset,
            status,
            confirm,
        } => run_ttl(&shell, set, reset, status, confirm),
        Cmd::Band {
            shell,
            list,
            lock_nr,
            lock_lte,
            unlock_all,
            status,
            confirm,
        } => run_band(&shell, list, lock_nr, lock_lte, unlock_all, status, confirm),
        Cmd::Firewall {
            shell,
            show,
            block_outbound,
            allow_port,
            confirm,
        } => run_firewall(&shell, show, block_outbound, allow_port, confirm),
        Cmd::Telemetry {
            shell,
            scan,
            disable,
            status,
            confirm,
        } => run_telemetry(&shell, scan, disable, status, confirm),
    }
}

fn run_dns(shell: &ShellArgs, show: bool, set: Option<String>, secondary: Option<String>, confirm: bool) -> Result<()> {
    let dev = get_device(shell)?;
    if show {
        let output = dev.shell("cat /etc/resolv.conf", 10)?;
        println!("{}", "Current DNS configuration:".bold());
        println!("{}", output.trim());
        return Ok(());
    }
    if let Some(primary) = set {
        if !confirm {
            println!("{}", "Use --confirm to apply DNS changes.".yellow());
            return Ok(());
        }
        let mut content = format!("nameserver {primary}");
        if let Some(ref sec) = secondary {
            content.push_str(&format!("\\nnameserver {sec}"));
        }
        content.push_str("\\n");
        dev.shell(&format!("echo -e \"{content}\" > /etc/resolv.conf"), 10)?;
        println!(
            "{}",
            format!(
                "DNS set to {primary}{}",
                secondary.as_ref().map(|s| format!(" / {s}")).unwrap_or_default()
            )
            .green()
        );
        let output = dev.shell("cat /etc/resolv.conf", 10)?;
        println!("{}", output.trim());
        return Ok(());
    }
    println!("Use --show to view or --set PRIMARY [--secondary SECONDARY] to configure DNS.");
    Ok(())
}

fn run_ttl(shell: &ShellArgs, set: Option<u32>, reset: bool, status: bool, confirm: bool) -> Result<()> {
    let dev = get_device(shell)?;
    if status {
        let output = dev.shell("iptables -t mangle -L -n -v 2>/dev/null", 10)?;
        println!("{}", "IPv4 mangle table:".bold());
        println!("{}", if output.trim().is_empty() { "(empty)" } else { output.trim() });
        let output6 = dev.shell("ip6tables -t mangle -L -n -v 2>/dev/null", 10)?;
        println!("\n{}", "IPv6 mangle table:".bold());
        println!("{}", if output6.trim().is_empty() { "(empty)" } else { output6.trim() });
        return Ok(());
    }
    if let Some(ttl_val) = set {
        if !confirm {
            println!("{}", "Use --confirm to apply TTL changes.".yellow());
            return Ok(());
        }
        let cmds = [
            format!("iptables -t mangle -I POSTROUTING -j TTL --ttl-set {ttl_val}"),
            format!("iptables -t mangle -I PREROUTING -j TTL --ttl-set {ttl_val}"),
            format!("ip6tables -t mangle -I POSTROUTING -j HL --hl-set {ttl_val}"),
            format!("ip6tables -t mangle -I PREROUTING -j HL --hl-set {ttl_val}"),
        ];
        for cmd in &cmds {
            dev.shell(cmd, 10)?;
        }
        println!("{}", format!("TTL/HL set to {ttl_val} on all chains.").green());
        return Ok(());
    }
    if reset {
        if !confirm {
            println!("{}", "Use --confirm to flush TTL rules.".yellow());
            return Ok(());
        }
        dev.shell("iptables -t mangle -F", 10)?;
        dev.shell("ip6tables -t mangle -F", 10)?;
        println!("{}", "Mangle table flushed (IPv4 + IPv6).".green());
        return Ok(());
    }
    println!("Use --set VALUE, --reset, or --status.");
    Ok(())
}

fn run_band(
    shell: &ShellArgs,
    list: bool,
    lock_nr: Option<String>,
    lock_lte: Option<String>,
    unlock_all: bool,
    status: bool,
    confirm: bool,
) -> Result<()> {
    let dev = get_device(shell)?;
    if list || status {
        let nw = ubus_call(&dev, "zte_nwinfo_api", "nwinfo_get_netinfo", None);
        if !nw.is_object() {
            println!("{}", "Failed to query network info".red());
            return Ok(());
        }
        let mut table = Table::new();
        table.set_header(vec!["Type", "Bands"]);
        let get = |k: &str| nw.get(k).and_then(|v| v.as_str()).unwrap_or("--");
        table.add_row(vec!["Active Band", get("wan_active_band")]);
        table.add_row(vec!["Network Type", get("network_type")]);
        table.add_row(vec!["LTE Bands", get("lte_band")]);
        table.add_row(vec!["NR5G NSA Lock", get("nr5g_nsa_band_lock")]);
        table.add_row(vec!["NR5G SA Lock", get("nr5g_sa_band_lock")]);
        println!("{table}");
        if status {
            println!(
                "\n{} NR5G RSRP={} dBm, SINR={} dB, PCI={}",
                "Signal:".bold(),
                get("nr5g_rsrp"),
                get("nr5g_snr"),
                get("nr5g_pci")
            );
            println!(
                "{} {} ({})",
                "Operator:".bold(),
                get("network_provider"),
                get("net_select_mode")
            );
        }
        return Ok(());
    }
    if let Some(bands) = lock_nr {
        if !confirm {
            println!("{}", "Use --confirm to lock NR5G bands.".yellow());
            return Ok(());
        }
        ubus_call(
            &dev,
            "zte_nwinfo_api",
            "nwinfo_set_nrbandlock",
            Some(&format!("{{\"nr5g_type\":\"nsa\",\"nr5g_band\":\"{bands}\"}}")),
        );
        ubus_call(
            &dev,
            "zte_nwinfo_api",
            "nwinfo_set_nrbandlock",
            Some(&format!("{{\"nr5g_type\":\"sa\",\"nr5g_band\":\"{bands}\"}}")),
        );
        println!("{}", format!("NR5G bands locked to: {bands}").green());
        return Ok(());
    }
    if let Some(bands) = lock_lte {
        if !confirm {
            println!("{}", "Use --confirm to lock LTE bands.".yellow());
            return Ok(());
        }
        ubus_call(
            &dev,
            "zte_nwinfo_api",
            "nwinfo_set_gwl_bandlock",
            Some(&format!("{{\"is_lte_band\":\"1\",\"lte_band_mask\":\"{bands}\",\"is_gw_band\":\"0\",\"gw_band_mask\":\"\"}}")),
        );
        println!("{}", format!("LTE bands locked to: {bands}").green());
        return Ok(());
    }
    if unlock_all {
        if !confirm {
            println!("{}", "Use --confirm to unlock all bands.".yellow());
            return Ok(());
        }
        ubus_call(&dev, "zte_nwinfo_api", "nwinfo_rest_band_rat", None);
        println!("{}", "All bands unlocked (reset to default).".green());
        return Ok(());
    }
    println!("Use --list, --lock, --lock-lte, --unlock-all, or --status.");
    Ok(())
}

fn run_firewall(
    shell: &ShellArgs,
    show: bool,
    block_outbound: Option<String>,
    allow_port: Option<u16>,
    confirm: bool,
) -> Result<()> {
    let dev = get_device(shell)?;
    if show {
        let output = dev.shell("iptables -L -n -v 2>/dev/null", 10)?;
        println!("{}", "Filter table:".bold());
        println!("{}", if output.trim().is_empty() { "(empty)" } else { output.trim() });
        let nat = dev.shell("iptables -t nat -L -n -v 2>/dev/null", 10)?;
        println!("\n{}", "NAT table:".bold());
        println!("{}", if nat.trim().is_empty() { "(empty)" } else { nat.trim() });
        return Ok(());
    }
    if let Some(ip) = block_outbound {
        if !confirm {
            println!("{}", "Use --confirm to add firewall rules.".yellow());
            return Ok(());
        }
        dev.shell(&format!("iptables -A OUTPUT -d {ip} -j DROP"), 10)?;
        println!("{}", format!("Blocked outbound traffic to {ip}.").green());
        return Ok(());
    }
    if let Some(port) = allow_port {
        if !confirm {
            println!("{}", "Use --confirm to add firewall rules.".yellow());
            return Ok(());
        }
        dev.shell(
            &format!("iptables -A INPUT -p tcp --dport {port} -j ACCEPT"),
            10,
        )?;
        println!("{}", format!("Allowed inbound TCP on port {port}.").green());
        return Ok(());
    }
    println!("Use --show, --block-outbound IP, or --allow-port PORT.");
    Ok(())
}

fn run_telemetry(shell: &ShellArgs, scan: bool, disable: bool, status: bool, confirm: bool) -> Result<()> {
    let dev = get_device(shell)?;
    if scan {
        println!("{}", "Scanning outbound connections...".bold());
        match dev.shell("netstat -tunap 2>/dev/null || ss -tunap 2>/dev/null", 10) {
            Ok(conns) if !conns.trim().is_empty() => println!("{}", conns.trim()),
            _ => println!("(no active connections found)"),
        }
        println!("\n{}", "Checking /etc/hosts...".bold());
        match dev.shell("cat /etc/hosts", 10) {
            Ok(hosts) if !hosts.trim().is_empty() => println!("{}", hosts.trim()),
            _ => println!("(empty)"),
        }
        let hosts_content = dev
            .shell("cat /etc/hosts", 10)
            .unwrap_or_default();
        println!("\n{}", "Known ZTE telemetry domains:".bold());
        let mut table = Table::new();
        table.set_header(vec!["Domain", "Status"]);
        for domain in ZTE_TELEMETRY_DOMAINS {
            let blocked = hosts_content.contains(domain) && hosts_content.contains("127.0.0.1");
            let status_str = if blocked { "blocked" } else { "active" };
            table.add_row(vec![*domain, status_str]);
        }
        println!("{table}");
        return Ok(());
    }
    if status {
        let hosts_content = dev.shell("cat /etc/hosts", 10).unwrap_or_default();
        let blocked_count = ZTE_TELEMETRY_DOMAINS
            .iter()
            .filter(|d| hosts_content.contains(&format!("127.0.0.1 {d}")))
            .count();
        let total = ZTE_TELEMETRY_DOMAINS.len();
        if blocked_count == total {
            println!(
                "{}",
                format!("Telemetry blocking is ACTIVE ({blocked_count}/{total} domains blocked).").green()
            );
        } else if blocked_count > 0 {
            println!(
                "{}",
                format!("Telemetry blocking is PARTIAL ({blocked_count}/{total} domains blocked).").yellow()
            );
        } else {
            println!(
                "{}",
                format!("Telemetry blocking is INACTIVE (0/{total} domains blocked).").red()
            );
        }
        return Ok(());
    }
    if disable {
        if !confirm {
            println!("{}", "Use --confirm to disable telemetry.".yellow());
            return Ok(());
        }
        println!("{}", "Blocking ZTE telemetry domains via /etc/hosts...".bold());
        let current_hosts = dev.shell("cat /etc/hosts", 10).unwrap_or_default();
        let new_entries: Vec<String> = ZTE_TELEMETRY_DOMAINS
            .iter()
            .filter(|d| !current_hosts.contains(&format!("127.0.0.1 {d}")))
            .map(|d| format!("127.0.0.1 {d}"))
            .collect();
        if !new_entries.is_empty() {
            let append = new_entries.join("\\n");
            dev.shell(
                &format!("echo -e \"\\n# ZTE telemetry block\\n{append}\" >> /etc/hosts"),
                10,
            )?;
            println!(
                "{}",
                format!("Added {} domain blocks to /etc/hosts.", new_entries.len()).green()
            );
        } else {
            println!("{}", "All domains already blocked in /etc/hosts.".green());
        }
        println!("{}", "Adding iptables rules to block known ZTE IP ranges...".bold());
        for range in ZTE_IP_RANGES {
            match dev.shell(&format!("iptables -A OUTPUT -d {range} -j DROP"), 10) {
                Ok(_) => println!("  Blocked {range}"),
                Err(e) => println!("  {} Failed to block {range}: {e}", "".red()),
            }
        }
        println!("{}", "Telemetry blocking applied.".green());
        return Ok(());
    }
    println!("Use --scan, --disable, or --status.");
    Ok(())
}
