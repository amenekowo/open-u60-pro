use ratatui::style::Color;

/// Map RSRP (dBm) to a color.
pub fn rsrp_color(rsrp: f64) -> Color {
    if rsrp >= -80.0 {
        Color::Green
    } else if rsrp >= -90.0 {
        Color::LightGreen
    } else if rsrp >= -100.0 {
        Color::Yellow
    } else if rsrp >= -110.0 {
        Color::LightRed
    } else {
        Color::Red
    }
}

/// Map SINR/SNR (dB) to a color.
pub fn sinr_color(sinr: f64) -> Color {
    if sinr >= 20.0 {
        Color::Green
    } else if sinr >= 13.0 {
        Color::LightGreen
    } else if sinr >= 0.0 {
        Color::Yellow
    } else if sinr >= -5.0 {
        Color::LightRed
    } else {
        Color::Red
    }
}

/// Map RSRQ (dB) to a color.
pub fn rsrq_color(rsrq: f64) -> Color {
    if rsrq >= -10.0 {
        Color::Green
    } else if rsrq >= -15.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Map battery percentage to a color.
pub fn battery_color(pct: u32) -> Color {
    if pct >= 60 {
        Color::Green
    } else if pct >= 30 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Map temperature (Celsius) to a color.
pub fn temp_color(temp: f64) -> Color {
    if temp < 40.0 {
        Color::Green
    } else if temp < 50.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Map CPU usage percentage to a color.
pub fn cpu_usage_color(pct: f64) -> Color {
    if pct < 30.0 {
        Color::Green
    } else if pct < 60.0 {
        Color::Yellow
    } else if pct < 85.0 {
        Color::LightRed
    } else {
        Color::Red
    }
}
