use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use zte_lib::signal::types::SignalSnapshot;

pub struct CsvLogger {
    writer: File,
    has_header: bool,
}

const HEADER: &str = "\
timestamp,\
nr_rsrp,nr_rsrq,nr_sinr,nr_band,nr_pci,nr_cell_id,nr_arfcn,nr_bandwidth,nr_rssi,nr_ca,\
lte_rsrp,lte_rsrq,lte_sinr,lte_band,lte_pci,lte_earfcn,lte_rssi,\
operator,act,\
battery,battery_current_ua,battery_voltage_mv,cpu_temp,cpu_usage,\
dl_speed_mbps,ul_speed_mbps,dl_total_bytes,ul_total_bytes,\
wifi_2g_enabled,wifi_5g_enabled,wifi_clients_2g,wifi_clients_5g";

impl CsvLogger {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let exists = Path::new(path).exists();
        let writer = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            writer,
            has_header: exists,
        })
    }

    pub fn log(&mut self, snap: &SignalSnapshot) -> std::io::Result<()> {
        if !self.has_header {
            writeln!(self.writer, "{HEADER}")?;
            self.has_header = true;
        }
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");

        let wifi_off = matches!(
            snap.wifi.wifi_onoff.as_deref(),
            Some("0") | Some("false") | Some("off")
        );
        let wifi_2g_enabled =
            !wifi_off && snap.wifi.radio_2g_disabled.as_deref() != Some("1");
        let wifi_5g_enabled =
            !wifi_off && snap.wifi.radio_5g_disabled.as_deref() != Some("1");

        writeln!(
            self.writer,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            ts,
            opt_f64(snap.nr.rsrp),
            opt_f64(snap.nr.rsrq),
            opt_f64(snap.nr.sinr),
            opt_str(&snap.nr.band),
            opt_str(&snap.nr.pci),
            opt_str(&snap.nr.cell_id),
            opt_str(&snap.nr.earfcn),
            opt_str(&snap.nr.bandwidth),
            opt_f64(snap.nr.rssi),
            opt_str(&snap.nr.ca_status),
            opt_f64(snap.lte.rsrp),
            opt_f64(snap.lte.rsrq),
            opt_f64(snap.lte.sinr),
            opt_str(&snap.lte.band),
            opt_str(&snap.lte.pci),
            opt_str(&snap.lte.earfcn),
            opt_f64(snap.lte.rssi),
            opt_str(&snap.cops.operator),
            opt_str(&snap.cops.act),
            snap.device.battery_pct.map(|v| v.to_string()).unwrap_or_default(),
            snap.device.battery_current_ua.map(|v| v.to_string()).unwrap_or_default(),
            snap.device.battery_voltage_mv.map(|v| v.to_string()).unwrap_or_default(),
            opt_f64(snap.device.cpu_temp),
            opt_f64(snap.device.cpu_usage),
            opt_f64(snap.connection.dl_speed_mbps),
            opt_f64(snap.connection.ul_speed_mbps),
            snap.connection.rx_bytes.map(|v| v.to_string()).unwrap_or_default(),
            snap.connection.tx_bytes.map(|v| v.to_string()).unwrap_or_default(),
            wifi_2g_enabled,
            wifi_5g_enabled,
            snap.wifi.clients_2g.map(|v| v.to_string()).unwrap_or_default(),
            snap.wifi.clients_5g.map(|v| v.to_string()).unwrap_or_default(),
        )?;
        self.writer.flush()
    }
}

fn opt_f64(v: Option<f64>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

fn opt_str(v: &Option<String>) -> String {
    v.as_deref().unwrap_or("").to_string()
}
