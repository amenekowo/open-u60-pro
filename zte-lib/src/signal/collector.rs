use std::collections::HashMap;
use std::time::Instant;

use crate::device::DeviceShell;
use crate::ubus::UbusClient;
use crate::signal::parsers::*;
use crate::signal::types::*;

/// Stateful signal collector supporting multiple collection methods.
pub struct SignalCollector {
    method: CollectionMethod,
    // Delta state for speed calculation
    prev_rx: u64,
    prev_tx: u64,
    prev_time: Option<Instant>,
    prev_speed_source: String,
    // Delta state for CPU usage
    prev_cpu_idle: u64,
    prev_cpu_total: u64,
    // Cached slow-changing data
    device_identity: DeviceIdentityCache,
    poll_count: u64,
    // Sticky device cache to prevent flickering
    devices_cache: HashMap<String, ConnectedDevice>,
    devices_missed: HashMap<String, u32>,
    // Source availability
    adb_ubus_available: bool,
    at_available: bool,
    http_available: bool,
    // One-shot warnings
}

#[derive(Default)]
struct DeviceIdentityCache {
    imei: Option<String>,
    iccid: Option<String>,
    imsi: Option<String>,
    msisdn: Option<String>,
    spn: Option<String>,
    mcc: Option<String>,
    mnc: Option<String>,
    sim_status: Option<String>,
    wan_apn: Option<String>,
    wan_ip: Option<String>,
    wan_ipv6: Option<String>,
    gateway_ip: Option<String>,
    lan_domain: Option<String>,
}

impl SignalCollector {
    pub fn new(method: CollectionMethod) -> Self {
        Self {
            method,
            prev_rx: 0,
            prev_tx: 0,
            prev_time: None,
            prev_speed_source: String::new(),
            prev_cpu_idle: 0,
            prev_cpu_total: 0,
            device_identity: DeviceIdentityCache::default(),
            poll_count: 0,
            devices_cache: HashMap::new(),
            devices_missed: HashMap::new(),
            adb_ubus_available: false,
            at_available: false,
            http_available: false,
        }
    }

    /// Probe available data sources. Call once before polling.
    pub fn probe(&mut self, device: Option<&DeviceShell>, http: Option<&UbusClient>) {
        if let Some(dev) = device {
            if matches!(self.method, CollectionMethod::Ubus | CollectionMethod::Auto) {
                if let Ok(raw) = dev.shell(
                    "ubus call zte_nwinfo_api nwinfo_get_netinfo '{}' 2>/dev/null",
                    5,
                ) {
                    if raw.contains("nr5g_rsrp") || raw.contains("signalbar") {
                        self.adb_ubus_available = true;
                    }
                }
            }
            if matches!(self.method, CollectionMethod::At | CollectionMethod::Auto) {
                // AT availability is checked when needed
                self.at_available = true;
            }
        }
        if let Some(_http) = http {
            if matches!(self.method, CollectionMethod::Wifi | CollectionMethod::Auto) {
                self.http_available = true;
            }
        }
    }

    /// Return string describing active data sources.
    pub fn sources(&self) -> String {
        let mut parts = Vec::new();
        if self.adb_ubus_available {
            parts.push("ubus");
        }
        if self.at_available {
            parts.push("AT");
        }
        if self.http_available {
            parts.push("wifi");
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join("+")
        }
    }

    /// Poll all available data sources and return a snapshot.
    pub fn poll(
        &mut self,
        device: Option<&DeviceShell>,
        http: Option<&mut UbusClient>,
    ) -> SignalSnapshot {
        if self.adb_ubus_available {
            if let Some(dev) = device {
                let snap = self.poll_ubus_local(dev);
                self.poll_count += 1;
                return snap;
            }
        }
        if self.http_available {
            if let Some(http) = http {
                let snap = self.poll_http(device, http);
                self.poll_count += 1;
                return snap;
            }
        }
        self.poll_count += 1;
        SignalSnapshot::default()
    }

    /// Collect via shell ubus calls (primary method, no auth needed).
    fn poll_ubus_local(&mut self, device: &DeviceShell) -> SignalSnapshot {
        let mut snap = SignalSnapshot::default();

        // Device identity: fetch at startup and re-poll every 30 cycles
        if self.poll_count == 0 || self.poll_count % 30 == 0 {
            self.read_device_identity_shell(device);
        }

        // Signal + network info
        if let Ok(nw) = device.ubus_call("zte_nwinfo_api", "nwinfo_get_netinfo", None, 10) {
            let (nr, lte, wcdma, cops) = parse_netinfo(&nw);
            snap.nr = nr;
            snap.lte = lte;
            snap.wcdma = wcdma;
            snap.cops = cops;

            // WAN IP fallback from netinfo
            if self.device_identity.wan_ip.is_none() {
                self.device_identity.wan_ip = get_string(&nw, "wan_ipaddr");
            }
            if self.device_identity.wan_ipv6.is_none() {
                self.device_identity.wan_ipv6 = get_string(&nw, "wan_ipv6addr")
                    .or_else(|| get_string(&nw, "ipv6_wan_ipaddr"));
            }

            snap.connection.network_type = snap.cops.act.clone();
            snap.connection.active_band = get_string(&nw, "wan_active_band");
            snap.connection.provider = snap.cops.operator.clone();
            snap.connection.roaming = snap.cops.roaming.clone();
        }

        // Battery
        if let Ok(bat) = device.ubus_call("zwrt_bsp.battery", "list", None, 5) {
            snap.device.battery_pct = get_u32(&bat, "battery_capacity");
            snap.device.battery_temp = get_f64(&bat, "battery_temperature");
            snap.device.battery_time_to_full = get_i64(&bat, "battery_time_to_full");
            snap.device.battery_time_to_empty = get_i64(&bat, "battery_time_to_empty");
        }

        // Battery current via zte-companion (microamps)
        if let Ok(resp) = device.ubus_call("zte-companion", "battery_current", None, 5) {
            snap.device.battery_current_ua = resp.get("current_now").and_then(|v| v.as_i64());
        }

        // Charger
        if let Ok(charger) = device.ubus_call("zwrt_bsp.charger", "list", None, 5) {
            snap.device.charge_status = get_u32(&charger, "charge_status");
            let charger_connected = get_u32(&charger, "charger_connect") == Some(1);
            let direct_power = get_string(&charger, "direct_power_supply_mode")
                .map(|s| s.to_lowercase());
            if charger_connected && matches!(direct_power.as_deref(), Some("enable") | Some("1")) {
                snap.device.charging = Some("wall".to_string());
            } else if snap.device.charge_status == Some(1) {
                snap.device.charging = Some("charging".to_string());
            } else {
                snap.device.charging = Some("discharging".to_string());
            }
        }

        // CPU temp
        if let Ok(thermal) = device.ubus_call("zwrt_bsp.thermal", "get_cpu_temp", None, 5) {
            snap.device.cpu_temp = get_f64(&thermal, "cpuss_temp");
        }

        // Traffic speed + totals via ubus (with /proc/net/dev fallback)
        self.read_traffic_speed_shell(device, &mut snap);

        // CPU usage from /proc/stat, with zte-companion fallback
        self.read_cpu_usage_shell(device, &mut snap);
        if snap.device.cpu_usage.is_none() {
            // Fallback: try zte-companion via ubus on device
            if let Ok(resp) = device.ubus_call("zte-companion", "cpu_usage", None, 5) {
                if let (Some(idle), Some(total)) = (
                    resp.get("idle").and_then(|v| v.as_u64()),
                    resp.get("total").and_then(|v| v.as_u64()),
                ) {
                    if self.prev_cpu_total > 0 {
                        let d_total = total.saturating_sub(self.prev_cpu_total);
                        let d_idle = idle.saturating_sub(self.prev_cpu_idle);
                        if d_total > 0 {
                            let usage = (1.0 - d_idle as f64 / d_total as f64) * 100.0;
                            snap.device.cpu_usage = Some((usage * 10.0).round() / 10.0);
                        }
                    }
                    self.prev_cpu_idle = idle;
                    self.prev_cpu_total = total;
                }
            }
        }

        // Connected devices (with sticky cache to prevent flickering)
        let fresh = self.read_connected_devices_shell(device);
        snap.connected_devices = self.merge_devices_cache(fresh, 5);
        snap.connection.device_count = Some(snap.connected_devices.len() as u32);

        // WiFi status
        self.read_wifi_shell(device, &mut snap);

        // Apply cached device identity
        snap.connection.ip_addr = self.device_identity.wan_ip.clone();
        snap.connection.ipv6_addr = self.device_identity.wan_ipv6.clone();
        snap.connection.gateway_ip = self.device_identity.gateway_ip.clone();
        snap.connection.lan_domain = self.device_identity.lan_domain.clone();
        snap.connection.wan_apn = self.device_identity.wan_apn.clone();
        snap.device.imei = self.device_identity.imei.clone();
        snap.device.iccid = self.device_identity.iccid.clone();
        snap.device.imsi = self.device_identity.imsi.clone();
        snap.device.msisdn = self.device_identity.msisdn.clone();
        snap.device.spn = self.device_identity.spn.clone();
        snap.device.mcc = self.device_identity.mcc.clone();
        snap.device.mnc = self.device_identity.mnc.clone();
        snap.device.sim_status = self.device_identity.sim_status.clone();

        snap
    }

    /// Collect via HTTP ubus JSON-RPC (WiFi mode).
    fn poll_http(&mut self, device: Option<&DeviceShell>, http: &mut UbusClient) -> SignalSnapshot {
        let mut snap = SignalSnapshot::default();

        // Device identity + session refresh: at startup and every 30 cycles
        if self.poll_count == 0 || self.poll_count % 30 == 0 {
            let _ = http.relogin();
            self.read_device_identity_http(http);
        }

        // Signal + network info
        if let Ok(nw) = http.call("zte_nwinfo_api", "nwinfo_get_netinfo", None) {
            let (nr, lte, wcdma, cops) = parse_netinfo(&nw);
            snap.nr = nr;
            snap.lte = lte;
            snap.wcdma = wcdma;
            snap.cops = cops;

            if self.device_identity.wan_ip.is_none() {
                self.device_identity.wan_ip = get_string(&nw, "wan_ipaddr");
            }
            if self.device_identity.wan_ipv6.is_none() {
                self.device_identity.wan_ipv6 = get_string(&nw, "wan_ipv6addr")
                    .or_else(|| get_string(&nw, "ipv6_wan_ipaddr"));
            }

            snap.connection.network_type = snap.cops.act.clone();
            snap.connection.active_band = get_string(&nw, "wan_active_band");
            snap.connection.provider = snap.cops.operator.clone();
            snap.connection.roaming = snap.cops.roaming.clone();
        }

        // Battery
        if let Ok(bat) = http.call("zwrt_bsp.battery", "list", None) {
            snap.device.battery_pct = get_u32(&bat, "battery_capacity");
            snap.device.battery_temp = get_f64(&bat, "battery_temperature");
            snap.device.battery_time_to_full = get_i64(&bat, "battery_time_to_full");
            snap.device.battery_time_to_empty = get_i64(&bat, "battery_time_to_empty");
        }

        // Battery current via zte-companion (microamps)
        if let Ok(resp) = http.call("zte-companion", "battery_current", None) {
            snap.device.battery_current_ua = resp.get("current_now").and_then(|v| v.as_i64());
        }

        // Charger
        if let Ok(charger) = http.call("zwrt_bsp.charger", "list", None) {
            snap.device.charge_status = get_u32(&charger, "charge_status");
            let charger_connected = get_u32(&charger, "charger_connect") == Some(1);
            let direct_power = get_string(&charger, "direct_power_supply_mode")
                .map(|s| s.to_lowercase());
            if charger_connected && matches!(direct_power.as_deref(), Some("enable") | Some("1")) {
                snap.device.charging = Some("wall".to_string());
            } else if snap.device.charge_status == Some(1) {
                snap.device.charging = Some("charging".to_string());
            } else {
                snap.device.charging = Some("discharging".to_string());
            }
        }

        // CPU temp
        if let Ok(thermal) = http.call("zwrt_bsp.thermal", "get_cpu_temp", None) {
            snap.device.cpu_temp = get_f64(&thermal, "cpuss_temp");
        }

        // Traffic stats via HTTP
        self.read_traffic_speed_http(http, &mut snap);

        // Connected devices via HTTP (with sticky cache to prevent flickering)
        let fresh = self.read_connected_devices_http(http);
        snap.connected_devices = self.merge_devices_cache(fresh, 5);
        snap.connection.device_count = Some(snap.connected_devices.len() as u32);

        // WiFi status via HTTP
        self.read_wifi_http(http, &mut snap);

        // Apply cached device identity
        snap.connection.ip_addr = self.device_identity.wan_ip.clone();
        snap.connection.ipv6_addr = self.device_identity.wan_ipv6.clone();
        snap.connection.gateway_ip = self.device_identity.gateway_ip.clone();
        snap.connection.lan_domain = self.device_identity.lan_domain.clone();
        snap.connection.wan_apn = self.device_identity.wan_apn.clone();
        snap.device.imei = self.device_identity.imei.clone();
        snap.device.iccid = self.device_identity.iccid.clone();
        snap.device.imsi = self.device_identity.imsi.clone();
        snap.device.msisdn = self.device_identity.msisdn.clone();
        snap.device.spn = self.device_identity.spn.clone();
        snap.device.mcc = self.device_identity.mcc.clone();
        snap.device.mnc = self.device_identity.mnc.clone();
        snap.device.sim_status = self.device_identity.sim_status.clone();

        // CPU usage: prefer /proc/stat via shell, then zte-companion, then file.read
        if let Some(dev) = device {
            self.read_cpu_usage_shell(dev, &mut snap);
        } else if !self.read_cpu_usage_companion(http, &mut snap) {
            self.read_cpu_usage_http(http, &mut snap);
        }

        snap
    }

    // --- Internal collection methods ---

    fn read_device_identity_shell(&mut self, device: &DeviceShell) {
        // SIM info
        if let Ok(sim) = device.ubus_call("zwrt_zte_mdm.api", "get_sim_info", None, 5) {
            self.device_identity.iccid = get_string(&sim, "sim_iccid");
            self.device_identity.imsi = get_string(&sim, "sim_imsi");
            self.device_identity.msisdn = get_string(&sim, "msisdn");
            self.device_identity.mcc = get_string(&sim, "mdm_mcc");
            self.device_identity.mnc = get_string(&sim, "mdm_mnc");
            self.device_identity.sim_status = get_string(&sim, "sim_states");
            if let Some(hex) = get_string(&sim, "spn_name_data") {
                self.device_identity.spn = decode_spn(&hex);
            }
        }

        // APN
        if let Ok(raw) = device.shell(
            "uci get network.zte_wan.apn 2>/dev/null",
            3,
        ) {
            let apn = raw.trim().to_string();
            if !apn.is_empty() && !apn.to_lowercase().contains("not found") {
                self.device_identity.wan_apn = Some(apn);
            }
        }

        // IMEI
        if let Ok(imei_resp) = device.ubus_call("zwrt_zte_mdm.api", "get_imei", None, 5) {
            self.device_identity.imei = get_string(&imei_resp, "imei");
        }

        // WAN IPv4
        if let Ok(wan) = device.ubus_call("network.interface.zte_wan", "status", None, 5) {
            if let Some(addrs) = wan.get("ipv4-address").and_then(|v| v.as_array()) {
                if let Some(first) = addrs.first() {
                    self.device_identity.wan_ip = get_string(first, "address");
                }
            }
        }

        // WAN IPv6
        if let Ok(wan6) = device.ubus_call("network.interface.zte_wan6", "status", None, 5) {
            if let Some(addrs) = wan6.get("ipv6-address").and_then(|v| v.as_array()) {
                for a in addrs {
                    if let Some(addr) = get_string(a, "address") {
                        if !addr.starts_with("fe80") {
                            self.device_identity.wan_ipv6 = Some(addr);
                            break;
                        }
                    }
                }
            }
        }

        // Gateway IP
        if let Ok(lan) = device.ubus_call("network.interface.lan", "status", None, 5) {
            if let Some(addrs) = lan.get("ipv4-address").and_then(|v| v.as_array()) {
                if let Some(first) = addrs.first() {
                    self.device_identity.gateway_ip = get_string(first, "address");
                }
            }
        }

        // LAN domain
        if let Ok(raw) = device.shell(
            "uci get zwrt_router.dnsmasq.localdomain 2>/dev/null",
            3,
        ) {
            let domain = raw.trim().to_string();
            if !domain.is_empty() && !domain.to_lowercase().contains("not found") {
                self.device_identity.lan_domain = Some(domain);
            }
        }
    }

    fn read_device_identity_http(&mut self, http: &UbusClient) {
        if let Ok(sim) = http.call("zwrt_zte_mdm.api", "get_sim_info", None) {
            self.device_identity.iccid = get_string(&sim, "sim_iccid");
            self.device_identity.imsi = get_string(&sim, "sim_imsi");
            self.device_identity.msisdn = get_string(&sim, "msisdn");
            self.device_identity.mcc = get_string(&sim, "mdm_mcc");
            self.device_identity.mnc = get_string(&sim, "mdm_mnc");
            self.device_identity.sim_status = get_string(&sim, "sim_states");
            if let Some(hex) = get_string(&sim, "spn_name_data") {
                self.device_identity.spn = decode_spn(&hex);
            }
        }

        // APN via HTTP
        if let Ok(apn_resp) = http.call("zwrt_web", "web_api_telus_para_get", None) {
            if let Some(apn) = get_string(&apn_resp, "wan_apn") {
                self.device_identity.wan_apn = Some(apn);
            }
        }
        if let Ok(imei_resp) = http.call("zwrt_zte_mdm.api", "get_imei", None) {
            self.device_identity.imei = get_string(&imei_resp, "imei");
        }
        if let Ok(wan) = http.call("network.interface.zte_wan", "status", None) {
            if let Some(addrs) = wan.get("ipv4-address").and_then(|v| v.as_array()) {
                if let Some(first) = addrs.first() {
                    self.device_identity.wan_ip = get_string(first, "address");
                }
            }
        }
        if let Ok(wan6) = http.call("network.interface.zte_wan6", "status", None) {
            if let Some(addrs) = wan6.get("ipv6-address").and_then(|v| v.as_array()) {
                for a in addrs {
                    if let Some(addr) = get_string(a, "address") {
                        if !addr.starts_with("fe80") {
                            self.device_identity.wan_ipv6 = Some(addr);
                            break;
                        }
                    }
                }
            }
        }
        if let Ok(lan) = http.call("network.interface.lan", "status", None) {
            if let Some(addrs) = lan.get("ipv4-address").and_then(|v| v.as_array()) {
                if let Some(first) = addrs.first() {
                    self.device_identity.gateway_ip = get_string(first, "address");
                }
            }
        }
        // Gateway fallback: the IP we connected to
        if self.device_identity.gateway_ip.is_none() {
            self.device_identity.gateway_ip = Some(http.gateway.clone());
        }
        // LAN domain via uci
        if let Ok(uci_resp) = http.call(
            "uci",
            "get",
            Some(&serde_json::json!({
                "config": "zwrt_router",
                "section": "dnsmasq",
                "option": "localdomain"
            })),
        ) {
            if let Some(domain) = uci_resp.get("value").and_then(|v| v.as_str()) {
                if !domain.is_empty() {
                    self.device_identity.lan_domain = Some(domain.to_string());
                }
            }
        }
    }

    fn read_traffic_speed_shell(&mut self, device: &DeviceShell, snap: &mut SignalSnapshot) {
        let mut rx_bytes: Option<u64> = None;
        let mut tx_bytes: Option<u64> = None;
        let mut source = String::new();
        let mut precomputed_rates: Option<(f64, f64)> = None;

        // Primary: zwrt_data get_wwandst (modem-level counters, matches router web UI)
        if let Ok(dst) = device.ubus_call("zwrt_data", "get_wwandst", None, 5) {
            // Check for pre-computed rate fields first (ignore zeros — fall through to delta calc)
            if let (Some(rx_rate), Some(tx_rate)) =
                (get_f64(&dst, "real_rx_speed"), get_f64(&dst, "real_tx_speed"))
            {
                if rx_rate > 0.0 || tx_rate > 0.0 {
                    precomputed_rates = Some((rx_rate, tx_rate));
                }
            }
            let rx = get_u64(&dst, "real_rx_bytes");
            let tx = get_u64(&dst, "real_tx_bytes");
            if rx.is_some() {
                rx_bytes = rx;
                tx_bytes = tx;
                source = "wwandst".to_string();
            }
        }

        // Fallback 1: network.device status via ubus
        if rx_bytes.is_none() {
            let params = serde_json::json!({"name": "rmnet_data0"});
            if let Ok(resp) = device.ubus_call("network.device", "status", Some(&params), 5) {
                if let Some(stats) = resp.get("statistics") {
                    rx_bytes = get_u64(stats, "rx_bytes");
                    tx_bytes = get_u64(stats, "tx_bytes");
                    if rx_bytes.is_some() {
                        source = "rmnet_ubus".to_string();
                    }
                }
            }
        }

        // Fallback 2: /proc/net/dev
        if rx_bytes.is_none() {
            if let Ok(raw) = device.shell("cat /proc/net/dev", 3) {
                for line in raw.lines() {
                    if !line.contains("rmnet_data0") { continue; }
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() < 2 { break; }
                    let fields: Vec<&str> = parts[1].split_whitespace().collect();
                    if fields.len() < 10 { break; }
                    rx_bytes = fields[0].parse().ok();
                    tx_bytes = fields[8].parse().ok();
                    if rx_bytes.is_some() {
                        source = "proc_net".to_string();
                    }
                    break;
                }
            }
        }

        if let Some(rx) = rx_bytes {
            let tx = tx_bytes.unwrap_or(0);
            snap.connection.rx_bytes = Some(rx);
            snap.connection.tx_bytes = tx_bytes;

            // Use pre-computed rates if available (already in bytes/sec)
            if let Some((rx_rate, tx_rate)) = precomputed_rates {
                snap.connection.dl_speed_mbps =
                    Some((rx_rate * 8.0 / 1_000_000.0 * 10.0).round() / 10.0);
                snap.connection.ul_speed_mbps =
                    Some((tx_rate * 8.0 / 1_000_000.0 * 10.0).round() / 10.0);
                snap.connection.raw_rx_rate = Some(rx_rate);
                snap.connection.raw_tx_rate = Some(tx_rate);
                snap.connection.speed_source = Some(source.clone());
            } else {
                let now = Instant::now();
                // Skip delta when source changes to avoid invalid spikes
                if let Some(prev_time) = self.prev_time {
                    if source == self.prev_speed_source {
                        let elapsed = now.duration_since(prev_time).as_secs_f64();
                        if elapsed > 0.0 {
                            let dl_bps = (rx.saturating_sub(self.prev_rx)) as f64 / elapsed;
                            let ul_bps = (tx.saturating_sub(self.prev_tx)) as f64 / elapsed;
                            snap.connection.dl_speed_mbps =
                                Some((dl_bps * 8.0 / 1_000_000.0 * 10.0).round() / 10.0);
                            snap.connection.ul_speed_mbps =
                                Some((ul_bps * 8.0 / 1_000_000.0 * 10.0).round() / 10.0);
                            snap.connection.raw_rx_rate = Some(dl_bps);
                            snap.connection.raw_tx_rate = Some(ul_bps);
                            snap.connection.speed_source =
                                Some(format!("{source}_delta"));
                        }
                    }
                }
                self.prev_time = Some(now);
            }

            self.prev_rx = rx;
            self.prev_tx = tx;
            self.prev_speed_source = source;
        }
    }

    fn read_traffic_speed_http(&mut self, http: &UbusClient, snap: &mut SignalSnapshot) {
        let mut rx_bytes: Option<u64> = None;
        let mut tx_bytes: Option<u64> = None;
        let mut source = String::new();
        let mut precomputed_rates: Option<(f64, f64)> = None;

        // Primary: zwrt_data get_wwandst (modem-level counters, matches router web UI)
        // Try auth session first, then anon (probe shows it's anon-accessible)
        let dst = http
            .call("zwrt_data", "get_wwandst", None)
            .ok()
            .filter(|v| !v.is_null())
            .or_else(|| {
                http.call_anon("zwrt_data", "get_wwandst", None)
                    .ok()
                    .filter(|v| !v.is_null())
            });
        if let Some(dst) = dst {
            // Check for pre-computed rate fields first (ignore zeros — fall through to delta calc)
            if let (Some(rx_rate), Some(tx_rate)) =
                (get_f64(&dst, "real_rx_speed"), get_f64(&dst, "real_tx_speed"))
            {
                if rx_rate > 0.0 || tx_rate > 0.0 {
                    precomputed_rates = Some((rx_rate, tx_rate));
                }
            }
            let rx = get_u64(&dst, "real_rx_bytes");
            let tx = get_u64(&dst, "real_tx_bytes");
            if rx.is_some() {
                rx_bytes = rx;
                tx_bytes = tx;
                source = "wwandst".to_string();
            }
        }

        // Fallback 1: network.device status (rmnet_data0)
        if rx_bytes.is_none() {
            let params = serde_json::json!({"name": "rmnet_data0"});
            if let Ok(resp) = http.call("network.device", "status", Some(&params)) {
                if let Some(stats) = resp.get("statistics") {
                    rx_bytes = get_u64(stats, "rx_bytes");
                    tx_bytes = get_u64(stats, "tx_bytes");
                    if rx_bytes.is_some() {
                        source = "rmnet_ubus".to_string();
                    }
                }
            }
        }

        // Fallback 2: luci-rpc getNetworkDevices (device-level traffic stats)
        if rx_bytes.is_none() {
            if let Ok(devs) = http.call("luci-rpc", "getNetworkDevices", None) {
                if let Some(obj) = devs.as_object() {
                    let wan_dev = obj.get("rmnet_data0").or_else(|| {
                        obj.keys()
                            .find(|k| k.contains("rmnet") || k.contains("wwan"))
                            .and_then(|k| obj.get(k))
                    });
                    if let Some(dev) = wan_dev {
                        if let Some(stats) = dev.get("stats").or_else(|| dev.get("statistics")) {
                            rx_bytes = get_u64(stats, "rx_bytes");
                            tx_bytes = get_u64(stats, "tx_bytes");
                            if rx_bytes.is_some() {
                                source = "luci_netdev".to_string();
                            }
                        }
                    }
                }
            }
        }

        // Fallback 3: discover WAN device from interface, then query network.device
        if rx_bytes.is_none() {
            if let Ok(wan_status) = http.call("network.interface.zte_wan", "status", None) {
                let dev_name = wan_status
                    .get("l3_device")
                    .or_else(|| wan_status.get("device"))
                    .and_then(|v| v.as_str());
                if let Some(name) = dev_name {
                    let params = serde_json::json!({"name": name});
                    if let Ok(resp) = http.call("network.device", "status", Some(&params)) {
                        if let Some(stats) = resp.get("statistics") {
                            rx_bytes = get_u64(stats, "rx_bytes");
                            tx_bytes = get_u64(stats, "tx_bytes");
                            if rx_bytes.is_some() {
                                source = "wan_dev".to_string();
                            }
                        }
                    }
                }
            }
        }

        if let Some(rx) = rx_bytes {
            snap.connection.rx_bytes = Some(rx);
            snap.connection.tx_bytes = tx_bytes;
            let tx = tx_bytes.unwrap_or(0);

            // Use pre-computed rates if available (already in bytes/sec)
            if let Some((rx_rate, tx_rate)) = precomputed_rates {
                snap.connection.dl_speed_mbps =
                    Some((rx_rate * 8.0 / 1_000_000.0 * 10.0).round() / 10.0);
                snap.connection.ul_speed_mbps =
                    Some((tx_rate * 8.0 / 1_000_000.0 * 10.0).round() / 10.0);
                snap.connection.raw_rx_rate = Some(rx_rate);
                snap.connection.raw_tx_rate = Some(tx_rate);
                snap.connection.speed_source = Some(source.clone());
            } else {
                let now = Instant::now();
                // Skip delta when source changes to avoid invalid spikes
                if let Some(prev_time) = self.prev_time {
                    if source == self.prev_speed_source {
                        let elapsed = now.duration_since(prev_time).as_secs_f64();
                        if elapsed > 0.0 {
                            let dl_bps = (rx.saturating_sub(self.prev_rx)) as f64 / elapsed;
                            let ul_bps = (tx.saturating_sub(self.prev_tx)) as f64 / elapsed;
                            snap.connection.dl_speed_mbps =
                                Some((dl_bps * 8.0 / 1_000_000.0 * 10.0).round() / 10.0);
                            snap.connection.ul_speed_mbps =
                                Some((ul_bps * 8.0 / 1_000_000.0 * 10.0).round() / 10.0);
                            snap.connection.raw_rx_rate = Some(dl_bps);
                            snap.connection.raw_tx_rate = Some(ul_bps);
                            snap.connection.speed_source =
                                Some(format!("{source}_delta"));
                        }
                    }
                }
                self.prev_time = Some(now);
            }

            self.prev_rx = rx;
            self.prev_tx = tx;
            self.prev_speed_source = source;
        }
    }

    fn read_cpu_usage_shell(&mut self, device: &DeviceShell, snap: &mut SignalSnapshot) {
        let raw = match device.shell("cat /proc/stat", 3) {
            Ok(r) => r,
            Err(_) => return,
        };

        for line in raw.lines() {
            if !line.starts_with("cpu ") {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 5 {
                break;
            }
            let values: Vec<u64> = fields[1..]
                .iter()
                .filter_map(|f| f.parse().ok())
                .collect();
            if values.len() < 4 {
                break;
            }
            let mut idle = values[3]; // idle
            if values.len() > 4 {
                idle += values[4]; // iowait
            }
            let total: u64 = values.iter().sum();

            if self.prev_cpu_total > 0 {
                let d_total = total.saturating_sub(self.prev_cpu_total);
                let d_idle = idle.saturating_sub(self.prev_cpu_idle);
                if d_total > 0 {
                    let usage = (1.0 - d_idle as f64 / d_total as f64) * 100.0;
                    snap.device.cpu_usage = Some((usage * 10.0).round() / 10.0);
                }
            }

            self.prev_cpu_idle = idle;
            self.prev_cpu_total = total;
            break;
        }
    }

    /// Read CPU usage via `zte-companion.cpu_usage` rpcd plugin.
    /// Returns `true` if successful, `false` to fall back to `file.read`.
    fn read_cpu_usage_companion(&mut self, http: &UbusClient, snap: &mut SignalSnapshot) -> bool {
        let resp = match http.call("zte-companion", "cpu_usage", None) {
            Ok(r) => r,
            Err(_) => return false,
        };

        let idle = match resp.get("idle").and_then(|v| v.as_u64()) {
            Some(v) => v,
            None => return false,
        };
        let total = match resp.get("total").and_then(|v| v.as_u64()) {
            Some(v) => v,
            None => return false,
        };

        if self.prev_cpu_total > 0 {
            let d_total = total.saturating_sub(self.prev_cpu_total);
            let d_idle = idle.saturating_sub(self.prev_cpu_idle);
            if d_total > 0 {
                let usage = (1.0 - d_idle as f64 / d_total as f64) * 100.0;
                snap.device.cpu_usage = Some((usage * 10.0).round() / 10.0);
            }
        }

        self.prev_cpu_idle = idle;
        self.prev_cpu_total = total;
        true
    }

    /// Read CPU usage from `/proc/stat` via ubus `file.read` (HTTP-only mode).
    /// Requires `file.read` in the ACL — run `zte acl patch` to grant access.
    fn read_cpu_usage_http(&mut self, http: &UbusClient, snap: &mut SignalSnapshot) {
        let params = serde_json::json!({
            "path": "/proc/stat",
            "base64": false,
            "ubus_rpc_session": http.session(),
        });
        let raw = match http.call("file", "read", Some(&params)) {
            Ok(resp) => match resp.get("data").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return,
            },
            Err(_) => return,
        };

        for line in raw.lines() {
            if !line.starts_with("cpu ") {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 5 {
                break;
            }
            let values: Vec<u64> = fields[1..]
                .iter()
                .filter_map(|f| f.parse().ok())
                .collect();
            if values.len() < 4 {
                break;
            }
            let mut idle = values[3];
            if values.len() > 4 {
                idle += values[4]; // iowait
            }
            let total: u64 = values.iter().sum();

            if self.prev_cpu_total > 0 {
                let d_total = total.saturating_sub(self.prev_cpu_total);
                let d_idle = idle.saturating_sub(self.prev_cpu_idle);
                if d_total > 0 {
                    let usage = (1.0 - d_idle as f64 / d_total as f64) * 100.0;
                    snap.device.cpu_usage = Some((usage * 10.0).round() / 10.0);
                }
            }

            self.prev_cpu_idle = idle;
            self.prev_cpu_total = total;
            break;
        }
    }

    fn read_connected_devices_shell(&self, device: &DeviceShell) -> Vec<ConnectedDevice> {
        let mut devices: HashMap<String, ConnectedDevice> = HashMap::new();

        // 1. DHCP leases
        if let Ok(raw) = device.shell("cat /tmp/dhcp.leases 2>/dev/null", 3) {
            for line in raw.lines() {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 4 {
                    let mac = parts[1].to_lowercase();
                    let ipv4 = parts[2].to_string();
                    let hostname = if parts[3] != "*" {
                        parts[3].to_string()
                    } else {
                        String::new()
                    };
                    let entry = devices.entry(mac.clone()).or_insert_with(|| {
                        ConnectedDevice {
                            mac: mac.clone(),
                            ..Default::default()
                        }
                    });
                    if !hostname.is_empty() {
                        entry.hostname = hostname;
                    }
                    if !ipv4.is_empty() {
                        entry.ipv4 = ipv4;
                    }
                }
            }
        }

        // 2. ip neigh show dev br-lan
        if let Ok(raw) = device.shell("ip neigh show dev br-lan 2>/dev/null", 3) {
            for line in raw.lines() {
                let line = line.trim();
                if line.is_empty() || line.contains("FAILED") {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 {
                    continue;
                }
                let ip_addr = parts[0].to_string();
                let mac = match parts.iter().position(|&p| p == "lladdr") {
                    Some(idx) if idx + 1 < parts.len() => parts[idx + 1].to_lowercase(),
                    _ => continue,
                };
                let entry = devices.entry(mac.clone()).or_insert_with(|| {
                    ConnectedDevice {
                        mac: mac.clone(),
                        ..Default::default()
                    }
                });
                if ip_addr.contains(':') && ip_addr.matches(':').count() >= 2 {
                    // IPv6
                    if !entry.ipv6.contains(&ip_addr) {
                        entry.ipv6.push(ip_addr);
                    }
                } else if entry.ipv4.is_empty() {
                    entry.ipv4 = ip_addr;
                }
            }
        }

        // 3. /proc/net/arp fallback
        if let Ok(raw) = device.shell("cat /proc/net/arp 2>/dev/null", 3) {
            for line in raw.lines() {
                if line.starts_with("IP") || !line.contains("br-lan") {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let ipv4 = parts[0].to_string();
                    let mac = parts[3].to_lowercase();
                    if mac == "00:00:00:00:00:00" {
                        continue;
                    }
                    let entry = devices.entry(mac.clone()).or_insert_with(|| {
                        ConnectedDevice {
                            mac: mac.clone(),
                            ..Default::default()
                        }
                    });
                    if entry.ipv4.is_empty() {
                        entry.ipv4 = ipv4;
                    }
                }
            }
        }

        devices.into_values().collect()
    }

    fn read_connected_devices_http(&self, http: &UbusClient) -> Vec<ConnectedDevice> {
        let mut devices: HashMap<String, ConnectedDevice> = HashMap::new();

        // luci-rpc getHostHints
        if let Ok(hints) = http.call("luci-rpc", "getHostHints", None) {
            if let Some(obj) = hints.as_object() {
                for (mac, info) in obj {
                    let mac_lower = mac.to_lowercase();
                    let mut dev = ConnectedDevice {
                        mac: mac_lower.clone(),
                        hostname: info
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        ..Default::default()
                    };
                    if let Some(addrs) = info.get("ipaddrs").and_then(|v| v.as_array()) {
                        if let Some(first) = addrs.first().and_then(|v| v.as_str()) {
                            dev.ipv4 = first.to_string();
                        }
                    }
                    if let Some(addrs) = info.get("ip6addrs").and_then(|v| v.as_array()) {
                        for a in addrs {
                            if let Some(addr) = a.as_str() {
                                dev.ipv6.push(addr.to_string());
                            }
                        }
                    }
                    devices.insert(mac_lower, dev);
                }
            }
        }

        // Enrich from DHCP leases
        let params = serde_json::json!({"family": 4});
        if let Ok(leases_resp) = http.call("luci-rpc", "getDHCPLeases", Some(&params)) {
            let leases = if let Some(obj) = leases_resp.as_object() {
                obj.get("dhcp_leases")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
            } else if let Some(arr) = leases_resp.as_array() {
                arr.clone()
            } else {
                vec![]
            };
            for lease in &leases {
                let mac = lease
                    .get("macaddr")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                let hostname = lease
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(dev) = devices.get_mut(&mac) {
                    if !hostname.is_empty() && dev.hostname.is_empty() {
                        dev.hostname = hostname;
                    }
                }
            }
        }

        devices.into_values().collect()
    }

    /// Merge fresh device list into sticky cache; keep stale entries for `max_misses` polls.
    fn merge_devices_cache(&mut self, fresh: Vec<ConnectedDevice>, max_misses: u32) -> Vec<ConnectedDevice> {
        let mut seen = std::collections::HashSet::new();
        for dev in fresh {
            let mac = dev.mac.clone();
            seen.insert(mac.clone());
            self.devices_missed.insert(mac.clone(), 0);
            self.devices_cache.insert(mac, dev);
        }

        let stale: Vec<String> = self
            .devices_missed
            .keys()
            .filter(|mac| !seen.contains(mac.as_str()))
            .cloned()
            .collect();
        for mac in stale {
            let count = self.devices_missed.entry(mac.clone()).or_insert(0);
            *count += 1;
            if *count > max_misses {
                self.devices_cache.remove(&mac);
                self.devices_missed.remove(&mac);
            }
        }

        let mut result: Vec<ConnectedDevice> = self.devices_cache.values().cloned().collect();
        result.sort_by(|a, b| a.mac.cmp(&b.mac));
        result
    }

    fn read_wifi_shell(&self, device: &DeviceShell, snap: &mut SignalSnapshot) {
        // iface_report: per-interface SSID/encryption/hidden
        if let Ok(iface_rep) = device.ubus_call("zwrt_wlan", "iface_report", None, 5) {
            if let Some(ifaces) = iface_rep.get("ifaces").and_then(|v| v.as_array()) {
                for iface in ifaces {
                    let sec = iface
                        .get("section_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    match sec {
                        "main_2g" => {
                            snap.wifi.ssid_2g = get_string(iface, "ssid");
                            snap.wifi.encryption_2g = get_string(iface, "encryption");
                            snap.wifi.hidden_2g = get_string(iface, "hidden");
                        }
                        "main_5g" => {
                            snap.wifi.ssid_5g = get_string(iface, "ssid");
                            snap.wifi.encryption_5g = get_string(iface, "encryption");
                            snap.wifi.hidden_5g = get_string(iface, "hidden");
                        }
                        _ => {}
                    }
                }
            }
        }

        // report: wifi_onoff, radio disabled
        if let Ok(report) = device.ubus_call("zwrt_wlan", "report", None, 5) {
            snap.wifi.wifi_onoff = get_string(&report, "wifi_onoff");
            snap.wifi.radio_2g_disabled = get_string(&report, "radio2_disabled");
            snap.wifi.radio_5g_disabled = get_string(&report, "radio5_disabled");
        }

        // wifi6 from zte_mbb section
        let zte_mbb_params = serde_json::json!({"section": "zte_mbb"});
        if let Ok(zte_mbb) =
            device.ubus_call("zwrt_wlan", "wlan_uci_get_section", Some(&zte_mbb_params), 5)
        {
            snap.wifi.wifi6 = get_string(&zte_mbb, "wifi6_switch");
        }

        // Per-radio channel + txpower from UCI
        let wifi0_params = serde_json::json!({"section": "wifi0"});
        let wifi1_params = serde_json::json!({"section": "wifi1"});
        if let Ok(wifi0) =
            device.ubus_call("zwrt_wlan", "wlan_uci_get_section", Some(&wifi0_params), 5)
        {
            snap.wifi.channel_2g = get_string(&wifi0, "channel");
            if snap.wifi.radio_2g_disabled.is_none() {
                snap.wifi.radio_2g_disabled = get_string(&wifi0, "disabled");
            }
            if snap.wifi.txpower_2g.is_none() {
                snap.wifi.txpower_2g = get_string(&wifi0, "txpowerpercent");
            }
        }
        if let Ok(wifi1) =
            device.ubus_call("zwrt_wlan", "wlan_uci_get_section", Some(&wifi1_params), 5)
        {
            snap.wifi.channel_5g = get_string(&wifi1, "channel");
            if snap.wifi.radio_5g_disabled.is_none() {
                snap.wifi.radio_5g_disabled = get_string(&wifi1, "disabled");
            }
            if snap.wifi.txpower_5g.is_none() {
                snap.wifi.txpower_5g = get_string(&wifi1, "txpowerpercent");
            }
        }

        // Client count via iw
        let mut clients_2g: u32 = 0;
        let mut clients_5g: u32 = 0;
        if let Ok(iw_raw) = device.shell("iw dev 2>/dev/null", 5) {
            let mut current_iface = String::new();
            let mut iface_band: HashMap<String, String> = HashMap::new();

            for line in iw_raw.lines() {
                let stripped = line.trim();
                if stripped.starts_with("Interface ") {
                    current_iface = stripped
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("")
                        .to_string();
                } else if stripped.to_lowercase().contains("channel")
                    && stripped.contains("MHz")
                    && !current_iface.is_empty()
                {
                    if let Some(caps) = regex::Regex::new(r"channel\s+(\d+)\s+\((\d+)\s*MHz\)")
                        .ok()
                        .and_then(|re| re.captures(stripped))
                    {
                        let ch = caps[1].to_string();
                        let freq: u32 = caps[2].parse().unwrap_or(0);
                        let band = if freq < 3000 { "2g" } else { "5g" };
                        iface_band.insert(current_iface.clone(), band.to_string());
                        // Override auto channel with actual
                        if band == "2g" {
                            snap.wifi.channel_2g = Some(ch);
                        } else {
                            snap.wifi.channel_5g = Some(ch);
                        }
                        current_iface.clear();
                    }
                }
            }

            for (iface, band) in &iface_band {
                if let Ok(count_raw) = device.shell(
                    &format!(
                        "iw dev {iface} station dump 2>/dev/null | grep -c '^Station'"
                    ),
                    5,
                ) {
                    let count: u32 = count_raw.trim().parse().unwrap_or(0);
                    if band == "2g" {
                        clients_2g += count;
                    } else {
                        clients_5g += count;
                    }
                }
            }
        }
        snap.wifi.clients_2g = Some(clients_2g);
        snap.wifi.clients_5g = Some(clients_5g);
    }

    fn read_wifi_http(&self, http: &UbusClient, snap: &mut SignalSnapshot) {
        // iface_report
        if let Ok(iface_rep) = http.call("zwrt_wlan", "iface_report", None) {
            if let Some(ifaces) = iface_rep.get("ifaces").and_then(|v| v.as_array()) {
                for iface in ifaces {
                    let sec = iface
                        .get("section_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    match sec {
                        "main_2g" => {
                            snap.wifi.ssid_2g = get_string(iface, "ssid");
                            snap.wifi.encryption_2g = get_string(iface, "encryption");
                            snap.wifi.hidden_2g = get_string(iface, "hidden");
                        }
                        "main_5g" => {
                            snap.wifi.ssid_5g = get_string(iface, "ssid");
                            snap.wifi.encryption_5g = get_string(iface, "encryption");
                            snap.wifi.hidden_5g = get_string(iface, "hidden");
                        }
                        _ => {}
                    }
                }
            }
        }

        // report
        if let Ok(report) = http.call("zwrt_wlan", "report", None) {
            snap.wifi.wifi_onoff = get_string(&report, "wifi_onoff");
            snap.wifi.radio_2g_disabled = get_string(&report, "radio2_disabled");
            snap.wifi.radio_5g_disabled = get_string(&report, "radio5_disabled");
        }

        // wifi6
        let zte_mbb_params = serde_json::json!({"section": "zte_mbb"});
        if let Ok(zte_mbb) = http.call("zwrt_wlan", "wlan_uci_get_section", Some(&zte_mbb_params))
        {
            snap.wifi.wifi6 = get_string(&zte_mbb, "wifi6_switch");
        }

        // Per-radio
        let wifi0_params = serde_json::json!({"section": "wifi0"});
        let wifi1_params = serde_json::json!({"section": "wifi1"});
        if let Ok(wifi0) = http.call("zwrt_wlan", "wlan_uci_get_section", Some(&wifi0_params)) {
            snap.wifi.channel_2g = get_string(&wifi0, "channel");
            if snap.wifi.radio_2g_disabled.is_none() {
                snap.wifi.radio_2g_disabled = get_string(&wifi0, "disabled");
            }
            if snap.wifi.txpower_2g.is_none() {
                snap.wifi.txpower_2g = get_string(&wifi0, "txpowerpercent");
            }
        }
        if let Ok(wifi1) = http.call("zwrt_wlan", "wlan_uci_get_section", Some(&wifi1_params)) {
            snap.wifi.channel_5g = get_string(&wifi1, "channel");
            if snap.wifi.radio_5g_disabled.is_none() {
                snap.wifi.radio_5g_disabled = get_string(&wifi1, "disabled");
            }
            if snap.wifi.txpower_5g.is_none() {
                snap.wifi.txpower_5g = get_string(&wifi1, "txpowerpercent");
            }
        }

        // Total clients via assoc_info (no per-band breakdown via HTTP)
        if let Ok(assoc) = http.call("zwrt_wlan", "get_assoc_info", None) {
            let total = get_u32(&assoc, "assoc_num").unwrap_or(0);
            snap.wifi.clients_total = Some(total);
        }
        snap.wifi.clients_2g = Some(0);
        snap.wifi.clients_5g = Some(0);
    }
}

/// Backward-compatible convenience function: poll via shell ubus.
pub fn collect_ubus_local(device: &DeviceShell) -> SignalSnapshot {
    let mut collector = SignalCollector::new(CollectionMethod::Ubus);
    collector.adb_ubus_available = true;
    collector.poll(Some(device), None)
}
