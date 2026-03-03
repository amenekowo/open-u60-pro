use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;

use zte_lib::device::DeviceShell;
use zte_lib::ubus::UbusClient;

use crate::cmd::ShellArgs;

/// Writable location for the plugin binary (rootfs is read-only).
const PLUGIN_STAGING: &str = "/data/local/tmp/zte-companion-plugin";
/// Writable overlay for rpcd plugin directory.
const RPCD_OVERLAY_DIR: &str = "/data/local/tmp/rpcd-plugins";
/// Original rpcd plugin directory (read-only rootfs).
const RPCD_PLUGIN_DIR: &str = "/usr/libexec/rpcd";
/// Boot persistence script.
const COMPANION_INIT_SCRIPT: &str = "/data/local/tmp/companion_plugin.sh";

/// The Lua rpcd plugin source code.
/// Speaks the rpcd exec plugin protocol (JSON on stdin/stdout).
const PLUGIN_LUA: &str = r#"#!/usr/bin/env lua

-- zte-companion: rpcd exec plugin for ZTE U60 Pro
-- Methods: battery_current, cpu_usage, bandwidth,
--          call_dial, call_hangup, call_answer, call_status, call_dtmf, call_mute

-- JSON library: try luci.jsonc first, fall back to cjson
local json
local ok, mod = pcall(require, "luci.jsonc")
if ok then
    json = { encode = mod.stringify, decode = mod.parse }
else
    ok, mod = pcall(require, "cjson")
    if ok then
        json = { encode = mod.encode, decode = mod.decode }
    else
        -- minimal fallback: only encode what we need
        json = {
            encode = function(t)
                local parts = {}
                for k, v in pairs(t) do
                    local vstr
                    if type(v) == "number" then
                        vstr = tostring(v)
                    elseif type(v) == "string" then
                        vstr = '"' .. v:gsub('"', '\\"') .. '"'
                    else
                        vstr = tostring(v)
                    end
                    parts[#parts + 1] = '"' .. k .. '":' .. vstr
                end
                return "{" .. table.concat(parts, ",") .. "}"
            end,
            decode = function(s)
                if not s or s == "" then return {} end
                local t = {}
                for k, v in s:gmatch('"([^"]+)"%s*:%s*"([^"]*)"') do t[k] = v end
                for k in s:gmatch('"([^"]+)"%s*:%s*true') do t[k] = true end
                for k in s:gmatch('"([^"]+)"%s*:%s*false') do t[k] = false end
                for k, v in s:gmatch('"([^"]+)"%s*:%s*(-?%d+%.?%d*)') do t[k] = tonumber(v) end
                return t
            end,
        }
    end
end

local function read_file(path)
    local f = io.open(path, "r")
    if not f then return nil end
    local content = f:read("*a")
    f:close()
    return content
end

-- AT command helpers for voice call control
local serial_port = nil

local function detect_serial()
    if serial_port then return serial_port end
    local ports = { "/dev/at_mdm0", "/dev/at_mdm1", "/dev/at_usb0", "/dev/smd7", "/dev/smd11" }
    for _, port in ipairs(ports) do
        local f = io.open(port, "r")
        if f then
            f:close()
            local h = io.popen(string.format(
                "cat %s & PID=$! ; sleep 0.3 ; echo -e 'AT\\r' > %s ; sleep 1 ; kill $PID 2>/dev/null",
                port, port), "r")
            if h then
                local resp = h:read("*a")
                h:close()
                if resp and resp:find("OK") then
                    serial_port = port
                    return port
                end
            end
        end
    end
    return nil
end

local function send_at(cmd, wait)
    local port = detect_serial()
    if not port then return nil, "no serial port found" end
    wait = wait or 2
    local h = io.popen(string.format(
        "cat %s & PID=$! ; sleep 0.3 ; echo -e '%s\\r' > %s ; sleep %d ; kill $PID 2>/dev/null",
        port, cmd, port, wait), "r")
    if not h then return nil, "failed to open port" end
    local resp = h:read("*a")
    h:close()
    return resp
end

local function parse_clcc(raw)
    local calls = {}
    if not raw then return calls end
    local dir_names = { [0] = "mo", [1] = "mt" }
    local stat_names = { [0] = "active", [1] = "held", [2] = "dialing", [3] = "alerting",
                         [4] = "incoming", [5] = "waiting", [6] = "releasing" }
    for line in raw:gmatch("[^\n]+") do
        local id, dir, stat, mode, mpty, number = line:match(
            "%+CLCC:%s*(%d+),(%d+),(%d+),(%d+),(%d+),?\"?([^\",]*)\"?")
        if id then
            calls[#calls + 1] = {
                id = tonumber(id),
                dir = dir_names[tonumber(dir)] or tostring(dir),
                stat = stat_names[tonumber(stat)] or tostring(stat),
                mode = tonumber(mode),
                number = number or ""
            }
        end
    end
    return calls
end

local function read_stdin_params()
    local input = io.read("*a")
    if not input or input == "" then return {} end
    return json.decode(input) or {}
end

local function handle_list()
    -- Output JSON directly: empty Lua tables serialize as [] but rpcd needs {}
    io.write('{"battery_current":{},"cpu_usage":{},"bandwidth":{},')
    io.write('"call_dial":{"number":"string"},')
    io.write('"call_hangup":{},')
    io.write('"call_answer":{},')
    io.write('"call_status":{},')
    io.write('"call_dtmf":{"digits":"string"},')
    io.write('"call_mute":{"enabled":"boolean"}}')
end

local function handle_call(method)
    if method == "battery_current" then
        -- Read from /sys/class/power_supply/*/current_now
        local dirs = { "/sys/class/power_supply/battery", "/sys/class/power_supply/BAT0" }
        for _, dir in ipairs(dirs) do
            local val = read_file(dir .. "/current_now")
            if val then
                local microamps = tonumber(val:match("%-?%d+"))
                if microamps then
                    io.write(json.encode({ current_now = microamps }))
                    return
                end
            end
        end
        io.write(json.encode({ error = "no power supply found" }))

    elseif method == "cpu_usage" then
        local raw = read_file("/proc/stat")
        if not raw then
            io.write(json.encode({ error = "cannot read /proc/stat" }))
            return
        end
        -- Parse aggregate "cpu " line
        local line = raw:match("(cpu %s[^\n]+)")
        if not line then
            io.write(json.encode({ error = "no cpu line in /proc/stat" }))
            return
        end
        local fields = {}
        for num in line:gmatch("%d+") do
            fields[#fields + 1] = tonumber(num)
        end
        if #fields < 4 then
            io.write(json.encode({ error = "insufficient cpu fields" }))
            return
        end
        local idle = fields[4]
        if #fields > 4 then
            idle = idle + fields[5] -- iowait
        end
        local total = 0
        for _, v in ipairs(fields) do total = total + v end

        -- Count per-CPU cores (cpu0, cpu1, ...)
        local cores = 0
        for _ in raw:gmatch("\ncpu%d+") do
            cores = cores + 1
        end
        if cores == 0 then cores = 1 end

        io.write(json.encode({ idle = idle, total = total, cores = cores }))

    elseif method == "bandwidth" then
        -- Read /proc/net/dev for per-interface byte counters
        local raw = read_file("/proc/net/dev")
        if not raw then
            io.write('{"error":"cannot read /proc/net/dev"}')
            return
        end
        -- Read /proc/uptime for monotonic timestamp
        local uptime_raw = read_file("/proc/uptime")
        local ts = 0
        if uptime_raw then
            ts = tonumber(uptime_raw:match("([%d%.]+)")) or 0
        end
        -- Parse interface lines: "  iface: rx_bytes rx_packets ... tx_bytes tx_packets ..."
        local ifaces = {}
        for line in raw:gmatch("[^\n]+") do
            local name, rest = line:match("^%s*([^:]+):%s*(.*)")
            if name and rest then
                local fields = {}
                for num in rest:gmatch("%d+") do
                    fields[#fields + 1] = num
                end
                -- fields[1]=rx_bytes, fields[9]=tx_bytes (standard /proc/net/dev layout)
                if #fields >= 9 then
                    ifaces[#ifaces + 1] = string.format('"%s":{"rx":%s,"tx":%s}',
                        name:match("^%s*(.-)%s*$"), fields[1], fields[9])
                end
            end
        end
        io.write(string.format('{"ts":%.3f,"if":{%s}}', ts, table.concat(ifaces, ",")))

    elseif method == "call_dial" then
        local params = read_stdin_params()
        local number = params.number
        if not number or number == "" then
            io.write(json.encode({ error = "missing number" }))
            return
        end
        number = number:gsub("[^%d%+%*#]", "")
        local resp, err = send_at("ATD" .. number .. ";", 5)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
        elseif resp:find("ERROR") then
            io.write(json.encode({ error = "dial failed" }))
        else
            io.write(json.encode({ status = "ok" }))
        end

    elseif method == "call_hangup" then
        local resp, err = send_at("AT+CHUP", 3)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
        else
            io.write(json.encode({ status = "ok" }))
        end

    elseif method == "call_answer" then
        local resp, err = send_at("ATA", 3)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
        else
            io.write(json.encode({ status = "ok" }))
        end

    elseif method == "call_status" then
        local resp, err = send_at("AT+CLCC", 2)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
            return
        end
        local calls = parse_clcc(resp)
        local parts = {}
        for _, c in ipairs(calls) do
            parts[#parts + 1] = string.format(
                '{"id":%d,"dir":"%s","stat":"%s","mode":%d,"number":"%s"}',
                c.id, c.dir, c.stat, c.mode, c.number)
        end
        io.write('{"calls":[' .. table.concat(parts, ",") .. ']}')

    elseif method == "call_dtmf" then
        local params = read_stdin_params()
        local digits = params.digits
        if not digits or digits == "" then
            io.write(json.encode({ error = "missing digits" }))
            return
        end
        local cmds = {}
        for i = 1, #digits do
            local d = digits:sub(i, i)
            if d:match("[%d%*#ABCD]") then
                cmds[#cmds + 1] = "+VTS=" .. d
            end
        end
        if #cmds == 0 then
            io.write(json.encode({ error = "no valid digits" }))
            return
        end
        local resp, err = send_at("AT" .. table.concat(cmds, ";"), 2)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
        else
            io.write(json.encode({ status = "ok" }))
        end

    elseif method == "call_mute" then
        local params = read_stdin_params()
        local enabled = params.enabled
        local val = enabled and "1" or "0"
        local resp, err = send_at("AT+CMUT=" .. val, 2)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
        else
            io.write(json.encode({ muted = enabled and true or false }))
        end

    else
        io.write(json.encode({ error = "unknown method" }))
    end
end

-- rpcd exec plugin protocol: argv[1] = "list" or "call", argv[2] = method
local cmd = arg[1] or "list"

if cmd == "list" then
    handle_list()
elseif cmd == "call" then
    handle_call(arg[2] or "")
else
    handle_list()
end
"#;

#[derive(Subcommand)]
pub enum Cmd {
    /// Install zte-companion rpcd plugin on device
    Install {
        #[command(flatten)]
        shell: ShellArgs,
    },
    /// Check zte-companion plugin status
    Status {
        #[command(flatten)]
        shell: ShellArgs,
    },
    /// Remove zte-companion plugin from device
    Remove {
        #[command(flatten)]
        shell: ShellArgs,
    },
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Install { shell } => run_install(&shell),
        Cmd::Status { shell } => run_status(&shell),
        Cmd::Remove { shell } => run_remove(&shell),
    }
}

fn run_install(shell: &ShellArgs) -> Result<()> {
    let password = shell.password.clone();
    let gateway = shell.gateway.clone();
    let dev = shell.connect()?;
    println!("\n  {}\n", "Companion — Installing rpcd plugin".bold());

    // 1. Write Lua plugin to staging area
    dev.write_content(PLUGIN_LUA.as_bytes(), PLUGIN_STAGING)?;
    println!("  {} Plugin written to {}", "OK".green(), PLUGIN_STAGING.cyan());

    // 2. Create overlay directory and copy existing plugins
    dev.shell(
        &format!("mkdir -p {RPCD_OVERLAY_DIR} && cp -a {RPCD_PLUGIN_DIR}/* {RPCD_OVERLAY_DIR}/ 2>/dev/null; true"),
        10,
    )?;

    // 3. Copy our plugin into the overlay
    dev.shell(
        &format!("cp {PLUGIN_STAGING} {RPCD_OVERLAY_DIR}/zte-companion && chmod +x {RPCD_OVERLAY_DIR}/zte-companion"),
        5,
    )?;
    println!("  {} Plugin copied to {}/zte-companion", "OK".green(), RPCD_OVERLAY_DIR.cyan());

    // 4. Unmount any existing bind-mount, then bind-mount overlay
    dev.shell(
        &format!("umount {RPCD_PLUGIN_DIR} 2>/dev/null; true"),
        5,
    )?;
    dev.shell(
        &format!("mount --bind {RPCD_OVERLAY_DIR} {RPCD_PLUGIN_DIR}"),
        5,
    )?;
    println!(
        "  {} Bind-mounted {} → {}",
        "OK".green(),
        RPCD_OVERLAY_DIR.cyan(),
        RPCD_PLUGIN_DIR.cyan()
    );

    // 5. Restart rpcd to pick up new plugin
    println!("  Restarting rpcd...");
    let restart_result = dev.shell("/etc/init.d/rpcd restart", 10);
    match restart_result {
        Ok(_) => println!("  {} rpcd restarted.", "OK".green()),
        Err(e) => {
            println!("  {} restart failed ({}), trying HUP...", "!".yellow(), e);
            let _ = dev.shell("kill -HUP $(pidof rpcd)", 5);
        }
    }

    // Brief pause for rpcd to come up
    std::thread::sleep(std::time::Duration::from_secs(1));

    // 6. Install boot persistence
    install_boot_persistence(&dev);

    // 7. Optionally verify via HTTP
    if let Some(pw) = &password {
        println!("\n  Verifying via HTTP ubus call...");
        let mut client = UbusClient::new(gateway.as_deref(), 10);
        match client.login(pw) {
            Ok(_) => {
                match client.call("zte-companion", "cpu_usage", None) {
                    Ok(resp) => {
                        if resp.get("idle").is_some() && resp.get("total").is_some() {
                            println!(
                                "  {} zte-companion.cpu_usage is working!",
                                "OK".green().bold()
                            );
                        } else {
                            println!(
                                "  {} Got response but missing expected fields: {}",
                                "!".yellow(),
                                resp
                            );
                        }
                    }
                    Err(e) => println!(
                        "  {} cpu_usage verification failed: {e}",
                        "FAIL".red()
                    ),
                }
                match client.call("zte-companion", "battery_current", None) {
                    Ok(resp) => {
                        if resp.get("current_now").is_some() {
                            println!(
                                "  {} zte-companion.battery_current is working!",
                                "OK".green().bold()
                            );
                        } else {
                            println!(
                                "  {} Got response but missing current_now: {}",
                                "!".yellow(),
                                resp
                            );
                        }
                    }
                    Err(e) => println!(
                        "  {} battery_current verification failed: {e}",
                        "FAIL".red()
                    ),
                }
                match client.call("zte-companion", "bandwidth", None) {
                    Ok(resp) => {
                        if resp.get("if").is_some() && resp.get("ts").is_some() {
                            println!(
                                "  {} zte-companion.bandwidth is working!",
                                "OK".green().bold()
                            );
                        } else {
                            println!(
                                "  {} Got response but missing if/ts: {}",
                                "!".yellow(),
                                resp
                            );
                        }
                    }
                    Err(e) => println!(
                        "  {} bandwidth verification failed: {e}",
                        "FAIL".red()
                    ),
                }
            }
            Err(e) => println!(
                "  {} Could not login for verification: {e}",
                "!".yellow()
            ),
        }
    } else {
        println!(
            "\n  {} Use -p <password> to auto-verify, or test manually:",
            "Tip:".bold()
        );
        println!("    ubus call zte-companion cpu_usage");
        println!("    ubus call zte-companion battery_current");
    }

    println!();
    Ok(())
}

fn run_status(shell: &ShellArgs) -> Result<()> {
    let dev = shell.connect()?;
    println!("\n  {}\n", "Companion — Plugin Status".bold());

    // Check if plugin file exists
    let file_check = dev
        .shell(&format!("test -x {RPCD_OVERLAY_DIR}/zte-companion && echo yes || echo no"), 5)
        .unwrap_or_default();
    let file_exists = file_check.trim() == "yes";

    // Check if bind-mount is active
    let mount_check = dev
        .shell(&format!("mount | grep '{RPCD_PLUGIN_DIR}'"), 5)
        .unwrap_or_default();
    let is_mounted = !mount_check.trim().is_empty();

    // Check if rpcd sees the plugin (try calling it)
    let rpcd_check = dev
        .shell("ubus call zte-companion cpu_usage '{}' 2>&1", 5)
        .unwrap_or_default();
    let rpcd_works = rpcd_check.contains("idle") && rpcd_check.contains("total");

    // Check boot persistence
    let boot_check = dev
        .shell(&format!(
            "grep -q '{COMPANION_INIT_SCRIPT}' /etc/rc.local 2>/dev/null && echo yes || echo no"
        ), 5)
        .unwrap_or_default();
    let boot_persistent = boot_check.trim() == "yes";

    println!("  {} {}", "Plugin file:".bold(),
        if file_exists { "installed".green().to_string() } else { "missing".red().to_string() }
    );
    println!("  {} {}", "Bind-mount:".bold(),
        if is_mounted { "active".green().to_string() } else { "inactive".red().to_string() }
    );
    println!("  {} {}", "rpcd responds:".bold(),
        if rpcd_works { "yes".green().to_string() } else { "no".red().to_string() }
    );
    println!("  {} {}", "Boot persist:".bold(),
        if boot_persistent { "yes".green().to_string() } else { "no".yellow().to_string() }
    );

    if rpcd_works {
        println!("\n  {} Plugin is fully operational.", "OK".green().bold());
    } else if file_exists && is_mounted {
        println!("\n  {} Plugin installed but rpcd not responding. Try restarting rpcd.", "!".yellow());
    } else if file_exists {
        println!("\n  {} Plugin file exists but bind-mount not active. Run {} to re-install.", "!".yellow(), "zte companion install".cyan());
    } else {
        println!("\n  {} Plugin not installed. Run {} to install.", "!".yellow(), "zte companion install".cyan());
    }

    println!();
    Ok(())
}

fn run_remove(shell: &ShellArgs) -> Result<()> {
    let dev = shell.connect()?;
    println!("\n  {}\n", "Companion — Removing rpcd plugin".bold());

    // 1. Unmount bind-mount
    let mount_check = dev
        .shell(&format!("mount | grep '{RPCD_PLUGIN_DIR}'"), 5)
        .unwrap_or_default();
    if !mount_check.trim().is_empty() {
        dev.shell(&format!("umount {RPCD_PLUGIN_DIR}"), 5)?;
        println!("  {} Unmounted bind-mount.", "OK".green());
    } else {
        println!("  No bind-mount active.");
    }

    // 2. Remove overlay directory and staging file
    dev.shell(&format!("rm -rf {RPCD_OVERLAY_DIR} {PLUGIN_STAGING}"), 5)?;
    println!("  {} Cleaned up plugin files.", "OK".green());

    // 3. Remove boot persistence
    remove_boot_persistence(&dev);

    // 4. Restart rpcd
    println!("  Restarting rpcd...");
    let restart_result = dev.shell("/etc/init.d/rpcd restart", 10);
    match restart_result {
        Ok(_) => println!("  {} rpcd restarted.", "OK".green()),
        Err(e) => {
            println!("  {} restart failed ({}), trying HUP...", "!".yellow(), e);
            let _ = dev.shell("kill -HUP $(pidof rpcd)", 5);
        }
    }

    println!();
    Ok(())
}

/// Write a boot script and hook it into rc.local so the plugin bind-mount survives reboot.
fn install_boot_persistence(dev: &DeviceShell) {
    let script = format!(
        "#!/bin/sh\n\
         # Re-apply zte-companion rpcd plugin bind-mount on boot\n\
         if [ -d {RPCD_OVERLAY_DIR} ]; then\n\
           mount | grep -q '{RPCD_PLUGIN_DIR}' || {{\n\
             mount --bind {RPCD_OVERLAY_DIR} {RPCD_PLUGIN_DIR}\n\
             /etc/init.d/rpcd restart\n\
           }}\n\
         fi\n"
    );

    let write_result = dev.write_content(script.as_bytes(), COMPANION_INIT_SCRIPT);
    if write_result.is_err() {
        println!(
            "  {} Could not write boot script to {COMPANION_INIT_SCRIPT}",
            "!".yellow()
        );
        return;
    }
    println!("\n  Init script written to {}", COMPANION_INIT_SCRIPT.cyan());

    // Hook into rc.local
    let check = dev
        .shell(
            &format!("grep -q '{COMPANION_INIT_SCRIPT}' /etc/rc.local 2>/dev/null && echo exists || true"),
            5,
        )
        .unwrap_or_default();
    if check.trim() == "exists" {
        println!("  rc.local already references the init script.");
    } else {
        let has_exit = dev
            .shell("grep -q '^exit 0' /etc/rc.local 2>/dev/null && echo yes || true", 5)
            .unwrap_or_default();
        let cmd = if has_exit.trim() == "yes" {
            format!("sed -i '/^exit 0/i {COMPANION_INIT_SCRIPT} &' /etc/rc.local 2>&1 || echo READONLY")
        } else {
            format!("echo \"{COMPANION_INIT_SCRIPT} &\" >> /etc/rc.local 2>&1 || echo READONLY")
        };
        let result = dev.shell(&cmd, 5).unwrap_or_default();
        if result.contains("READONLY") || result.contains("Read-only") {
            println!(
                "  {} Could not modify /etc/rc.local (read-only filesystem).",
                "!".yellow()
            );
            println!(
                "  {} Bind-mount will be lost on reboot. Re-run {} to re-apply.",
                "Note:".bold(),
                "zte companion install".cyan()
            );
        } else {
            println!("  {} Boot persistence installed.", "OK".green());
        }
    }
}

/// Remove the boot script and its rc.local entry.
fn remove_boot_persistence(dev: &DeviceShell) {
    let _ = dev.shell(&format!("rm -f {COMPANION_INIT_SCRIPT}"), 5);

    let check = dev
        .shell(
            &format!("grep -q '{COMPANION_INIT_SCRIPT}' /etc/rc.local 2>/dev/null && echo exists || true"),
            5,
        )
        .unwrap_or_default();
    if check.trim() == "exists" {
        let result = dev
            .shell(
                &format!("sed -i '\\|{COMPANION_INIT_SCRIPT}|d' /etc/rc.local 2>&1 || echo READONLY"),
                5,
            )
            .unwrap_or_default();
        if result.contains("READONLY") || result.contains("Read-only") {
            println!(
                "  {} Could not remove entry from /etc/rc.local (read-only filesystem).",
                "!".yellow()
            );
        } else {
            println!("  {} Removed boot persistence.", "OK".green());
        }
    }
    let _ = dev.shell(&format!("rm -f {COMPANION_INIT_SCRIPT}"), 5);
}

