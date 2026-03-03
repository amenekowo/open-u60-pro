use crate::device::DeviceShell;
use crate::error::{Result, ZteError};

const SERIAL_DEVICES: &[&str] = &[
    "/dev/at_mdm0",
    "/dev/at_mdm1",
    "/dev/at_usb0",
    "/dev/smd7",
    "/dev/smd11",
    "/dev/ttyUSB0",
    "/dev/ttyUSB1",
    "/dev/ttyUSB2",
];

/// AT command interface via serial device (background-cat method).
pub struct AtInterface<'a> {
    device: &'a DeviceShell,
    pub serial_device: String,
}

impl<'a> AtInterface<'a> {
    pub fn new(device: &'a DeviceShell, serial_device: Option<&str>) -> Result<Self> {
        let dev = match serial_device {
            Some(d) => d.to_string(),
            None => Self::detect_serial(device)?,
        };
        Ok(Self {
            device,
            serial_device: dev,
        })
    }

    fn detect_serial(device: &DeviceShell) -> Result<String> {
        for dev in SERIAL_DEVICES {
            if let Ok(result) = device.shell(&format!("[ -e {dev} ] && echo exists"), 5) {
                if !result.contains("exists") {
                    continue;
                }
                // Test with background-cat method
                if let Ok(test) = send_raw(device, "AT", dev, 3) {
                    if test.contains("OK") {
                        return Ok(dev.to_string());
                    }
                }
            }
        }
        // Fallback: return first device that exists
        for dev in SERIAL_DEVICES {
            if let Ok(result) = device.shell(&format!("[ -e {dev} ] && echo exists"), 5) {
                if result.contains("exists") {
                    return Ok(dev.to_string());
                }
            }
        }
        Err(ZteError::AtNoDevice)
    }

    /// Send an AT command and return the response.
    pub fn send(&self, command: &str) -> Result<String> {
        let cmd = if command.starts_with("AT") {
            command.to_string()
        } else {
            format!("AT{command}")
        };
        let result = send_raw(self.device, &cmd, &self.serial_device, 5)?;
        Ok(result.trim().to_string())
    }

    /// Send AT command and raise if OK not in response.
    pub fn send_expect_ok(&self, command: &str) -> Result<String> {
        let resp = self.send(command)?;
        if !resp.contains("OK") && resp.contains("ERROR") {
            return Err(ZteError::At(format!("AT command returned error: {resp}")));
        }
        Ok(resp)
    }

    /// Get signal strength info via AT+CSQ.
    pub fn get_signal_info(&self) -> Result<String> {
        let resp = self.send("AT+CSQ")?;
        Ok(parse_response(&resp))
    }

    /// Get serving cell info.
    pub fn get_serving_cell(&self) -> Result<String> {
        let resp = self.send("AT+QENG=\"servingcell\"")?;
        Ok(parse_response(&resp))
    }

    /// Get network info via AT+QNWINFO.
    pub fn get_network_info(&self) -> Result<String> {
        let resp = self.send("AT+QNWINFO")?;
        Ok(parse_response(&resp))
    }

    /// Get operator info via AT+COPS?.
    pub fn get_operator(&self) -> Result<String> {
        let resp = self.send("AT+COPS?")?;
        Ok(parse_response(&resp))
    }

    /// Get comprehensive cell information.
    pub fn get_cell_info(&self) -> std::collections::HashMap<String, Option<String>> {
        let commands = [
            ("csq", "AT+CSQ"),
            ("cops", "AT+COPS?"),
            ("serving_cell", "AT+QENG=\"servingcell\""),
            ("network_info", "AT+QNWINFO"),
        ];
        let mut info = std::collections::HashMap::new();
        for (key, cmd) in commands {
            match self.send(cmd) {
                Ok(resp) => info.insert(key.to_string(), Some(parse_response(&resp))),
                Err(_) => info.insert(key.to_string(), None),
            };
        }
        info
    }
}

fn send_raw(device: &DeviceShell, command: &str, serial_dev: &str, timeout: u64) -> Result<String> {
    let wait_s = timeout.min(2);
    let cmd = format!(
        "sh -c 'cat {serial_dev} &\nPID=$!\nsleep 0.3\necho -e \"{command}\\r\" > {serial_dev}\nsleep {wait_s}\nkill $PID 2>/dev/null\n'"
    );
    device
        .shell(&cmd, timeout + 5)
        .map_err(|e| ZteError::At(format!("AT command failed ({command}): {e}")))
}

/// Parse AT response, stripping echo and OK/ERROR lines.
pub fn parse_response(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut result_lines = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("AT") || line == "OK" || line == "ERROR" {
            continue;
        }
        result_lines.push(line);
    }
    result_lines.join("\n")
}
