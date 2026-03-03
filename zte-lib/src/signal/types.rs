use serde::{Deserialize, Serialize};

/// Collection method for signal data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionMethod {
    Ubus,
    At,
    Wifi,
    Auto,
}

impl std::fmt::Display for CollectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ubus => write!(f, "ubus"),
            Self::At => write!(f, "at"),
            Self::Wifi => write!(f, "wifi"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NrSignal {
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub sinr: Option<f64>,
    pub rssi: Option<f64>,
    pub pci: Option<String>,
    pub earfcn: Option<String>,
    pub band: Option<String>,
    pub bandwidth: Option<String>,
    pub mcc: Option<String>,
    pub mnc: Option<String>,
    pub cell_id: Option<String>,
    pub ca_status: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LteCarrier {
    pub carrier_type: Option<String>, // "PCC" or "SCC0", "SCC1", ...
    pub pci: Option<String>,
    pub band: Option<String>,
    pub earfcn: Option<String>,
    pub bandwidth: Option<String>,
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub sinr: Option<f64>,
    pub rssi: Option<f64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LteSignal {
    pub rsrp: Option<f64>,
    pub rsrq: Option<f64>,
    pub sinr: Option<f64>,
    pub rssi: Option<f64>,
    pub pci: Option<String>,
    pub earfcn: Option<String>,
    pub band: Option<String>,
    pub bandwidth: Option<String>,
    pub mcc: Option<String>,
    pub mnc: Option<String>,
    pub cell_id: Option<String>,
    pub ca_state: Option<String>,
    pub scc_carriers: Vec<LteCarrier>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WcdmaSignal {
    pub rscp: Option<f64>,
    pub ecio: Option<f64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CopsInfo {
    pub operator: Option<String>,
    pub act: Option<String>,
    pub signalbar: Option<String>,
    pub roaming: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConnectedDevice {
    pub mac: String,
    pub hostname: String,
    pub ipv4: String,
    pub ipv6: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub network_type: Option<String>,
    pub active_band: Option<String>,
    pub provider: Option<String>,
    pub roaming: Option<String>,
    pub ip_addr: Option<String>,
    pub ipv6_addr: Option<String>,
    pub uptime: Option<String>,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
    pub dl_speed_mbps: Option<f64>,
    pub ul_speed_mbps: Option<f64>,
    pub speed_source: Option<String>,
    pub raw_rx_rate: Option<f64>,
    pub raw_tx_rate: Option<f64>,
    pub gateway_ip: Option<String>,
    pub lan_domain: Option<String>,
    pub device_count: Option<u32>,
    pub wan_apn: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub battery_pct: Option<u32>,
    pub battery_temp: Option<f64>,
    pub battery_current_ua: Option<i64>,
    pub cpu_temp: Option<f64>,
    pub cpu_usage: Option<f64>,
    pub sim_status: Option<String>,
    pub imei: Option<String>,
    pub iccid: Option<String>,
    pub imsi: Option<String>,
    pub msisdn: Option<String>,
    pub spn: Option<String>,
    pub mcc: Option<String>,
    pub mnc: Option<String>,
    pub charging: Option<String>,
    pub charge_status: Option<u32>,
    pub battery_present: Option<bool>,
    pub battery_time_to_full: Option<i64>,
    pub battery_time_to_empty: Option<i64>,
    pub monitoring_uptime_secs: Option<u64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WifiInfo {
    pub wifi_onoff: Option<String>,
    pub ssid_2g: Option<String>,
    pub ssid_5g: Option<String>,
    pub channel_2g: Option<String>,
    pub channel_5g: Option<String>,
    pub encryption_2g: Option<String>,
    pub encryption_5g: Option<String>,
    pub hidden_2g: Option<String>,
    pub hidden_5g: Option<String>,
    pub txpower_2g: Option<String>,
    pub txpower_5g: Option<String>,
    pub radio_2g_disabled: Option<String>,
    pub radio_5g_disabled: Option<String>,
    pub clients_2g: Option<u32>,
    pub clients_5g: Option<u32>,
    pub clients_total: Option<u32>,
    pub wifi6: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SignalSnapshot {
    pub nr: NrSignal,
    pub lte: LteSignal,
    pub wcdma: WcdmaSignal,
    pub cops: CopsInfo,
    pub connection: ConnectionInfo,
    pub device: DeviceInfo,
    pub wifi: WifiInfo,
    pub connected_devices: Vec<ConnectedDevice>,
}
