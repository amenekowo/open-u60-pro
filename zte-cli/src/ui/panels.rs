use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use zte_lib::signal::types::*;

use super::colors::*;

// --- Helper ---

const COL_W: usize = 20;

fn fmt_val(val: Option<f64>, suffix: &str) -> String {
    match val {
        Some(v) => format!("{v:.1}{suffix}"),
        None => "--".into(),
    }
}

fn s_or(opt: &Option<String>, default: &str) -> String {
    opt.as_deref().unwrap_or(default).to_string()
}

fn fmt_speed(bytes_per_sec: f64) -> String {
    let bits = bytes_per_sec * 8.0;
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const TB: f64 = 1024.0 * 1024.0 * 1024.0 * 1024.0;
    if bits / TB >= 0.5 {
        format!("{:.2}Tb/s", (bits / TB * 100.0).round() / 100.0)
    } else if bits / GB >= 0.5 {
        format!("{:.2}Gb/s", (bits / GB * 100.0).round() / 100.0)
    } else if bits / MB >= 0.5 {
        format!("{:.2}Mb/s", (bits / MB * 100.0).round() / 100.0)
    } else if bits / KB >= 0.5 {
        format!("{:.2}Kb/s", (bits / KB * 100.0).round() / 100.0)
    } else {
        format!("{:.2}b/s", (bits * 100.0).round() / 100.0)
    }
}

fn format_eta(mins: i64) -> String {
    if mins >= 60 {
        format!("{}h {}min", mins / 60, mins % 60)
    } else {
        format!("{mins}min")
    }
}

/// Carrier column data for side-by-side display.
struct CarrierCol {
    label: String,
    pci: String,
    band: String,
    earfcn: String,
    bw: String,
    rsrp: (String, Color),
    rsrq: (String, Color),
    sinr: (String, Color),
    rssi: String,
}

/// Separator span between columns.
fn col_sep() -> Span<'static> {
    Span::styled(" │ ", Style::default().fg(Color::DarkGray))
}

/// Build carrier column lines from a slice of CarrierCol into `lines`.
fn push_carrier_columns(lines: &mut Vec<Line<'static>>, carriers: &[CarrierCol]) {
    if carriers.is_empty() {
        return;
    }

    // Field rows: (label_prefix, getter returning (text, Option<Color>))
    let field_rows: Vec<(&str, Box<dyn Fn(&CarrierCol) -> (String, Option<Color>)>)> = vec![
        ("", Box::new(|cc: &CarrierCol| (cc.label.clone(), None))),
        ("PCI: ", Box::new(|cc: &CarrierCol| (cc.pci.clone(), None))),
        ("Band: ", Box::new(|cc: &CarrierCol| (cc.band.clone(), None))),
        ("EARFCN: ", Box::new(|cc: &CarrierCol| (cc.earfcn.clone(), None))),
        ("BW: ", Box::new(|cc: &CarrierCol| (cc.bw.clone(), None))),
        ("RSRP: ", Box::new(|cc: &CarrierCol| (cc.rsrp.0.clone(), Some(cc.rsrp.1)))),
        ("RSRQ: ", Box::new(|cc: &CarrierCol| (cc.rsrq.0.clone(), Some(cc.rsrq.1)))),
        ("SINR: ", Box::new(|cc: &CarrierCol| (cc.sinr.0.clone(), Some(cc.sinr.1)))),
        ("RSSI: ", Box::new(|cc: &CarrierCol| (cc.rssi.clone(), None))),
    ];

    for (prefix, getter) in &field_rows {
        let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
        for (j, cc) in carriers.iter().enumerate() {
            if j > 0 {
                spans.push(col_sep());
            }
            let (val, color) = getter(cc);
            let text = if prefix.is_empty() {
                format!("{:<width$}", val, width = COL_W)
            } else {
                format!("{:<width$}", format!("{prefix}{val}"), width = COL_W)
            };
            let style = match color {
                Some(c) => Style::default().fg(c),
                None => Style::default(),
            };
            spans.push(Span::styled(text, style));
        }
        lines.push(Line::from(spans));
    }
}

// --- Unified Cell Information Panel ---

pub fn render_cell_panel(
    f: &mut Frame,
    area: Rect,
    nr: &NrSignal,
    lte: &LteSignal,
    wcdma: &WcdmaSignal,
    cops: &CopsInfo,
    show_nr: bool,
    show_lte: bool,
    show_3g: bool,
) {
    let mut lines = Vec::new();

    // 3G only: simple RSCP + Ec/Io, no carrier columns
    if show_3g && !show_nr && !show_lte {
        lines.push(Line::from(format!("  RSCP:  {}", fmt_val(wcdma.rscp, " dBm"))));
        lines.push(Line::from(format!("  Ec/Io: {}", fmt_val(wcdma.ecio, " dB"))));

        let block = Block::default()
            .title(" Cell Information ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let para = Paragraph::new(lines).block(block);
        f.render_widget(para, area);
        return;
    }

    // Header lines for 4G/5G modes
    if show_nr || show_lte {
        let operator = s_or(&cops.operator, "--");
        let raw_act = s_or(&cops.act, "--").to_uppercase();
        let display_act = if show_nr && !show_lte
            && !raw_act.contains("SA") && !raw_act.contains("NR") && !raw_act.contains("5G") {
            "5G SA".to_string()
        } else if show_nr && show_lte
            && !raw_act.contains("SA") && !raw_act.contains("NR") && !raw_act.contains("5G")
            && !raw_act.contains("ENDC") && !raw_act.contains("EN-DC") {
            "5G NSA".to_string()
        } else {
            s_or(&cops.act, "--")
        };
        lines.push(Line::from(format!("  Operator: {operator} ({display_act})")));
    }

    // Cell ID line (combined if both present)
    if show_nr && show_lte {
        lines.push(Line::from(format!(
            "  5G Cell ID: {}  |  LTE Cell ID: {}",
            s_or(&nr.cell_id, "--"),
            s_or(&lte.cell_id, "--"),
        )));
    } else if show_nr {
        lines.push(Line::from(format!("  Cell ID: {}", s_or(&nr.cell_id, "--"))));
    } else if show_lte {
        lines.push(Line::from(format!("  Cell ID: {}", s_or(&lte.cell_id, "--"))));
    }

    // CA status line (combined if both present)
    let nr_ca_str = if nr.ca_status.as_deref().is_some_and(|s| !s.is_empty()) {
        "Active"
    } else {
        "Inactive"
    };
    let lte_scc_count = lte.scc_carriers.len();
    let lte_num_cc = 1 + lte_scc_count;
    let lte_ca_str = if let Some(ref ca_state) = lte.ca_state {
        if ca_state != "0" && lte_scc_count > 0 {
            format!("Active ({lte_num_cc} CC)")
        } else {
            "Inactive".to_string()
        }
    } else {
        "Inactive".to_string()
    };

    if show_nr && show_lte {
        lines.push(Line::from(format!(
            "  5G DL CA: {nr_ca_str}  |  4G DL CA: {lte_ca_str}"
        )));
    } else if show_nr {
        lines.push(Line::from(format!("  5G DL CA: {nr_ca_str}")));
    } else if show_lte {
        lines.push(Line::from(format!("  4G DL CA: {lte_ca_str}")));
    }

    lines.push(Line::from(""));

    // Build carrier columns with tech-prefixed labels
    let mut carriers = Vec::new();

    if show_nr {
        carriers.push(CarrierCol {
            label: "5G PCC".into(),
            pci: s_or(&nr.pci, "--"),
            band: s_or(&nr.band, "--"),
            earfcn: s_or(&nr.earfcn, "--"),
            bw: nr.bandwidth.as_deref().map(|b| format!("{b} MHz")).unwrap_or("--".into()),
            rsrp: (fmt_val(nr.rsrp, " dBm"), nr.rsrp.map(rsrp_color).unwrap_or(Color::DarkGray)),
            rsrq: (fmt_val(nr.rsrq, " dB"), nr.rsrq.map(rsrq_color).unwrap_or(Color::DarkGray)),
            sinr: (fmt_val(nr.sinr, " dB"), nr.sinr.map(sinr_color).unwrap_or(Color::DarkGray)),
            rssi: fmt_val(nr.rssi, " dBm"),
        });
    }

    if show_lte {
        carriers.push(CarrierCol {
            label: "4G PCC".into(),
            pci: s_or(&lte.pci, "--"),
            band: s_or(&lte.band, "--"),
            earfcn: s_or(&lte.earfcn, "--"),
            bw: lte.bandwidth.as_deref().map(|b| format!("{b} MHz")).unwrap_or("--".into()),
            rsrp: (fmt_val(lte.rsrp, " dBm"), lte.rsrp.map(rsrp_color).unwrap_or(Color::DarkGray)),
            rsrq: (fmt_val(lte.rsrq, " dB"), lte.rsrq.map(rsrq_color).unwrap_or(Color::DarkGray)),
            sinr: (fmt_val(lte.sinr, " dB"), lte.sinr.map(sinr_color).unwrap_or(Color::DarkGray)),
            rssi: fmt_val(lte.rssi, " dBm"),
        });

        for (i, sc) in lte.scc_carriers.iter().enumerate() {
            carriers.push(CarrierCol {
                label: format!("4G SCC{i}"),
                pci: s_or(&sc.pci, "--"),
                band: s_or(&sc.band, "--"),
                earfcn: s_or(&sc.earfcn, "--"),
                bw: sc.bandwidth.as_deref().map(|b| format!("{b} MHz")).unwrap_or("--".into()),
                rsrp: (fmt_val(sc.rsrp, " dBm"), sc.rsrp.map(rsrp_color).unwrap_or(Color::DarkGray)),
                rsrq: (fmt_val(sc.rsrq, " dB"), sc.rsrq.map(rsrq_color).unwrap_or(Color::DarkGray)),
                sinr: (fmt_val(sc.sinr, " dB"), sc.sinr.map(sinr_color).unwrap_or(Color::DarkGray)),
                rssi: fmt_val(sc.rssi, " dBm"),
            });
        }
    }

    push_carrier_columns(&mut lines, &carriers);

    let block = Block::default()
        .title(" Cell Information ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

// --- Connection Panel ---

pub fn render_connection_panel(f: &mut Frame, area: Rect, conn: &ConnectionInfo, debug: bool) {
    let src_tag = conn
        .speed_source
        .as_deref()
        .map(|s| format!(" [{s}]"))
        .unwrap_or_default();
    let dl = conn.raw_rx_rate.map(fmt_speed).unwrap_or("--".into());
    let ul = conn.raw_tx_rate.map(fmt_speed).unwrap_or("--".into());
    let dl_total = conn
        .rx_bytes
        .map(|b| format_bytes(b))
        .unwrap_or("--".into());
    let ul_total = conn
        .tx_bytes
        .map(|b| format_bytes(b))
        .unwrap_or("--".into());

    let mut lines = vec![
        Line::from(format!("  DL Speed:   {dl}{src_tag}")),
        Line::from(format!("  UL Speed:   {ul}")),
        Line::from(format!("  DL Total:   {dl_total}")),
        Line::from(format!("  UL Total:   {ul_total}")),
        Line::from(format!(
            "  Devices:    {}",
            conn.device_count.map(|c| c.to_string()).unwrap_or("--".into())
        )),
        Line::from(format!(
            "  WAN IP:     {}",
            s_or(&conn.ip_addr, "--")
        )),
        Line::from(format!(
            "  WAN IPv6:   {}",
            s_or(&conn.ipv6_addr, "--")
        )),
        Line::from(format!(
            "  Gateway:    {}",
            s_or(&conn.gateway_ip, "--")
        )),
        Line::from(format!(
            "  LAN Domain: {}",
            s_or(&conn.lan_domain, "--")
        )),
    ];

    if debug {
        if let (Some(rx), Some(tx)) = (conn.raw_rx_rate, conn.raw_tx_rate) {
            lines.push(Line::from(format!(
                "  Raw RX:     {rx:.1}  TX: {tx:.1}"
            )));
        }
    }

    let block = Block::default()
        .title(" Connection ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

// --- Device Panel ---

pub fn render_device_panel(
    f: &mut Frame,
    area: Rect,
    dev: &DeviceInfo,
) {
    let mut lines = Vec::new();

    // Battery with charging annotation
    let bat_str = dev
        .battery_pct
        .map(|p| format!("{p}%"))
        .unwrap_or("--".into());
    let charging_note = match dev.charging.as_deref() {
        Some("wall") => " (wall mode)",
        Some("charging") => " (charging)",
        _ => "",
    };
    let bat_temp = dev
        .battery_temp
        .map(|t| format!("{t:.1}C"))
        .unwrap_or("--".into());
    lines.push(Line::from(vec![
        Span::raw("  Battery:    "),
        Span::styled(
            &bat_str,
            Style::default().fg(
                dev.battery_pct
                    .map(battery_color)
                    .unwrap_or(Color::DarkGray),
            ),
        ),
        Span::raw(format!("{charging_note}  ({bat_temp})")),
    ]));

    // Battery ETA (state-aware)
    match dev.charging.as_deref() {
        Some("wall") => {
            // Wall mode — no ETA needed
        }
        Some("charging") => {
            if dev.battery_pct == Some(100) {
                lines.push(Line::from("  Status:     Full"));
            } else if let Some(mins) = dev.battery_time_to_full {
                if mins > 0 {
                    let eta = format_eta(mins);
                    lines.push(Line::from(format!("  Charge ETA: {eta}")));
                }
            }
        }
        _ => {
            // Discharging
            if let Some(mins) = dev.battery_time_to_empty {
                if mins > 0 {
                    let eta = format_eta(mins);
                    lines.push(Line::from(format!("  Drain ETA:  {eta}")));
                }
            }
        }
    }

    // Battery current
    if let Some(ua) = dev.battery_current_ua {
        let ma = ua as f64 / 1000.0;
        let label = if ma < 0.0 { "Draw" } else { "Charge" };
        lines.push(Line::from(format!("  Current:    {:.0} mA ({label})", ma.abs())));
    }

    // CPU temp
    lines.push(Line::from(vec![
        Span::raw("  CPU Temp:   "),
        Span::styled(
            dev.cpu_temp
                .map(|t| format!("{t:.1}C"))
                .unwrap_or("--".into()),
            Style::default().fg(dev.cpu_temp.map(temp_color).unwrap_or(Color::DarkGray)),
        ),
    ]));

    // CPU usage or load average
    if let Some(pct) = dev.cpu_usage {
        lines.push(Line::from(vec![
            Span::raw("  CPU Usage:  "),
            Span::styled(
                format!("{pct:.1}%"),
                Style::default().fg(cpu_usage_color(pct)),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  CPU Usage:  "),
            Span::styled("--", Style::default().fg(Color::DarkGray)),
            Span::styled("  zte acl patch", Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Monitoring uptime
    if let Some(secs) = dev.monitoring_uptime_secs {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        lines.push(Line::from(format!("  Monitoring: {h}:{m:02}:{s:02}")));
    }

    let block = Block::default()
        .title(" Device ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

// --- SIM / Device Identity Panel ---

pub fn render_sim_panel(
    f: &mut Frame,
    area: Rect,
    dev: &DeviceInfo,
    cops: &CopsInfo,
    conn: &ConnectionInfo,
) {
    let msisdn = dev.msisdn.as_deref().unwrap_or("--");
    let msisdn_display = if !msisdn.is_empty() && msisdn != "--" && !msisdn.starts_with('+') {
        format!("+{msisdn}")
    } else {
        msisdn.to_string()
    };

    let mut lines = vec![
        Line::from(format!("  IMEI:    {}", s_or(&dev.imei, "--"))),
        Line::from(format!("  ICCID:   {}", s_or(&dev.iccid, "--"))),
        Line::from(format!("  IMSI:    {}", s_or(&dev.imsi, "--"))),
        Line::from(format!("  Phone #: {msisdn_display}")),
    ];

    // Row 5: SPN + MCC/MNC (two-column)
    let spn = s_or(&dev.spn, "--");
    let mcc_mnc = match (&dev.mcc, &dev.mnc) {
        (Some(mcc), Some(mnc)) => format!("{mcc}/{mnc}"),
        _ => "--".to_string(),
    };
    lines.push(Line::from(format!(
        "  SPN:     {:<13}MCC/MNC: {mcc_mnc}",
        spn
    )));

    // Row 6: SIM status + Roaming (colored)
    let (sim_text, sim_color) = match dev.sim_status.as_deref() {
        Some("1") | Some("ready") => ("Ready", Color::Green),
        Some("0") | Some("not_ready") | Some("") => ("Not Ready", Color::Red),
        Some(s) => (s, Color::Yellow),
        None => ("--", Color::DarkGray),
    };
    let (roam_text, roam_color) = match cops.roaming.as_deref() {
        Some("0") | Some("Home") | Some("home") => ("Home", Color::Green),
        Some("1") | Some("Roaming") | Some("roaming") => ("Roaming", Color::Yellow),
        Some(s) => (s, Color::DarkGray),
        None => ("--", Color::DarkGray),
    };
    lines.push(Line::from(vec![
        Span::raw("  SIM:     "),
        Span::styled(format!("{:<13}", sim_text), Style::default().fg(sim_color)),
        Span::raw("Roaming: "),
        Span::styled(roam_text, Style::default().fg(roam_color)),
    ]));

    // Row 7: APN
    lines.push(Line::from(format!(
        "  APN:     {}",
        s_or(&conn.wan_apn, "--")
    )));

    // Row 8: Signal bars
    let (bars_text, bars_color) = match cops.signalbar.as_deref().and_then(|s| s.parse::<u32>().ok()) {
        Some(n) => {
            let filled = "█".repeat(n as usize);
            let empty = "░".repeat(5_usize.saturating_sub(n as usize));
            let color = match n {
                5 => Color::Green,
                4 => Color::LightGreen,
                3 => Color::Yellow,
                2 => Color::LightRed,
                _ => Color::Red,
            };
            (format!("{filled}{empty} ({n}/5)"), color)
        }
        None => ("--".to_string(), Color::DarkGray),
    };
    lines.push(Line::from(vec![
        Span::raw("  Signal:  "),
        Span::styled(bars_text, Style::default().fg(bars_color)),
    ]));

    let block = Block::default()
        .title(" SIM / Device ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

// --- WiFi Radios Panel ---

pub fn render_wifi_panel(f: &mut Frame, area: Rect, wifi: &WifiInfo) {
    let wifi_off = matches!(
        wifi.wifi_onoff.as_deref(),
        Some("0") | Some("false") | Some("off")
    );

    let disabled_2g = wifi.radio_2g_disabled.as_deref() == Some("1");
    let enabled_2g = !wifi_off && !disabled_2g;
    let disabled_5g = wifi.radio_5g_disabled.as_deref() == Some("1");
    let enabled_5g = !wifi_off && !disabled_5g;

    let clients_2g = wifi.clients_2g.unwrap_or(0);
    let clients_5g = wifi.clients_5g.unwrap_or(0);
    let clients_total = wifi.clients_total.unwrap_or(0);
    let has_per_band = clients_2g + clients_5g > 0 || clients_total == 0;

    let ch_2g = wifi
        .channel_2g
        .as_deref()
        .map(|c| if c == "0" { "auto" } else { c })
        .unwrap_or("auto");
    let ch_5g = wifi
        .channel_5g
        .as_deref()
        .map(|c| if c == "0" { "auto" } else { c })
        .unwrap_or("auto");

    let cli_2g_str = if has_per_band {
        format!("{clients_2g} client{}", if clients_2g != 1 { "s" } else { "" })
    } else {
        format!(
            "{clients_total} client{} (total)",
            if clients_total != 1 { "s" } else { "" }
        )
    };
    let cli_5g_str = if has_per_band {
        format!("{clients_5g} client{}", if clients_5g != 1 { "s" } else { "" })
    } else {
        String::new()
    };

    let hidden_2g = matches!(wifi.hidden_2g.as_deref(), Some("1") | Some("true"));
    let hidden_5g = matches!(wifi.hidden_5g.as_deref(), Some("1") | Some("true"));
    let hidden_note_2g = if hidden_2g { " (hidden)" } else { "" };
    let hidden_note_5g = if hidden_5g { " (hidden)" } else { "" };

    let enc_2g = wifi.encryption_2g.as_deref().unwrap_or("--");
    let enc_5g = wifi.encryption_5g.as_deref().unwrap_or("--");
    let tx_2g = wifi.txpower_2g.as_deref().unwrap_or("--");
    let tx_5g = wifi.txpower_5g.as_deref().unwrap_or("--");

    let mut lines = vec![];

    // 2.4 GHz
    let status_2g = if enabled_2g { "Enabled" } else { "Disabled" };
    let status_2g_color = if enabled_2g { Color::Green } else { Color::Red };
    lines.push(Line::from(vec![
        Span::styled("  2.4 GHz:  ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(status_2g, Style::default().fg(status_2g_color)),
    ]));
    lines.push(Line::from(format!(
        "    SSID:     {}",
        s_or(&wifi.ssid_2g, "--")
    )));
    lines.push(Line::from(format!(
        "    Channel:  CH {ch_2g}  {cli_2g_str}{hidden_note_2g}"
    )));
    lines.push(Line::from(format!(
        "    Security: {enc_2g}   TX {tx_2g}%"
    )));

    lines.push(Line::from(""));

    // 5 GHz
    let status_5g = if enabled_5g { "Enabled" } else { "Disabled" };
    let status_5g_color = if enabled_5g { Color::Green } else { Color::Red };
    lines.push(Line::from(vec![
        Span::styled("  5 GHz:    ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(status_5g, Style::default().fg(status_5g_color)),
    ]));
    lines.push(Line::from(format!(
        "    SSID:     {}",
        s_or(&wifi.ssid_5g, "--")
    )));
    let cli_5g_part = if !cli_5g_str.is_empty() {
        format!("  {cli_5g_str}")
    } else {
        String::new()
    };
    lines.push(Line::from(format!(
        "    Channel:  CH {ch_5g}{cli_5g_part}{hidden_note_5g}"
    )));
    lines.push(Line::from(format!(
        "    Security: {enc_5g}   TX {tx_5g}%"
    )));

    // WiFi 6
    let wifi6_on = matches!(
        wifi.wifi6.as_deref(),
        Some("1") | Some("true") | Some("on")
    );
    let wifi6_text = if wifi6_on { "Enabled" } else { "Disabled" };
    let wifi6_color = if wifi6_on {
        Color::Green
    } else {
        Color::DarkGray
    };
    lines.push(Line::from(vec![
        Span::styled("  WiFi 6:   ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(wifi6_text, Style::default().fg(wifi6_color)),
    ]));

    let block = Block::default()
        .title(" WiFi Radios ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::LightCyan));
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

// --- Connected Devices Panel ---

pub fn render_devices_panel(f: &mut Frame, area: Rect, devices: &[ConnectedDevice], scroll_offset: u16) {
    let count = devices.len();

    if devices.is_empty() {
        let lines = vec![Line::from("  No devices connected")];
        let block = Block::default()
            .title(format!(" Connected Devices ({count}) "))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let para = Paragraph::new(lines)
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(para, area);
        return;
    }

    // Header
    let mut lines = vec![Line::from(vec![Span::styled(
        "  Hostname        MAC                IPv4             IPv6",
        Style::default().add_modifier(Modifier::BOLD),
    )])];

    for dev in devices {
        let hostname = if dev.hostname.is_empty() {
            "--"
        } else if dev.hostname.len() > 15 {
            &dev.hostname[..15]
        } else {
            &dev.hostname
        };
        let mac = if dev.mac.is_empty() { "--" } else { &dev.mac };
        let ipv4 = if dev.ipv4.is_empty() { "--" } else { &dev.ipv4 };

        // Prefer GUA (non-link-local) over link-local
        let gua: Vec<&str> = dev.ipv6.iter().filter(|a| !a.starts_with("fe80")).map(|s| s.as_str()).collect();
        let ll: Vec<&str> = dev.ipv6.iter().filter(|a| a.starts_with("fe80")).map(|s| s.as_str()).collect();
        let mut ordered: Vec<&str> = Vec::new();
        ordered.extend(gua);
        ordered.extend(ll);
        let ipv6_str = if let Some(first) = ordered.first() {
            if ordered.len() > 1 {
                format!("{first}+")
            } else {
                first.to_string()
            }
        } else {
            "--".to_string()
        };

        lines.push(Line::from(format!(
            "  {:<15} {:<18} {:<16} {}",
            hostname, mac, ipv4, ipv6_str
        )));
    }

    // inner_height = area height minus 2 for borders
    let inner_height = area.height.saturating_sub(2) as usize;
    // total_lines includes header row + device rows
    let total_lines = lines.len();
    let overflows = total_lines > inner_height;

    let title = if overflows {
        format!(" Connected Devices ({count}) [↕ {}/{count}] ", scroll_offset + 1)
    } else {
        format!(" Connected Devices ({count}) ")
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let para = Paragraph::new(lines)
        .block(block)
        .scroll((scroll_offset, 0));
    f.render_widget(para, area);
}

// --- Helpers ---

fn format_bytes(bytes: u64) -> String {
    let b = bytes as f64;
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const TB: f64 = 1024.0 * 1024.0 * 1024.0 * 1024.0;
    if b / TB >= 0.5 {
        format!("{:.2}TB", (b / TB * 100.0).round() / 100.0)
    } else if b / GB >= 0.5 {
        format!("{:.2}GB", (b / GB * 100.0).round() / 100.0)
    } else if b / MB >= 0.5 {
        format!("{:.2}MB", (b / MB * 100.0).round() / 100.0)
    } else if b / KB >= 0.5 {
        format!("{:.2}KB", (b / KB * 100.0).round() / 100.0)
    } else {
        format!("{bytes}B")
    }
}
