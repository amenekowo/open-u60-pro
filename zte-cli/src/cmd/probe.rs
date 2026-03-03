use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;
use std::time::Instant;

use anyhow::Result;
use clap::Args as ClapArgs;
use colored::Colorize;
use comfy_table::{Cell, Table};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use serde_json::{json, Map, Value};

use zte_lib::error::ZteError;
use zte_lib::ubus::UbusClient;

static READONLY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(get[_A-Z]|get$|list$|status$|report$|info$|contents$|result$|check|query|dump$)|_get_|_get$|_list$|_status$|_report$|_info$|_contents$|_result$|_check$|_check_"
    ).unwrap()
});

#[derive(ClapArgs)]
pub struct Args {
    /// Router IP (auto-detect if omitted)
    #[arg(short, long)]
    gateway: Option<String>,

    /// Admin password (prompted if omitted)
    #[arg(short, long)]
    password: Option<String>,

    /// Output JSON file
    #[arg(short, long, default_value = "ubus_probe_report.json")]
    output: String,

    /// Seconds between calls
    #[arg(short, long, default_value_t = 0.1)]
    delay: f64,

    /// Only enumerate, don't call methods
    #[arg(long)]
    skip_calls: bool,

    /// Also probe set/reset/delete methods (may cause reboot!)
    #[arg(long)]
    include_writes: bool,

    /// Print per-call status
    #[arg(short, long)]
    verbose: bool,

    /// Retry code-2 methods from existing report with guessed params
    #[arg(long)]
    retry: bool,
}

fn seed_objects() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        ("zwrt_web", vec!["web_login_info", "web_login", "web_logout"]),
        ("zte_nwinfo_api", vec![
            "nwinfo_get_netinfo", "nwinfo_get_siminfo", "nwinfo_get_apninfo",
            "nwinfo_set_nrbandlock", "nwinfo_set_gwl_bandlock", "nwinfo_rest_band_rat",
        ]),
        ("zwrt_apn_object", vec!["get_apn_list", "get_current_apn"]),
        ("zwrt_wlan", vec!["get_wlan_info", "get_wlan_station_list"]),
        ("zwrt_router.api", vec!["get_router_info"]),
        ("zwrt_smart_mng.api", vec!["get_smart_mng_info"]),
        ("zwrt_bsp.battery", vec!["list"]),
        ("zwrt_bsp.charger", vec!["list"]),
        ("zwrt_bsp.powerbank", vec!["list"]),
        ("zwrt_bsp.led", vec!["list"]),
        ("zwrt_bsp.usb", vec!["list", "set"]),
        ("zwrt_bsp.thermal", vec!["get_cpu_temp", "get_policy", "list"]),
        ("system", vec!["info", "board"]),
        ("zwrt_time_manager", vec!["get_time"]),
        ("zwrt_data", vec!["get_wwandst"]),
        ("zwrt_zte_mdm.api", vec!["get_mdm_info"]),
        ("network.device", vec!["status"]),
        ("network.interface.lan", vec!["status"]),
        ("luci-rpc", vec!["getNetworkDevices"]),
    ])
}

fn is_readonly_method(name: &str) -> bool {
    READONLY_RE.is_match(name)
}

/// Parse list result into {object: {method: arg_spec}} map.
fn parse_list_result(value: &Value) -> BTreeMap<String, BTreeMap<String, Value>> {
    let obj = match value {
        Value::Object(o) => o,
        _ => return BTreeMap::new(),
    };
    let mut out = BTreeMap::new();
    for (key, val) in obj {
        if let Value::Object(methods) = val {
            let mut method_map = BTreeMap::new();
            for (m, spec) in methods {
                method_map.insert(m.clone(), spec.clone());
            }
            out.insert(key.clone(), method_map);
        }
    }
    out
}

fn enumerate_objects(client: &UbusClient, use_anon: bool) -> BTreeMap<String, BTreeMap<String, Value>> {
    let label = if use_anon { "anon" } else { "auth" };

    let list_fn = |pattern: Option<&str>| -> zte_lib::error::Result<Value> {
        if use_anon { client.list_anon(pattern) } else { client.list(pattern) }
    };

    // Attempt 1: list("*")
    if let Ok(result) = list_fn(Some("*")) {
        let parsed = parse_list_result(&result);
        if !parsed.is_empty() {
            println!("  {} list('*') returned {} objects", label.green(), parsed.len());
            return parsed;
        }
    }

    // Attempt 2: list() without pattern
    if let Ok(result) = list_fn(None) {
        let parsed = parse_list_result(&result);
        if !parsed.is_empty() {
            println!("  {} list() returned {} objects", label.green(), parsed.len());
            return parsed;
        }
    }

    // Attempt 3: probe seed objects individually
    println!("  {} list RPC not available, falling back to seed list", label.yellow());
    let seeds = seed_objects();
    let mut discovered = BTreeMap::new();
    for (obj_name, methods) in &seeds {
        match list_fn(Some(obj_name)) {
            Ok(result) => {
                let parsed = parse_list_result(&result);
                if !parsed.is_empty() {
                    discovered.extend(parsed);
                    continue;
                }
            }
            Err(_) => {}
        }
        // Build stub from seed
        let mut method_map = BTreeMap::new();
        for m in methods {
            method_map.insert(m.to_string(), json!({}));
        }
        discovered.insert(obj_name.to_string(), method_map);
    }

    if discovered.is_empty() {
        println!("  {} using hardcoded seed list", label.yellow());
        for (obj, methods) in &seeds {
            let mut method_map = BTreeMap::new();
            for m in methods {
                method_map.insert(m.to_string(), json!({}));
            }
            discovered.insert(obj.to_string(), method_map);
        }
    }
    discovered
}

#[derive(Clone)]
struct ProbeEntry {
    status: String,
    data: Value,
    error: Option<String>,
}

fn probe_methods(
    client: &UbusClient,
    objects: &BTreeMap<String, BTreeMap<String, Value>>,
    use_anon: bool,
    delay: f64,
    verbose: bool,
    skip_writes: bool,
) -> BTreeMap<String, BTreeMap<String, ProbeEntry>> {
    let label = if use_anon { "Anonymous" } else { "Authenticated" };
    let total: u64 = objects.values().map(|m| m.len() as u64).sum();
    let mut results = BTreeMap::new();
    let mut skipped = 0u64;
    let mut consecutive_conn_errors = 0u32;
    const MAX_CONN_ERRORS: u32 = 3;

    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(&format!("  {{spinner}} {label} probe {{bar:40}} {{pos}}/{{len}}"))
            .unwrap()
            .progress_chars("=> "),
    );

    'outer: for (obj_name, methods) in objects {
        let mut obj_results = BTreeMap::new();
        for method_name in methods.keys() {
            if skip_writes && !is_readonly_method(method_name) {
                obj_results.insert(method_name.clone(), ProbeEntry {
                    status: "skipped".into(),
                    data: Value::Null,
                    error: Some("non-readonly method skipped".into()),
                });
                skipped += 1;
                pb.inc(1);
                if verbose {
                    pb.suspend(|| println!("  {} {}.{}", "SKIP".dimmed(), obj_name, method_name));
                }
                continue;
            }

            let call_result = if use_anon {
                client.call_anon(obj_name, method_name, None)
            } else {
                client.call(obj_name, method_name, None)
            };

            let entry = match call_result {
                Ok(data) => {
                    consecutive_conn_errors = 0;
                    if verbose {
                        pb.suspend(|| println!("  {}  {}.{}", "OK".green(), obj_name, method_name));
                    }
                    ProbeEntry { status: "ok".into(), data, error: None }
                }
                Err(ZteError::Http(ref e)) => {
                    consecutive_conn_errors += 1;
                    if verbose {
                        pb.suspend(|| println!("  {} {}.{}: {}", "EXC".red(), obj_name, method_name, e));
                    }
                    ProbeEntry {
                        status: "error".into(),
                        data: Value::Null,
                        error: Some(format!("Http: {e}")),
                    }
                }
                Err(ref e) => {
                    consecutive_conn_errors = 0;
                    if verbose {
                        pb.suspend(|| println!("  {} {}.{}: {}", "ERR".red(), obj_name, method_name, e));
                    }
                    ProbeEntry {
                        status: "error".into(),
                        data: Value::Null,
                        error: Some(e.to_string()),
                    }
                }
            };

            obj_results.insert(method_name.clone(), entry);
            pb.inc(1);

            if consecutive_conn_errors >= MAX_CONN_ERRORS {
                pb.suspend(|| {
                    eprintln!(
                        "\n  {} Device unreachable ({} consecutive failures) — aborting {} probe.",
                        "!".red().bold(),
                        consecutive_conn_errors,
                        label,
                    );
                });
                results.insert(obj_name.clone(), obj_results);
                break 'outer;
            }

            if delay > 0.0 {
                std::thread::sleep(std::time::Duration::from_secs_f64(delay));
            }
        }
        results.insert(obj_name.clone(), obj_results);
    }
    pb.finish_and_clear();

    if skipped > 0 {
        println!(
            "  {} Skipped {} write/destructive methods (use --include-writes to probe them)",
            " ".dimmed(),
            skipped,
        );
    }
    results
}

fn build_report(
    gateway: &str,
    duration: f64,
    enum_auth: &BTreeMap<String, BTreeMap<String, Value>>,
    enum_anon: &BTreeMap<String, BTreeMap<String, Value>>,
    probe_auth: &BTreeMap<String, BTreeMap<String, ProbeEntry>>,
    probe_anon: &BTreeMap<String, BTreeMap<String, ProbeEntry>>,
) -> Value {
    let enum_to_json = |e: &BTreeMap<String, BTreeMap<String, Value>>| -> Value {
        let mut objects = Map::new();
        for (obj, methods) in e {
            let keys: Vec<Value> = methods.keys().map(|k| Value::String(k.clone())).collect();
            objects.insert(obj.clone(), Value::Array(keys));
        }
        json!({
            "object_count": e.len(),
            "total_methods": e.values().map(|m| m.len()).sum::<usize>(),
            "objects": objects,
        })
    };

    let mut total_calls = 0u64;
    let mut successful = 0u64;
    let mut failed = 0u64;
    let mut skipped_count = 0u64;
    let mut anon_accessible = Vec::new();

    for (obj, methods) in probe_auth {
        for (method, entry) in methods {
            if entry.status == "skipped" {
                skipped_count += 1;
                continue;
            }
            total_calls += 1;
            if entry.status == "ok" {
                successful += 1;
            } else {
                failed += 1;
            }
            let _ = (obj, method); // used in loop binding
        }
    }

    for (obj, methods) in probe_anon {
        for (method, entry) in methods {
            if entry.status == "skipped" {
                skipped_count += 1;
                continue;
            }
            total_calls += 1;
            if entry.status == "ok" {
                successful += 1;
                anon_accessible.push(format!("{obj}.{method}"));
            } else {
                failed += 1;
            }
        }
    }

    anon_accessible.sort();

    let success_rate = if total_calls > 0 {
        (successful as f64 / total_calls as f64 * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let probe_to_json = |p: &BTreeMap<String, BTreeMap<String, ProbeEntry>>| -> Value {
        let mut out = Map::new();
        for (obj, methods) in p {
            let mut m_out = Map::new();
            for (method, entry) in methods {
                m_out.insert(method.clone(), json!({
                    "status": entry.status,
                    "data": entry.data,
                    "error": entry.error,
                }));
            }
            out.insert(obj.clone(), Value::Object(m_out));
        }
        Value::Object(out)
    };

    json!({
        "metadata": {
            "gateway": gateway,
            "timestamp": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string(),
            "duration_seconds": (duration * 10.0).round() / 10.0,
        },
        "enumeration": {
            "auth": enum_to_json(enum_auth),
            "anon": enum_to_json(enum_anon),
        },
        "probe_results": {
            "auth": probe_to_json(probe_auth),
            "anon": probe_to_json(probe_anon),
        },
        "summary": {
            "total_calls": total_calls,
            "successful": successful,
            "failed": failed,
            "skipped": skipped_count,
            "success_rate": success_rate,
            "anon_accessible_methods": anon_accessible,
        },
    })
}

fn print_results(
    enum_auth: &BTreeMap<String, BTreeMap<String, Value>>,
    enum_anon: &BTreeMap<String, BTreeMap<String, Value>>,
    probe_auth: &BTreeMap<String, BTreeMap<String, ProbeEntry>>,
    probe_anon: &BTreeMap<String, BTreeMap<String, ProbeEntry>>,
    report: &Value,
) {
    println!();

    // Enumeration overview
    let mut table = Table::new();
    table.set_header(vec!["", "Objects", "Methods"]);
    table.add_row(vec![
        Cell::new("Authenticated"),
        Cell::new(enum_auth.len()),
        Cell::new(enum_auth.values().map(|m| m.len()).sum::<usize>()),
    ]);
    table.add_row(vec![
        Cell::new("Anonymous"),
        Cell::new(enum_anon.len()),
        Cell::new(enum_anon.values().map(|m| m.len()).sum::<usize>()),
    ]);
    println!("  Enumeration Overview\n{table}");
    println!();

    // Per-object summary
    let all_objects: BTreeSet<&String> = enum_auth.keys().chain(enum_anon.keys()).collect();
    if !all_objects.is_empty() {
        let mut obj_table = Table::new();
        obj_table.set_header(vec!["Object", "Methods", "Auth OK", "Anon OK"]);
        for obj in &all_objects {
            let auth_methods = enum_auth.get(*obj);
            let anon_methods = enum_anon.get(*obj);
            let method_count = std::cmp::max(
                auth_methods.map(|m| m.len()).unwrap_or(0),
                anon_methods.map(|m| m.len()).unwrap_or(0),
            );
            let auth_ok: usize = probe_auth
                .get(*obj)
                .map(|m| m.values().filter(|e| e.status == "ok").count())
                .unwrap_or(0);
            let anon_ok: usize = probe_anon
                .get(*obj)
                .map(|m| m.values().filter(|e| e.status == "ok").count())
                .unwrap_or(0);
            obj_table.add_row(vec![
                Cell::new(obj),
                Cell::new(method_count),
                Cell::new(if auth_ok > 0 { auth_ok.to_string() } else { "-".into() }),
                Cell::new(if anon_ok > 0 { anon_ok.to_string() } else { "-".into() }),
            ]);
        }
        println!("  Object Summary\n{obj_table}");
        println!();
    }

    // Anonymous access highlights
    if let Some(anon_methods) = report["summary"]["anon_accessible_methods"].as_array() {
        if !anon_methods.is_empty() {
            println!("  {} {}", "!".yellow().bold(), "Anonymous Access (no login required)".yellow().bold());
            for m in anon_methods {
                if let Some(s) = m.as_str() {
                    println!("    {s}");
                }
            }
            println!();
        }
    }

    // Statistics
    let s = &report["summary"];
    let total = s["total_calls"].as_u64().unwrap_or(0);
    let ok = s["successful"].as_u64().unwrap_or(0);
    let fail = s["failed"].as_u64().unwrap_or(0);
    let rate = s["success_rate"].as_f64().unwrap_or(0.0);
    let skipped = s["skipped"].as_u64().unwrap_or(0);
    print!(
        "  Total calls: {}  OK: {}  Failed: {}  Success rate: {:.1}%",
        total,
        ok.to_string().green(),
        fail.to_string().red(),
        rate,
    );
    if skipped > 0 {
        print!("  Skipped: {}", skipped.to_string().dimmed());
    }
    println!("\n");
}

/// Returns a map of (object, method) -> params for safe read-only methods that
/// returned code 2 (missing params) in the initial probe.
fn retry_params(mac: &str) -> Vec<(&'static str, &'static str, Value)> {
    vec![
        // WWAN data methods — need iface param
        ("zwrt_data", "get_wwandst", json!({"iface": "rmnet_data0"})),
        ("zwrt_data", "get_wwandst_clearday", json!({"iface": "rmnet_data0"})),
        ("zwrt_data", "get_wwandst_monthlimit", json!({"iface": "rmnet_data0"})),
        ("zwrt_data", "get_wwaniface", json!({"iface": "rmnet_data0"})),
        // Router API — retry with empty params (may need fresh auth)
        ("zwrt_router.api", "router_offline_list", json!({})),
        ("zwrt_router.api", "router_wireless_access_list", json!({})),
        // MAC-based methods
        ("zwrt_router.api", "router_get_pctrl_by_mac", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_cutoff_get", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_app_filter_get", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_time_ctl_get", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_times_ctl_get", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_domain_filter_get", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_defective_get", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_get_app_list_info", json!({"type": "0"})),
        ("zwrt_smart_mng.api", "smart_mng_get_appfilter_all_or_type", json!({"type": "0"})),
        ("zwrt_smart_mng.api", "get_youth_device_info_by_mac", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_used_appinfo_by_mac_get", json!({"mac": mac})),
        ("zwrt_smart_mng.api", "smart_mng_app_time_info_by_mac_get", json!({"mac": mac})),
        // WLAN section
        ("zwrt_wlan", "wlan_uci_get_section", json!({"section": "radio0"})),
    ]
}

/// Extract the first MAC address from the ARP table in an existing probe report.
fn extract_mac_from_report(report: &Value) -> Option<String> {
    report
        .pointer("/probe_results/auth/zwrt_router.api/router_get_arptable/data/arptable")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|entry| entry["mac"].as_str())
        .map(|s| s.to_string())
}

fn run_retry(args: &Args) -> Result<()> {
    println!("\n  {}\n", "ZTE Probe — Retry Mode".bold());

    // Load existing report
    let report_path = &args.output;
    let raw = std::fs::read_to_string(report_path)
        .map_err(|e| anyhow::anyhow!("Cannot read report {report_path}: {e}"))?;
    let mut report: Value = serde_json::from_str(&raw)?;

    // Extract MAC for param substitution
    let mac = extract_mac_from_report(&report).unwrap_or_default();
    if mac.is_empty() {
        println!("  {} No MAC found in ARP table, MAC-based methods will use empty string", "!".yellow());
    } else {
        println!("  Using MAC from report: {}", mac.cyan());
    }

    let entries = retry_params(&mac);
    println!("  Retry list: {} methods (delay: {}s)\n", entries.len(), args.delay);

    // Connect and authenticate
    let mut client = UbusClient::new(args.gateway.as_deref(), 10);
    println!("  Gateway: {}", client.gateway.cyan());

    let password = match &args.password {
        Some(p) => p.clone(),
        None => rpassword::prompt_password("  Admin password: ")?,
    };
    client.login(&password)?;
    println!("  {}", "Authenticated.".green());

    let delay = if args.delay > 0.0 { args.delay } else { 1.0 };
    let mut ok_count = 0u32;
    let mut err_count = 0u32;
    let mut consecutive_conn_errors = 0u32;
    const MAX_CONN_ERRORS: u32 = 3;

    for (obj, method, params) in &entries {
        let tag = format!("{obj}.{method}");
        let result = client.call(obj, method, Some(params));

        match result {
            Ok(data) => {
                consecutive_conn_errors = 0;
                ok_count += 1;
                println!("  {}  {tag}", "OK".green());

                // Merge into report
                let path = format!("/probe_results/auth/{obj}/{method}");
                if let Some(slot) = report.pointer_mut(&path) {
                    *slot = json!({"status": "ok", "data": data, "error": null});
                } else {
                    // Ensure object exists, then insert method
                    let obj_path = format!("/probe_results/auth/{obj}");
                    if report.pointer(&obj_path).is_none() {
                        if let Some(auth) = report.pointer_mut("/probe_results/auth") {
                            if let Value::Object(map) = auth {
                                map.insert(obj.to_string(), json!({}));
                            }
                        }
                    }
                    if let Some(obj_val) = report.pointer_mut(&obj_path) {
                        if let Value::Object(map) = obj_val {
                            map.insert(method.to_string(), json!({"status": "ok", "data": data, "error": null}));
                        }
                    }
                }
            }
            Err(ZteError::Http(ref e)) => {
                consecutive_conn_errors += 1;
                err_count += 1;
                println!("  {} {tag}: {e}", "EXC".red());
            }
            Err(ref e) => {
                consecutive_conn_errors = 0;
                err_count += 1;
                println!("  {} {tag}: {e}", "ERR".red());

                // Update error in report
                let path = format!("/probe_results/auth/{obj}/{method}");
                if let Some(slot) = report.pointer_mut(&path) {
                    *slot = json!({"status": "error", "data": null, "error": e.to_string()});
                }
            }
        }

        if consecutive_conn_errors >= MAX_CONN_ERRORS {
            eprintln!(
                "\n  {} Device unreachable ({} consecutive failures) — aborting retry.",
                "!".red().bold(),
                consecutive_conn_errors,
            );
            break;
        }

        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }

    // Save updated report
    let json_str = serde_json::to_string_pretty(&report)?;
    std::fs::write(report_path, &json_str)?;

    println!(
        "\n  {} Retry complete: {} OK, {} errors. Report updated: {}",
        "Done".green().bold(),
        ok_count.to_string().green(),
        err_count.to_string().red(),
        report_path,
    );

    Ok(())
}

pub fn run(args: Args) -> Result<()> {
    if args.retry {
        return run_retry(&args);
    }

    println!("\n  {}\n", "ZTE HTTP Ubus Probe".bold());

    // Connect
    let mut client = UbusClient::new(args.gateway.as_deref(), 10);
    println!("  Gateway: {}", client.gateway.cyan());

    // Authenticate
    let mut auth_ok = false;
    let password = match args.password {
        Some(p) => p,
        None => {
            let p = rpassword::prompt_password("  Admin password (Enter to skip): ")?;
            p
        }
    };
    if !password.is_empty() {
        match client.login(&password) {
            Ok(_) => {
                auth_ok = true;
                println!("  {}", "Authenticated.".green());
            }
            Err(e) => {
                println!("  {} Login failed ({}), continuing anonymous-only.", "!".yellow(), e);
            }
        }
    } else {
        println!("  {} No password provided, anonymous-only mode.", "!".yellow());
    }

    let start = Instant::now();
    let mut enum_auth = BTreeMap::new();
    let enum_anon;
    let mut probe_auth = BTreeMap::new();
    let mut probe_anon = BTreeMap::new();

    // Phase 1: Enumerate
    println!("\n  {}", "Phase 1: Enumeration".bold());
    if auth_ok {
        enum_auth = enumerate_objects(&client, false);
    }
    enum_anon = enumerate_objects(&client, true);

    if !args.skip_calls {
        let skip_writes = !args.include_writes;

        // Phase 2: Authenticated probe
        if auth_ok && !enum_auth.is_empty() {
            println!("\n  {}", "Phase 2: Authenticated Probe".bold());
            probe_auth = probe_methods(&client, &enum_auth, false, args.delay, args.verbose, skip_writes);
        }

        // Phase 3: Anonymous probe
        if !enum_anon.is_empty() {
            let phase = if auth_ok { "3" } else { "2" };
            println!("\n  {}", format!("Phase {phase}: Anonymous Probe").bold());
            probe_anon = probe_methods(&client, &enum_anon, true, args.delay, args.verbose, skip_writes);
        }
    }

    let duration = start.elapsed().as_secs_f64();

    // Build and save report
    let report = build_report(&client.gateway, duration, &enum_auth, &enum_anon, &probe_auth, &probe_anon);
    let json_str = serde_json::to_string_pretty(&report)?;
    std::fs::write(&args.output, &json_str)?;
    println!("\n  {} Report saved to {}", "OK".green(), args.output);

    // Terminal summary
    print_results(&enum_auth, &enum_anon, &probe_auth, &probe_anon, &report);

    Ok(())
}
