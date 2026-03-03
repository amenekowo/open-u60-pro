use std::fs;

use anyhow::Result;
use clap::Args as ClapArgs;
use colored::Colorize;
use comfy_table::Table;
use serde_json::{Map, Value};

use zte_lib::at::AtInterface;

use crate::cmd::ShellArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Output JSON file
    #[arg(short, long, default_value = "device_report.json")]
    output: String,

    /// Print verbose error details
    #[arg(short, long)]
    verbose: bool,

    /// Shell connection args (SSH default, --adb for USB)
    #[command(flatten)]
    shell: ShellArgs,
}

struct Section {
    name: &'static str,
    commands: Vec<(&'static str, &'static str)>,
}

fn sections() -> Vec<Section> {
    vec![
        Section {
            name: "system_info",
            commands: vec![
                ("product_model", "getprop ro.product.model"),
                ("kernel_version", "cat /proc/version"),
                ("uptime", "uptime"),
                ("openwrt_release", "cat /etc/openwrt_release"),
            ],
        },
        Section {
            name: "filesystem_map",
            commands: vec![
                ("ls_root", "ls -la /"),
                ("ls_etc", "ls -la /etc/"),
                ("ls_www", "ls -la /www/"),
                ("ls_data", "ls -la /data/"),
                ("config_files", "find / -name \"*.conf\" -o -name \"*.cfg\" -o -name \"*.xml\" 2>/dev/null"),
            ],
        },
        Section {
            name: "running_services",
            commands: vec![
                ("processes", "ps -ef 2>/dev/null || ps w"),
                ("init_scripts", "ls /etc/init.d/"),
            ],
        },
        Section {
            name: "network_config",
            commands: vec![
                ("interfaces", "ifconfig 2>/dev/null || ip addr"),
                ("routes", "ip route"),
                ("resolv_conf", "cat /etc/resolv.conf"),
                ("iptables_filter", "iptables -L -n -v"),
                ("iptables_nat", "iptables -t nat -L -n -v"),
                ("iptables_mangle", "iptables -t mangle -L -n -v"),
            ],
        },
        Section {
            name: "installed_binaries",
            commands: vec![
                ("which_common", "which busybox dropbear sshd iptables ip6tables curl wget"),
                ("bin_listings", "ls /usr/bin/ /usr/sbin/ /bin/ /sbin/"),
            ],
        },
        Section {
            name: "web_interface",
            commands: vec![
                ("web_files", "find / -path \"*/www/*\" -o -path \"*/goahead/*\" -o -path \"*/uhttpd/*\" 2>/dev/null"),
                ("uhttpd_config", "cat /etc/config/uhttpd"),
            ],
        },
        Section {
            name: "telemetry",
            commands: vec![
                ("open_connections", "netstat -tunap 2>/dev/null || ss -tunap 2>/dev/null"),
                ("hosts", "cat /etc/hosts"),
                ("firewall_config", "cat /etc/config/firewall"),
            ],
        },
    ]
}

pub fn run(args: Args) -> Result<()> {
    let dev = args.shell.connect()?;
    println!("{}", "Checking device connection...".bold());
    dev.wait_for_device(10)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{} ({})\n", "Device connected.".green(), dev.transport_name());

    let mut report = serde_json::Map::new();

    for section in sections() {
        print!("Collecting {}...", section.name.cyan());
        let mut data = serde_json::Map::new();
        for (key, cmd) in &section.commands {
            match dev.shell(cmd, 30) {
                Ok(out) => {
                    data.insert(key.to_string(), Value::String(out.trim().to_string()));
                }
                Err(e) => {
                    let msg = format!("ERROR: {e}");
                    data.insert(key.to_string(), Value::String(msg.clone()));
                    if args.verbose {
                        eprint!(" {}", format!("[{e}]").dimmed());
                    }
                }
            }
        }
        println!(" done");
        report.insert(section.name.to_string(), Value::Object(data));
    }

    // Modem info via AT interface
    print!("Collecting {}...", "modem_info".cyan());
    match AtInterface::new(&dev, None) {
        Ok(at) => {
            let mut data = serde_json::Map::new();
            let at_cmds = [
                ("operator", "AT+COPS?"),
                ("signal_quality", "AT+CSQ"),
                ("serving_cell", "AT+QENG=\"servingcell\""),
                ("network_info", "AT+QNWINFO"),
            ];
            for (key, cmd) in at_cmds {
                match at.send(cmd) {
                    Ok(resp) => {
                        data.insert(key.to_string(), Value::String(resp));
                    }
                    Err(e) => {
                        data.insert(
                            key.to_string(),
                            Value::String(format!("ERROR: {e}")),
                        );
                    }
                }
            }
            println!(" done");
            report.insert("modem_info".to_string(), Value::Object(data));
        }
        Err(e) => {
            println!(" {}", format!("failed: {e}").red());
            let mut data = serde_json::Map::new();
            data.insert("error".to_string(), Value::String(e.to_string()));
            report.insert("modem_info".to_string(), Value::Object(data));
        }
    }

    // Save JSON report
    let json_str = serde_json::to_string_pretty(&Value::Object(report.clone()))?;
    fs::write(&args.output, &json_str)?;
    println!("\n{}", format!("Report saved to {}", args.output).green());

    // Print summary
    print_summary(&report);

    Ok(())
}

fn print_summary(report: &Map<String, Value>) {
    println!();
    println!("{}", "=== ZTE Device Explorer Report ===".bold());

    if let Some(Value::Object(sys)) = report.get("system_info") {
        let model = sys
            .get("product_model")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let uptime = sys
            .get("uptime")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!("{} {model}", "Model:".bold());
        println!("{} {uptime}", "Uptime:".bold());
    }

    let mut table = Table::new();
    table.set_header(vec!["Section", "Keys", "Status"]);

    for (section, data) in report {
        if let Value::Object(obj) = data {
            let keys: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
            let errors = obj
                .values()
                .filter(|v| {
                    v.as_str()
                        .map(|s| s.starts_with("ERROR"))
                        .unwrap_or(false)
                })
                .count();
            let total = obj.len();
            let ok = total - errors;
            let status = if errors > 0 {
                format!("{ok}/{total} OK ({errors} errors)")
            } else {
                format!("{ok}/{total} OK")
            };
            table.add_row(vec![section.as_str(), &keys.join(", "), &status]);
        }
    }
    println!("{table}");

    // Modem quick-look
    if let Some(Value::Object(modem)) = report.get("modem_info") {
        println!("\n{}", "Modem Quick-Look:".bold());
        for key in ["operator", "signal_quality", "serving_cell", "network_info"] {
            if let Some(val) = modem.get(key).and_then(|v| v.as_str()) {
                let first_line = val.lines().next().unwrap_or("");
                let display = if first_line.len() > 120 {
                    &first_line[..120]
                } else {
                    first_line
                };
                println!("  {key}: {display}");
            }
        }
    }
    println!();
}
