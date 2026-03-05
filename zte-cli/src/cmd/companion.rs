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
-- Methods: battery_current, cpu_usage, bandwidth, modem_online, modem_status,
--          call_dial, call_hangup, call_answer, call_status, call_dtmf, call_mute,
--          ussd_send, ussd_respond, ussd_cancel,
--          stk_get_menu, stk_select_item, stk_terminal_response,
--          wifi_status, wifi_set

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

-- GSM 7-bit default alphabet (build via string.char to avoid raw string issues)
local gsm7_map = {
    [0]="@", "\194\163","$", "\194\165","\195\168","\195\169","\195\185",
    "\195\172","\195\178","\195\135","\n","\195\152","\195\184","\r",
    "\195\133","\195\165","_","_"," ","!",string.char(34),
    string.char(35),"\194\164","%","&","'","(",")","*","+",",","-",".","/",
    "0","1","2","3","4","5","6","7","8","9",":",";","<","=",">","?",
    "\194\161","A","B","C","D","E","F","G","H","I","J","K","L","M",
    "N","O","P","Q","R","S","T","U","V","W","X","Y","Z",
    "\195\132","\195\150","\195\145","\195\156","\194\167",
    "\194\191","a","b","c","d","e","f","g","h","i","j","k","l","m",
    "n","o","p","q","r","s","t","u","v","w","x","y","z",
    "\195\164","\195\182","\195\177","\195\188","\195\160"
}

local function decode_gsm7(hex_str)
    if not hex_str or hex_str == "" then return "" end
    local bytes = {}
    for i = 1, #hex_str, 2 do
        bytes[#bytes + 1] = tonumber(hex_str:sub(i, i + 1), 16) or 0
    end
    -- Unpack 7-bit septets from 8-bit octets
    local septets = {}
    local shift = 0
    local bi = 1
    while bi <= #bytes do
        local val = bytes[bi]
        if bi > 1 then
            val = val * (2 ^ shift) + math.floor(bytes[bi - 1] / (2 ^ (8 - shift)))
        end
        septets[#septets + 1] = val % 128
        shift = shift + 1
        if shift == 7 then
            septets[#septets + 1] = math.floor(bytes[bi] / 2)
            shift = 0
        end
        bi = bi + 1
    end
    local out = {}
    for _, s in ipairs(septets) do
        local ch = gsm7_map[s]
        if ch then out[#out + 1] = ch end
    end
    return table.concat(out)
end

local function decode_ucs2(hex_str)
    if not hex_str or hex_str == "" then return "" end
    local out = {}
    for i = 1, #hex_str, 4 do
        local cp = tonumber(hex_str:sub(i, i + 3), 16)
        if cp then
            if cp < 0x80 then
                out[#out + 1] = string.char(cp)
            elseif cp < 0x800 then
                out[#out + 1] = string.char(0xC0 + math.floor(cp / 64), 0x80 + cp % 64)
            else
                out[#out + 1] = string.char(
                    0xE0 + math.floor(cp / 4096),
                    0x80 + math.floor(cp / 64) % 64,
                    0x80 + cp % 64)
            end
        end
    end
    return table.concat(out)
end

local function decode_ussd_response(hex_str, dcs)
    if not hex_str or hex_str == "" then return "" end
    dcs = dcs or 15
    local coding = dcs % 16
    if coding == 8 then
        return decode_ucs2(hex_str)
    elseif coding == 0 or coding == 15 then
        -- Try GSM 7-bit; if result is mostly printable, use it
        local decoded = decode_gsm7(hex_str)
        if decoded and #decoded > 0 then return decoded end
    end
    -- Fallback: try as plain ASCII hex
    local out = {}
    for i = 1, #hex_str, 2 do
        local b = tonumber(hex_str:sub(i, i + 1), 16)
        if b and b >= 32 and b < 127 then
            out[#out + 1] = string.char(b)
        end
    end
    if #out > 0 then return table.concat(out) end
    return hex_str
end

local function parse_cusd(raw)
    if not raw then return nil end
    local status, body, dcs = raw:match('%+CUSD:%s*(%d+)%s*[,"]+"?([^"]*)"?%s*,?%s*(%d*)')
    if not status then
        status = raw:match('%+CUSD:%s*(%d+)')
    end
    if status then
        return {
            status = tonumber(status) or -1,
            body = body or "",
            dcs = tonumber(dcs) or 15
        }
    end
    return nil
end

local function parse_stk_menu_tlv(hex)
    if not hex or hex == "" then return nil end
    local items = {}
    local title = ""
    local i = 1
    while i <= #hex - 3 do
        local tag = tonumber(hex:sub(i, i + 1), 16)
        local len = tonumber(hex:sub(i + 2, i + 3), 16)
        if not tag or not len then break end
        local val_start = i + 4
        local val_hex = hex:sub(val_start, val_start + len * 2 - 1)
        if tag == 0x85 then
            -- Alpha identifier (title)
            title = decode_ucs2(val_hex)
            if title == "" then
                local t = {}
                for j = 1, #val_hex, 2 do
                    local b = tonumber(val_hex:sub(j, j + 1), 16)
                    if b and b >= 32 then t[#t + 1] = string.char(b) end
                end
                title = table.concat(t)
            end
        elseif tag == 0x8F then
            -- Item: first byte = item ID, rest = text
            if #val_hex >= 4 then
                local item_id = tonumber(val_hex:sub(1, 2), 16) or 0
                local text_hex = val_hex:sub(3)
                local label = decode_ucs2(text_hex)
                if label == "" then
                    local t = {}
                    for j = 1, #text_hex, 2 do
                        local b = tonumber(text_hex:sub(j, j + 1), 16)
                        if b and b >= 32 then t[#t + 1] = string.char(b) end
                    end
                    label = table.concat(t)
                end
                items[#items + 1] = { id = item_id, label = label }
            end
        end
        i = val_start + len * 2
    end
    if #items > 0 or title ~= "" then
        return { title = title, items = items }
    end
    return nil
end

local function run_qcrilnr(commands, wait)
    wait = wait or 3
    local input = table.concat(commands, "\n") .. "\n"
    local tmpfile = "/tmp/qcrilnr_input_" .. os.time()
    local f = io.open(tmpfile, "w")
    if not f then return nil, "cannot write temp file" end
    f:write(input)
    f:close()
    local h = io.popen(string.format(
        "timeout %d qcrilnr-console-app < %s 2>&1; rm -f %s",
        wait, tmpfile, tmpfile), "r")
    if not h then return nil, "failed to run qcrilnr" end
    local out = h:read("*a")
    h:close()
    return out
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
    io.write('"call_mute":{"enabled":"boolean"},')
    io.write('"ussd_send":{"code":"string"},')
    io.write('"ussd_respond":{"reply":"string"},')
    io.write('"ussd_cancel":{},')
    io.write('"stk_get_menu":{},')
    io.write('"stk_select_item":{"item_id":"number"},')
    io.write('"stk_terminal_response":{"command":"string"},')
    io.write('"modem_online":{},')
    io.write('"modem_status":{},')
    io.write('"wifi_status":{},')
    io.write('"wifi_set":{"wifi_onoff":"string","wifi6_switch":"string","radio2_disabled":"string","radio5_disabled":"string","ssid_2g":"string","ssid_5g":"string","key_2g":"string","key_5g":"string","encryption_2g":"string","encryption_5g":"string","hidden_2g":"string","hidden_5g":"string","channel_2g":"string","channel_5g":"string","txpower_2g":"string","txpower_5g":"string","htmode_2g":"string","htmode_5g":"string"}}')

end

local function handle_call(method)
    if method == "battery_current" then
        -- Read from /sys/class/power_supply/*/current_now (and voltage_now)
        local dirs = { "/sys/class/power_supply/battery", "/sys/class/power_supply/BAT0" }
        for _, dir in ipairs(dirs) do
            local val = read_file(dir .. "/current_now")
            if val then
                local microamps = tonumber(val:match("%-?%d+"))
                if microamps then
                    local result = { current_now = microamps }
                    local vval = read_file(dir .. "/voltage_now")
                    if vval then
                        local microvolts = tonumber(vval:match("%-?%d+"))
                        if microvolts then
                            result.voltage_now = microvolts
                        end
                    end
                    io.write(json.encode(result))
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

    elseif method == "ussd_send" then
        local params = read_stdin_params()
        local code = params.code
        if not code or code == "" then
            io.write(json.encode({ error = "missing code" }))
            return
        end
        -- Sanitize: allow digits, *, #, +
        code = code:gsub("[^%d%*#%+]", "")
        local resp, err = send_at('AT+CUSD=1,"' .. code .. '",15', 8)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
            return
        end
        if resp:find("ERROR") then
            io.write(json.encode({ error = "USSD failed: " .. resp:match("ERROR.-\n") or "ERROR" }))
            return
        end
        local parsed = parse_cusd(resp)
        if parsed then
            local decoded = decode_ussd_response(parsed.body, parsed.dcs)
            -- If decoding produced garbage or empty, use raw body
            if not decoded or decoded == "" then decoded = parsed.body end
            local session = (parsed.status == 1)
            io.write(string.format(
                '{"response":"%s","raw_response":"%s","status":%d,"dcs":%d,"session_active":%s}',
                decoded:gsub('"', '\\"'):gsub("\n", "\\n"),
                (parsed.body or ""):gsub('"', '\\"'):gsub("\n", "\\n"),
                parsed.status, parsed.dcs,
                session and "true" or "false"))
        else
            -- No +CUSD in response, return raw
            local clean = resp:gsub("[%c]", " "):gsub('"', '\\"')
            io.write(string.format('{"response":"%s","raw_response":"%s","status":-1,"dcs":15,"session_active":false}',
                clean, clean))
        end

    elseif method == "ussd_respond" then
        local params = read_stdin_params()
        local reply = params.reply
        if not reply or reply == "" then
            io.write(json.encode({ error = "missing reply" }))
            return
        end
        reply = reply:gsub("[^%d%*#%+]", "")
        local resp, err = send_at('AT+CUSD=1,"' .. reply .. '",15', 8)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
            return
        end
        local parsed = parse_cusd(resp)
        if parsed then
            local decoded = decode_ussd_response(parsed.body, parsed.dcs)
            if not decoded or decoded == "" then decoded = parsed.body end
            local session = (parsed.status == 1)
            io.write(string.format(
                '{"response":"%s","raw_response":"%s","status":%d,"dcs":%d,"session_active":%s}',
                decoded:gsub('"', '\\"'):gsub("\n", "\\n"),
                (parsed.body or ""):gsub('"', '\\"'):gsub("\n", "\\n"),
                parsed.status, parsed.dcs,
                session and "true" or "false"))
        else
            local clean = resp:gsub("[%c]", " "):gsub('"', '\\"')
            io.write(string.format('{"response":"%s","raw_response":"%s","status":-1,"dcs":15,"session_active":false}',
                clean, clean))
        end

    elseif method == "ussd_cancel" then
        local resp, err = send_at("AT+CUSD=2", 3)
        if not resp then
            io.write(json.encode({ error = err or "AT command failed" }))
        else
            io.write(json.encode({ status = "ok" }))
        end

    elseif method == "stk_get_menu" then
        -- Try AT+CUSATD first (ZTE/Qualcomm STK AT interface)
        local resp = send_at("AT+CUSATD=1", 5)
        if resp and not resp:find("ERROR") then
            -- Look for hex TLV payload in response
            local hex = resp:match("CUSATD:%s*(%x+)") or resp:match("\n(%x%x%x%x+)\r?\n")
            if hex then
                local menu = parse_stk_menu_tlv(hex)
                if menu then
                    local parts = {}
                    for _, item in ipairs(menu.items) do
                        parts[#parts + 1] = string.format('{"id":%d,"label":"%s"}',
                            item.id, item.label:gsub('"', '\\"'))
                    end
                    io.write(string.format('{"title":"%s","items":[%s],"source":"at_cusatd"}',
                        (menu.title or ""):gsub('"', '\\"'), table.concat(parts, ",")))
                    return
                end
            end
        end
        -- Try AT+STIN? / AT+STGI fallback
        resp = send_at("AT+STIN?", 3)
        if resp and resp:find("STIN") then
            local stin_type = resp:match("STIN:%s*(%d+)")
            if stin_type == "37" or stin_type == "25" then
                local stgi = send_at("AT+STGI=" .. stin_type, 5)
                if stgi and not stgi:find("ERROR") then
                    local title = stgi:match('STGI:%s*"([^"]*)"') or "SIM Menu"
                    local items = {}
                    for id, label in stgi:gmatch('STGI:%s*(%d+)%s*,%s*%d+%s*,"([^"]*)"') do
                        items[#items + 1] = string.format('{"id":%d,"label":"%s"}',
                            tonumber(id) or 0, label:gsub('"', '\\"'))
                    end
                    if #items > 0 then
                        io.write(string.format('{"title":"%s","items":[%s],"source":"at_stgi"}',
                            title:gsub('"', '\\"'), table.concat(items, ",")))
                        return
                    end
                end
            end
        end
        -- Try qcrilnr Envelope as last resort
        local qout = run_qcrilnr({
            "1",   -- Card_Services
            "5",   -- Envelope_Req (STK envelope)
            "A0C00000011901",  -- SELECT MF + setup menu envelope
            "q"    -- quit
        }, 5)
        if qout and qout:find("SUCCESS") then
            local hex = qout:match("data%s*=%s*(%x+)") or qout:match("(%x%x%x%x%x%x+)")
            if hex then
                local menu = parse_stk_menu_tlv(hex)
                if menu and #menu.items > 0 then
                    local parts = {}
                    for _, item in ipairs(menu.items) do
                        parts[#parts + 1] = string.format('{"id":%d,"label":"%s"}',
                            item.id, item.label:gsub('"', '\\"'))
                    end
                    io.write(string.format('{"title":"%s","items":[%s],"source":"qcrilnr"}',
                        (menu.title or ""):gsub('"', '\\"'), table.concat(parts, ",")))
                    return
                end
            end
        end
        io.write('{"error":"STK menu not available","hint":"SIM may not support STK or no proactive command pending"}')

    elseif method == "stk_select_item" then
        local params = read_stdin_params()
        local item_id = tonumber(params.item_id)
        if not item_id then
            io.write(json.encode({ error = "missing item_id" }))
            return
        end
        -- Build envelope: Menu Selection TLV
        local envelope = string.format("D30782020181900101%02X", item_id)
        -- Try AT+CUSATE first
        local resp = send_at('AT+CUSATE="' .. envelope .. '"', 8)
        if resp and not resp:find("ERROR") then
            -- Check for sub-menu TLV
            local hex = resp:match("CUSATE:%s*(%x+)") or resp:match("\n(%x%x%x%x+)\r?\n")
            if hex then
                local menu = parse_stk_menu_tlv(hex)
                if menu and #menu.items > 0 then
                    local parts = {}
                    for _, item in ipairs(menu.items) do
                        parts[#parts + 1] = string.format('{"id":%d,"label":"%s"}',
                            item.id, item.label:gsub('"', '\\"'))
                    end
                    io.write(string.format('{"type":"menu","title":"%s","items":[%s],"source":"at_cusate"}',
                        (menu.title or ""):gsub('"', '\\"'), table.concat(parts, ",")))
                    return
                end
            end
            -- Display text or other response
            local text = resp:match('"([^"]+)"') or resp:gsub("[%c]", " ")
            io.write(string.format('{"type":"display","data":"%s"}', text:gsub('"', '\\"')))
            return
        end
        -- Fallback: qcrilnr Envelope_Req
        local qout = run_qcrilnr({
            "1",          -- Card_Services
            "5",          -- Envelope_Req
            envelope,
            "q"
        }, 5)
        if qout and qout:find("SUCCESS") then
            local hex = qout:match("data%s*=%s*(%x+)") or qout:match("(%x%x%x%x%x%x+)")
            if hex then
                local menu = parse_stk_menu_tlv(hex)
                if menu and #menu.items > 0 then
                    local parts = {}
                    for _, item in ipairs(menu.items) do
                        parts[#parts + 1] = string.format('{"id":%d,"label":"%s"}',
                            item.id, item.label:gsub('"', '\\"'))
                    end
                    io.write(string.format('{"type":"menu","title":"%s","items":[%s],"source":"qcrilnr"}',
                        (menu.title or ""):gsub('"', '\\"'), table.concat(parts, ",")))
                    return
                end
            end
            io.write('{"type":"display","data":"Command sent"}')
        else
            io.write(json.encode({ error = "item selection failed" }))
        end

    elseif method == "stk_terminal_response" then
        local params = read_stdin_params()
        local command = params.command
        if not command or command == "" then
            io.write(json.encode({ error = "missing command" }))
            return
        end
        -- Try AT+CUSATR first
        local resp = send_at('AT+CUSATR="' .. command .. '"', 5)
        if resp and not resp:find("ERROR") then
            io.write(json.encode({ status = "ok", source = "at_cusatr" }))
            return
        end
        -- Fallback: qcrilnr TerminalResp
        local qout = run_qcrilnr({
            "1",   -- Card_Services
            "7",   -- TerminalResp_command
            command,
            "q"
        }, 5)
        if qout and qout:find("SUCCESS") then
            io.write(json.encode({ status = "ok", source = "qcrilnr" }))
        else
            io.write(json.encode({ error = "terminal response failed" }))
        end

    elseif method == "modem_online" then
        -- Send AT+CFUN=1 to bring modem back online (firmware workaround for broken nwinfo_set_mode ONLINE)
        -- 8s wait: modem waking from LPM (especially with pending generation change) can take >4s to respond
        local resp, err = send_at("AT+CFUN=1", 8)
        if resp and resp:find("OK") then
            io.write(json.encode({ status = "ok" }))
        else
            io.write(json.encode({ error = err or "AT+CFUN=1 failed", raw = resp or "" }))
        end

    elseif method == "modem_status" then
        -- Read modem operating mode from UCI (always reliable, unlike nwinfo_get_netinfo which omits operate_mode)
        local h = io.popen("uci get zte_nwinfo.sys_info.operate_mode 2>/dev/null")
        local mode = h:read("*l") or ""
        h:close()
        io.write(json.encode({ operate_mode = mode }))

    elseif method == "wifi_status" then
        -- Read WiFi config from UCI + runtime info from iwinfo
        local function uci_get(key)
            local h = io.popen("uci get wireless." .. key .. " 2>/dev/null")
            if not h then return "" end
            local v = h:read("*l") or ""
            h:close()
            return v
        end
        local function uci_get_mbb(key)
            local h = io.popen("uci get zte_mbb." .. key .. " 2>/dev/null")
            if not h then return "" end
            local v = h:read("*l") or ""
            h:close()
            return v
        end
        local function iw_info(iface)
            local h = io.popen("iw " .. iface .. " info 2>/dev/null")
            if not h then return "", "" end
            local out = h:read("*a") or ""
            h:close()
            local ch = out:match("channel (%d+)")
            local bw = out:match("width: (%d+ MHz)")
            return ch or "", bw or ""
        end
        local function assoclist_count(iface)
            local h = io.popen("iw " .. iface .. " station dump 2>/dev/null | grep -c Station")
            if not h then return 0 end
            local v = tonumber(h:read("*l")) or 0
            h:close()
            return v
        end
        local result = {}
        -- Global switches from zte_mbb
        result.wifi_onoff = uci_get_mbb("wifi.wifi_onoff")
        result.wifi6_switch = uci_get_mbb("wifi.wifi6_switch")
        -- Fallback: if mbb UCI keys are missing, read from zwrt_wlan report
        if result.wifi_onoff == "" or result.wifi6_switch == "" then
            local h = io.popen("ubus call zwrt_wlan report '{}' 2>/dev/null")
            if h then
                local out = h:read("*a") or ""
                h:close()
                if result.wifi_onoff == "" then
                    result.wifi_onoff = out:match('"wifi_onoff"%s*:%s*"(%d)"') or "1"
                end
                if result.wifi6_switch == "" then
                    result.wifi6_switch = out:match('"wifi6_switch"%s*:%s*"(%d)"') or "0"
                end
            end
        end
        -- Radio config
        result.radio2_disabled = uci_get("wifi0.disabled")
        result.radio5_disabled = uci_get("wifi1.disabled")
        result.channel_2g = uci_get("wifi0.channel")
        result.channel_5g = uci_get("wifi1.channel")
        result.txpower_2g = uci_get("wifi0.txpowerpercent")
        result.txpower_5g = uci_get("wifi1.txpowerpercent")
        result.htmode_2g = uci_get("wifi0.htmode")
        result.htmode_5g = uci_get("wifi1.htmode")
        result.country_code = uci_get("wifi0.country")
        -- Interface config (SSID, key, encryption, hidden)
        result.ssid_2g = uci_get("main_2g.ssid")
        result.ssid_5g = uci_get("main_5g.ssid")
        result.key_2g = uci_get("main_2g.key")
        result.key_5g = uci_get("main_5g.key")
        result.encryption_2g = uci_get("main_2g.encryption")
        result.encryption_5g = uci_get("main_5g.encryption")
        result.hidden_2g = uci_get("main_2g.hidden")
        result.hidden_5g = uci_get("main_5g.hidden")
        -- Runtime info from iw
        result.actual_channel_2g, result.actual_bw_2g = iw_info("wlan0")
        result.actual_channel_5g, result.actual_bw_5g = iw_info("wlan2")
        -- Client counts
        local c2g = assoclist_count("wlan0")
        local c5g = assoclist_count("wlan2")
        result.clients_2g = c2g
        result.clients_5g = c5g
        result.clients_total = c2g + c5g
        io.write(json.encode(result))

    elseif method == "wifi_set" then
        -- Accept params, write to UCI, commit and reload
        local raw = io.read("*a")
        local params = json.decode(raw or "")
        if not params or type(params) ~= "table" then
            io.write(json.encode({ error = "invalid params" }))
            return
        end
        -- Map param names to UCI paths
        local uci_map = {
            ssid_2g = "wireless.main_2g.ssid",
            ssid_5g = "wireless.main_5g.ssid",
            key_2g = "wireless.main_2g.key",
            key_5g = "wireless.main_5g.key",
            encryption_2g = "wireless.main_2g.encryption",
            encryption_5g = "wireless.main_5g.encryption",
            hidden_2g = "wireless.main_2g.hidden",
            hidden_5g = "wireless.main_5g.hidden",
            channel_2g = "wireless.wifi0.channel",
            channel_5g = "wireless.wifi1.channel",
            txpower_2g = "wireless.wifi0.txpowerpercent",
            txpower_5g = "wireless.wifi1.txpowerpercent",
            htmode_2g = "wireless.wifi0.htmode",
            htmode_5g = "wireless.wifi1.htmode",
            radio2_disabled = "wireless.wifi0.disabled",
            radio5_disabled = "wireless.wifi1.disabled",
        }
        local mbb_map = {
            wifi_onoff = "zte_mbb.wifi.wifi_onoff",
            wifi6_switch = "zte_mbb.wifi.wifi6_switch",
        }
        local wireless_changed = false
        local mbb_changed = false
        for k, v in pairs(params) do
            local path = uci_map[k]
            if path then
                local h = io.popen("uci get " .. path .. " 2>/dev/null")
                local cur = h and h:read("*l") or ""
                if h then h:close() end
                local new_val = tostring(v):gsub("'", "")
                if cur ~= new_val then
                    os.execute("uci set " .. path .. "='" .. new_val .. "'")
                    wireless_changed = true
                end
            else
                path = mbb_map[k]
                if path then
                    local h = io.popen("uci get " .. path .. " 2>/dev/null")
                    local cur = h and h:read("*l") or ""
                    if h then h:close() end
                    local new_val = tostring(v):gsub("'", "")
                    if cur ~= new_val then
                        os.execute("uci set " .. path .. "='" .. new_val .. "'")
                        mbb_changed = true
                    end
                end
            end
        end
        if not wireless_changed and not mbb_changed then
            io.write(json.encode({ status = "ok", note = "no changes" }))
            return
        end
        if wireless_changed then os.execute("uci commit wireless") end
        if mbb_changed then os.execute("uci commit zte_mbb") end
        io.write(json.encode({ status = "ok" }))
        if wireless_changed then
            os.execute("ubus call zwrt_wlan reload >/dev/null 2>&1 &")
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
    /// Re-apply bind-mount and restart rpcd (fixes plugin after reboot)
    Repair {
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
        Cmd::Repair { shell } => run_repair(&shell),
        Cmd::Remove { shell } => run_remove(&shell),
    }
}

fn run_install(shell: &ShellArgs) -> Result<()> {
    let dev = shell.connect()?;
    println!("\n  {}\n", "Companion — Installing rpcd plugin".bold());
    install_companion_on_device(&dev, shell.password.as_deref(), shell.gateway.as_deref(), true)?;
    println!();
    Ok(())
}

/// Install companion on a pre-connected device. Returns Ok(true) if installed.
pub(crate) fn install_companion_on_device(
    dev: &DeviceShell,
    password: Option<&str>,
    gateway: Option<&str>,
    restart_rpcd: bool,
) -> Result<bool> {
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

    if restart_rpcd {
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
    }

    // 6. Install boot persistence
    install_boot_persistence(dev);

    // 7. Optionally verify via HTTP
    if let Some(pw) = password {
        println!("\n  Verifying via HTTP ubus call...");
        let mut client = UbusClient::new(gateway, 10);
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
                // Verify USSD endpoint is registered (safe IMEI query)
                let ussd_params = serde_json::json!({"code": "*#06#"});
                match client.call_anon("zte-companion", "ussd_send",
                    Some(&ussd_params))
                {
                    Ok(resp) => {
                        if resp.get("response").is_some() || resp.get("raw_response").is_some() {
                            println!(
                                "  {} zte-companion.ussd_send is working!",
                                "OK".green().bold()
                            );
                        } else if resp.get("error").is_some() {
                            println!(
                                "  {} ussd_send registered (AT port may be busy): {}",
                                "OK".yellow(),
                                resp.get("error").unwrap()
                            );
                        } else {
                            println!(
                                "  {} Got unexpected ussd_send response: {}",
                                "!".yellow(),
                                resp
                            );
                        }
                    }
                    Err(e) => println!(
                        "  {} ussd_send verification failed: {e}",
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

    Ok(true)
}

fn run_repair(shell: &ShellArgs) -> Result<()> {
    let dev = shell.connect()?;
    println!("\n  {}\n", "Companion — Repairing plugin".bold());

    // Check if plugin file exists in overlay
    let file_check = dev
        .shell(
            &format!("test -x {RPCD_OVERLAY_DIR}/zte-companion && echo yes || echo no"),
            5,
        )
        .unwrap_or_default();
    if file_check.trim() != "yes" {
        println!(
            "  {} Plugin file not found in {}. Run {} instead.",
            "FAIL".red(),
            RPCD_OVERLAY_DIR.cyan(),
            "zte companion install".cyan()
        );
        println!();
        return Ok(());
    }
    println!("  {} Plugin file exists.", "OK".green());

    // Check if bind-mount is already active
    let mount_check = dev
        .shell(&format!("mount | grep '{RPCD_PLUGIN_DIR}'"), 5)
        .unwrap_or_default();
    if !mount_check.trim().is_empty() {
        println!("  Bind-mount already active, re-applying...");
        let _ = dev.shell(&format!("umount {RPCD_PLUGIN_DIR} 2>/dev/null; true"), 5);
    }

    // Re-apply bind-mount
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

    // Restart rpcd
    println!("  Restarting rpcd...");
    let restart_result = dev.shell("/etc/init.d/rpcd restart", 10);
    match restart_result {
        Ok(_) => println!("  {} rpcd restarted.", "OK".green()),
        Err(e) => {
            println!("  {} restart failed ({}), trying HUP...", "!".yellow(), e);
            let _ = dev.shell("kill -HUP $(pidof rpcd)", 5);
        }
    }

    std::thread::sleep(std::time::Duration::from_secs(1));

    // Verify
    let rpcd_check = dev
        .shell("ubus call zte-companion cpu_usage '{}' 2>&1", 5)
        .unwrap_or_default();
    if rpcd_check.contains("idle") && rpcd_check.contains("total") {
        println!("  {} Plugin is working!", "OK".green().bold());
    } else {
        println!(
            "  {} rpcd not responding to zte-companion. Try {}.",
            "!".yellow(),
            "zte companion install".cyan()
        );
    }

    // Re-install boot persistence
    install_boot_persistence(&dev);

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

    // Check boot persistence (rc.local or crontab)
    let rc_local_check = dev
        .shell(&format!(
            "grep -q '{COMPANION_INIT_SCRIPT}' /etc/rc.local 2>/dev/null && echo yes || echo no"
        ), 5)
        .unwrap_or_default();
    let cron_check = dev
        .shell(&format!(
            "crontab -l 2>/dev/null | grep -q '{COMPANION_INIT_SCRIPT}' && echo yes || echo no"
        ), 5)
        .unwrap_or_default();
    let boot_persistent = rc_local_check.trim() == "yes" || cron_check.trim() == "yes";

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
        println!("\n  {} Plugin file exists but bind-mount not active (lost after reboot?).", "!".yellow());
        println!("      Run {} to re-apply quickly.", "zte companion repair".cyan());
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
/// Falls back to crontab @reboot if rc.local is on a read-only filesystem.
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
    let _ = dev.shell(&format!("chmod +x {COMPANION_INIT_SCRIPT}"), 5);
    println!("\n  Init script written to {}", COMPANION_INIT_SCRIPT.cyan());

    // Try rc.local first
    let check = dev
        .shell(
            &format!("grep -q '{COMPANION_INIT_SCRIPT}' /etc/rc.local 2>/dev/null && echo exists || true"),
            5,
        )
        .unwrap_or_default();
    if check.trim() == "exists" {
        println!("  rc.local already references the init script.");
        return;
    }

    let has_exit = dev
        .shell("grep -q '^exit 0' /etc/rc.local 2>/dev/null && echo yes || true", 5)
        .unwrap_or_default();
    let cmd = if has_exit.trim() == "yes" {
        format!("sed -i '/^exit 0/i {COMPANION_INIT_SCRIPT} &' /etc/rc.local 2>&1 || echo READONLY")
    } else {
        format!("echo \"{COMPANION_INIT_SCRIPT} &\" >> /etc/rc.local 2>&1 || echo READONLY")
    };
    let result = dev.shell(&cmd, 5).unwrap_or_default();
    if !result.contains("READONLY") && !result.contains("Read-only") {
        println!("  {} Boot persistence installed (rc.local).", "OK".green());
        return;
    }

    // rc.local is read-only — fall back to crontab @reboot
    println!(
        "  {} /etc/rc.local is read-only, trying crontab @reboot fallback...",
        "!".yellow()
    );
    let cron_check = dev
        .shell(
            &format!("crontab -l 2>/dev/null | grep -q '{COMPANION_INIT_SCRIPT}' && echo exists || true"),
            5,
        )
        .unwrap_or_default();
    if cron_check.trim() == "exists" {
        println!("  crontab already has @reboot entry.");
        return;
    }
    let cron_result = dev
        .shell(
            &format!(
                "(crontab -l 2>/dev/null; echo '@reboot {COMPANION_INIT_SCRIPT}') | crontab - 2>&1"
            ),
            5,
        )
        .unwrap_or_default();
    if cron_result.contains("not found") || cron_result.contains("No such") {
        println!(
            "  {} crontab not available either. Bind-mount will be lost on reboot.",
            "!".yellow()
        );
        println!(
            "  {} Re-run {} after reboot to re-apply.",
            "Note:".bold(),
            "zte companion repair".cyan()
        );
    } else {
        // Ensure crond is running
        let _ = dev.shell("/etc/init.d/cron enable 2>/dev/null; /etc/init.d/cron start 2>/dev/null; true", 5);
        println!("  {} Boot persistence installed (crontab @reboot).", "OK".green());
    }
}

/// Remove the boot script and its rc.local / crontab entries.
fn remove_boot_persistence(dev: &DeviceShell) {
    let _ = dev.shell(&format!("rm -f {COMPANION_INIT_SCRIPT}"), 5);

    // Remove from rc.local
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
            println!("  {} Removed rc.local boot persistence.", "OK".green());
        }
    }

    // Remove from crontab
    let cron_check = dev
        .shell(
            &format!("crontab -l 2>/dev/null | grep -q '{COMPANION_INIT_SCRIPT}' && echo exists || true"),
            5,
        )
        .unwrap_or_default();
    if cron_check.trim() == "exists" {
        let _ = dev.shell(
            &format!(
                "crontab -l 2>/dev/null | grep -v '{COMPANION_INIT_SCRIPT}' | crontab - 2>/dev/null; true"
            ),
            5,
        );
        println!("  {} Removed crontab boot persistence.", "OK".green());
    }
}

