use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

use super::types::*;

/// Parse a string to f64, returning None if > 9000 or < -9000 (firmware sentinels).
pub fn safe_float(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || s == "-" {
        return None;
    }
    match s.parse::<f64>() {
        Ok(f) if f > 9000.0 || f < -9000.0 => None,
        Ok(f) => Some(f),
        Err(_) => None,
    }
}

/// Decode UCS-2 hex-encoded SPN string (e.g. "004400490054004F" → "DITO").
pub fn decode_spn(hex: &str) -> Option<String> {
    let hex = hex.trim();
    if hex.is_empty() || hex.len() % 4 != 0 {
        return None;
    }
    let u16s: Result<Vec<u16>, _> = (0..hex.len())
        .step_by(4)
        .map(|i| u16::from_str_radix(&hex[i..i + 4], 16))
        .collect();
    let u16s = u16s.ok()?;
    // Filter out null chars
    let u16s: Vec<u16> = u16s.into_iter().filter(|&c| c != 0).collect();
    String::from_utf16(&u16s).ok().filter(|s| !s.is_empty())
}

/// Parse AT+QENG="servingcell" response into NR and LTE data.
pub fn parse_serving_cell(raw: &str) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut nr = HashMap::new();
    let mut lte = HashMap::new();

    for line in raw.lines() {
        let line = line.trim();

        // NR5G-NSA line
        if let Some(caps) = Regex::new(
            r#"\+QENG:\s*"servingcell"\s*,\s*"[^"]*"\s*,\s*"NR5G-NSA"\s*,(.*)"#,
        )
        .ok()
        .and_then(|re| re.captures(line))
        {
            let parts: Vec<&str> = caps[1].split(',').map(|s| s.trim()).collect();
            let fields = [
                "mcc", "mnc", "cell_id", "pcid", "arfcn", "band", "dl_bw",
                "rsrp", "rsrq", "sinr",
            ];
            for (i, &field) in fields.iter().enumerate() {
                if let Some(&val) = parts.get(i) {
                    if !val.is_empty() && val != "-" {
                        // Apply safe_float filtering for signal values
                        if matches!(field, "rsrp" | "rsrq" | "sinr") {
                            if safe_float(val).is_some() {
                                nr.insert(field.to_string(), val.to_string());
                            }
                        } else {
                            nr.insert(field.to_string(), val.to_string());
                        }
                    }
                }
            }
        }

        // NR5G-SA line
        if let Some(caps) = Regex::new(
            r#"\+QENG:\s*"servingcell"\s*,\s*"[^"]*"\s*,\s*"NR5G-SA"\s*,(.*)"#,
        )
        .ok()
        .and_then(|re| re.captures(line))
        {
            let parts: Vec<&str> = caps[1].split(',').map(|s| s.trim()).collect();
            let fields = [
                "state", "mode", "mcc", "mnc", "cell_id", "pcid", "arfcn",
                "band", "dl_bw", "rsrp", "rsrq", "sinr",
            ];
            for (i, &field) in fields.iter().enumerate() {
                if let Some(&val) = parts.get(i) {
                    if !val.is_empty() && val != "-" {
                        if matches!(field, "rsrp" | "rsrq" | "sinr") {
                            if safe_float(val).is_some() {
                                nr.insert(field.to_string(), val.to_string());
                            }
                        } else {
                            nr.insert(field.to_string(), val.to_string());
                        }
                    }
                }
            }
        }

        // LTE line
        if let Some(caps) = Regex::new(
            r#"\+QENG:\s*"servingcell"\s*,\s*"[^"]*"\s*,\s*"LTE"\s*,(.*)"#,
        )
        .ok()
        .and_then(|re| re.captures(line))
        {
            let parts: Vec<&str> = caps[1].split(',').map(|s| s.trim()).collect();
            let fields = [
                "is_tdd", "mcc", "mnc", "cell_id", "pcid", "earfcn", "freq_band",
                "ul_bw", "dl_bw", "tac", "rsrp", "rsrq", "rssi", "sinr",
                "srxlev",
            ];
            for (i, &field) in fields.iter().enumerate() {
                if let Some(&val) = parts.get(i) {
                    if !val.is_empty() && val != "-" {
                        if matches!(field, "rsrp" | "rsrq" | "rssi" | "sinr") {
                            if safe_float(val).is_some() {
                                lte.insert(field.to_string(), val.to_string());
                            }
                        } else {
                            lte.insert(field.to_string(), val.to_string());
                        }
                    }
                }
            }
        }
    }

    (nr, lte)
}

/// Parse AT+CSQ response.
pub fn parse_csq(raw: &str) -> Option<(u32, u32)> {
    let re = Regex::new(r"\+CSQ:\s*(\d+)\s*,\s*(\d+)").ok()?;
    let caps = re.captures(raw)?;
    let rssi: u32 = caps[1].parse().ok()?;
    let ber: u32 = caps[2].parse().ok()?;
    Some((rssi, ber))
}

/// Parse AT+COPS? response into CopsInfo.
pub fn parse_cops(raw: &str) -> Option<CopsInfo> {
    let re = Regex::new(r#"\+COPS:\s*(\d+)\s*,\s*(\d+)\s*,\s*"([^"]*)"\s*,\s*(\d+)"#).ok()?;
    let caps = re.captures(raw)?;
    let operator = caps[3].to_string();
    let act_num = &caps[4];
    let act = match act_num {
        "0" => "GSM",
        "2" => "UTRAN",
        "3" => "GSM/EGPRS",
        "4" => "UTRAN/HSDPA",
        "5" => "UTRAN/HSUPA",
        "6" => "UTRAN/HSDPA+HSUPA",
        "7" => "E-UTRAN (LTE)",
        "10" => "E-UTRAN (5G)",
        "11" => "NR",
        "12" => "NG-RAN",
        "13" => "E-UTRAN-NR",
        other => other,
    }
    .to_string();

    Some(CopsInfo {
        operator: Some(operator),
        act: Some(act),
        signalbar: None,
        roaming: None,
    })
}

/// Parse AT+QNWINFO response -> (network_type, operator_code, band, channel).
pub fn parse_qnwinfo(raw: &str) -> Option<(String, String, String, String)> {
    let re = Regex::new(
        r#"\+QNWINFO:\s*"([^"]*)"\s*,\s*"([^"]*)"\s*,\s*"([^"]*)"\s*,\s*(\d+)"#,
    )
    .ok()?;
    let caps = re.captures(raw)?;
    Some((
        caps[1].to_string(),
        caps[2].to_string(),
        caps[3].to_string(),
        caps[4].to_string(),
    ))
}

/// Parse AT+QTEMP response -> HashMap of sensor name to temperature.
pub fn parse_qtemp(raw: &str) -> HashMap<String, i64> {
    let mut temps = HashMap::new();
    for line in raw.lines() {
        let line = line.trim();
        // Named format: +QTEMP: "name","val"
        if let Some(caps) = Regex::new(r#"\+QTEMP:\s*"?(\w+)"?\s*,\s*"?(\d+)"?"#)
            .ok()
            .and_then(|re| re.captures(line))
        {
            if let Ok(val) = caps[2].parse::<i64>() {
                temps.insert(caps[1].to_string(), val);
            }
        } else if let Some(caps) = Regex::new(r"\+QTEMP:\s*([\d,\s]+)")
            .ok()
            .and_then(|re| re.captures(line))
        {
            // Unnamed format: +QTEMP: val1,val2,...
            for (i, v) in caps[1].split(',').map(|s| s.trim()).enumerate() {
                if let Ok(val) = v.parse::<i64>() {
                    temps.insert(format!("sensor_{i}"), val);
                }
            }
        }
    }
    temps
}

/// Parse ubus nwinfo_get_netinfo response into typed signal structs.
pub fn parse_netinfo(nw: &Value) -> (NrSignal, LteSignal, WcdmaSignal, CopsInfo) {
    let mut nr = NrSignal::default();
    let mut lte = LteSignal::default();
    let mut wcdma = WcdmaSignal::default();

    // NR5G fields
    nr.rsrp = get_f64_safe(nw, "nr5g_rsrp");
    nr.rsrq = get_f64_safe(nw, "nr5g_rsrq");
    nr.sinr = get_f64_safe(nw, "nr5g_snr");
    nr.rssi = get_f64_safe(nw, "nr5g_rssi");
    nr.pci = get_string(nw, "nr5g_pci");
    nr.earfcn = get_string(nw, "nr5g_action_channel");
    nr.band = get_string(nw, "nr5g_action_band")
        .or_else(|| get_string(nw, "wan_active_band"));
    nr.bandwidth = get_string(nw, "nr5g_bandwidth");
    nr.cell_id = get_string(nw, "nr5g_cell_id");
    nr.ca_status = get_string(nw, "nrca");

    // LTE PCC from main fields
    let pcc_pci = get_string(nw, "lte_pci");
    let pcc_earfcn = get_string(nw, "wan_active_channel");
    let pcc_band = get_string(nw, "wan_active_band").unwrap_or_default();
    lte.rsrp = get_f64_safe(nw, "lte_rsrp");
    lte.rsrq = get_f64_safe(nw, "lte_rsrq");
    lte.sinr = get_f64_safe(nw, "lte_snr");
    lte.rssi = get_f64_safe(nw, "lte_rssi");
    lte.pci = pcc_pci.clone();
    lte.earfcn = pcc_earfcn.clone();
    lte.band = Some(pcc_band);
    lte.cell_id = get_string(nw, "cell_id");
    lte.ca_state = get_string(nw, "lteca_state");

    // Parse lteca: "PCI,Band,Index,EARFCN,BW;..."
    let lteca_str = get_string(nw, "lteca").unwrap_or_default();
    let mut carriers: Vec<LteCarrier> = Vec::new();
    for entry in lteca_str.trim_end_matches(';').split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let parts: Vec<&str> = entry.split(',').collect();
        if parts.len() >= 5 {
            carriers.push(LteCarrier {
                carrier_type: None,
                pci: Some(parts[0].to_string()),
                band: Some(parts[1].to_string()),
                earfcn: Some(parts[3].to_string()),
                bandwidth: Some(parts[4].to_string()),
                ..Default::default()
            });
        }
    }

    // Parse ltecasig: "RSRP,RSRQ,SINR,RSSI,X,Y;..."
    let ltecasig_str = get_string(nw, "ltecasig").unwrap_or_default();
    let mut scc_sigs: Vec<(Option<f64>, Option<f64>, Option<f64>, Option<f64>)> = Vec::new();
    for entry in ltecasig_str.trim_end_matches(';').split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let parts: Vec<&str> = entry.split(',').collect();
        if parts.len() >= 4 {
            scc_sigs.push((
                safe_float(parts[0]),
                safe_float(parts[1]),
                safe_float(parts[2]),
                safe_float(parts[3]),
            ));
        }
    }

    // Find PCC in lteca entries by matching pci + earfcn, rest = SCCs
    let mut pcc_bw = String::new();
    let mut pcc_found = false;
    let mut scc_carriers: Vec<LteCarrier> = Vec::new();
    for c in &carriers {
        let c_pci = c.pci.as_deref().unwrap_or("");
        let c_earfcn = c.earfcn.as_deref().unwrap_or("");
        let match_pci = pcc_pci.as_deref().unwrap_or("__none__");
        let match_earfcn = pcc_earfcn.as_deref().unwrap_or("__none__");
        if !pcc_found && c_pci == match_pci && c_earfcn == match_earfcn {
            pcc_bw = c.bandwidth.clone().unwrap_or_default();
            pcc_found = true;
        } else {
            scc_carriers.push(c.clone());
        }
    }

    if !pcc_bw.is_empty() {
        lte.bandwidth = Some(pcc_bw);
    }

    // Assign SCC signals
    for (i, sc) in scc_carriers.iter_mut().enumerate() {
        if i < scc_sigs.len() {
            sc.rsrp = scc_sigs[i].0;
            sc.rsrq = scc_sigs[i].1;
            sc.sinr = scc_sigs[i].2;
            sc.rssi = scc_sigs[i].3;
        }
        sc.carrier_type = Some(format!("SCC{i}"));
    }
    lte.scc_carriers = scc_carriers;

    // WCDMA
    wcdma.rscp = get_f64_safe(nw, "rscp").filter(|&v| v != 0.0);
    wcdma.ecio = get_f64_safe(nw, "ecio");

    // COPS from netinfo
    let cops = CopsInfo {
        operator: get_string(nw, "network_provider"),
        act: get_string(nw, "network_type"),
        signalbar: get_string(nw, "signalbar"),
        roaming: get_string(nw, "simcard_roam"),
    };

    (nr, lte, wcdma, cops)
}

// --- Helper functions for JSON extraction ---

pub fn get_string(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|val| match val {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    })
}

pub fn get_f64(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|val| {
        val.as_f64()
            .or_else(|| val.as_str().and_then(|s| s.parse().ok()))
    })
}

/// Get f64 with safe_float filtering (rejects firmware sentinel values).
pub fn get_f64_safe(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|val| {
        val.as_f64()
            .and_then(|f| if f > 9000.0 || f < -9000.0 { None } else { Some(f) })
            .or_else(|| val.as_str().and_then(|s| safe_float(s)))
    })
}

pub fn get_u32(v: &Value, key: &str) -> Option<u32> {
    v.get(key).and_then(|val| {
        val.as_u64()
            .map(|n| n as u32)
            .or_else(|| val.as_str().and_then(|s| s.parse().ok()))
    })
}

pub fn get_u64(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|val| {
        val.as_u64()
            .or_else(|| val.as_str().and_then(|s| s.parse().ok()))
    })
}

pub fn get_i64(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(|val| {
        val.as_i64()
            .or_else(|| val.as_str().and_then(|s| s.parse().ok()))
    })
}
