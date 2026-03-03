#!/usr/bin/env bash
#
# cpu-compare.sh — Compare CPU usage from SSH /proc/stat vs HTTP load average
#
# Requires: curl, jq, ssh
#
set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
HOST="192.168.0.1"
SSH_PORT="2222"
PASSWORD="${ZTE_PASSWORD:-}"
ITERATIONS=3
SSH_KEY="$HOME/.ssh/id_ed25519"
SSH_OPTS=(-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR)

# ── Argument parsing ─────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Compare CPU usage: SSH /proc/stat delta (ground truth) vs HTTP ubus methods.

Options:
  --host HOST        Router IP (default: 192.168.0.1)
  --ssh-port PORT    SSH port (default: 2222)
  --password PASS    Router password (default: \$ZTE_PASSWORD or prompt)
  --iterations N     Number of comparison rounds (default: 3)
  -h, --help         Show this help
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)       HOST="$2"; shift 2 ;;
        --ssh-port)   SSH_PORT="$2"; shift 2 ;;
        --password)   PASSWORD="$2"; shift 2 ;;
        --iterations) ITERATIONS="$2"; shift 2 ;;
        -h|--help)    usage ;;
        *)            echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

if [[ -z "$PASSWORD" ]]; then
    read -rsp "Router password: " PASSWORD
    echo
fi

# ── HTTP helpers ─────────────────────────────────────────────────────────────
UBUS_BASE="http://${HOST}/ubus/"
SESSION_ID=""
RPC_ID=0

next_id() { RPC_ID=$((RPC_ID + 1)); echo "$RPC_ID"; }
timestamp_ms() { date +%s000; }

# Low-level JSON-RPC post (wraps payload in array, appends ?t=)
ubus_rpc() {
    local payload="$1"
    local url="${UBUS_BASE}?t=$(timestamp_ms)"
    local resp
    resp=$(curl -s --max-time 10 "$url" \
        -H 'Content-Type: application/json' \
        -d "[${payload}]")
    if [[ -z "$resp" ]]; then
        echo '{"error":"empty response"}'
        return
    fi
    # Response is an array; return first element
    echo "$resp" | jq '.[0]'
}

# Authenticated ubus call: obj.method(params)
# Returns the full JSON-RPC response object
ubus_call() {
    local obj="$1" method="$2" params="$3"
    local id
    id=$(next_id)
    local payload="{\"jsonrpc\":\"2.0\",\"id\":${id},\"method\":\"call\",\"params\":[\"${SESSION_ID}\",\"${obj}\",\"${method}\",${params}]}"
    ubus_rpc "$payload"
}

# Check if ubus call succeeded (result[0] == 0)
ubus_ok() {
    local resp="$1"
    local code
    code=$(echo "$resp" | jq -r '.result[0] // "null"')
    [[ "$code" == "0" ]]
}

# ZTE-specific login: get salt, double-SHA256 hash, then web_login
http_login() {
    local anon="00000000000000000000000000000000"

    # Step 1: Get salt via zwrt_web.web_login_info
    local id resp salt
    id=$(next_id)
    resp=$(ubus_rpc "{\"jsonrpc\":\"2.0\",\"id\":${id},\"method\":\"call\",\"params\":[\"${anon}\",\"zwrt_web\",\"web_login_info\",{}]}")
    salt=$(echo "$resp" | jq -r '.result[1].zte_web_sault // .result[1].salt // empty')
    if [[ -z "$salt" ]]; then
        echo "ERROR: Failed to get login salt" >&2
        echo "Response: $resp" >&2
        exit 1
    fi

    # Step 2: Double-SHA256 hash: UPPER(SHA256(UPPER(SHA256(password)) + salt))
    local hash1 hash2
    hash1=$(printf '%s' "$PASSWORD" | shasum -a 256 | awk '{print toupper($1)}')
    hash2=$(printf '%s' "${hash1}${salt}" | shasum -a 256 | awk '{print toupper($1)}')

    # Step 3: Login with hashed password
    id=$(next_id)
    resp=$(ubus_rpc "{\"jsonrpc\":\"2.0\",\"id\":${id},\"method\":\"call\",\"params\":[\"${anon}\",\"zwrt_web\",\"web_login\",{\"password\":\"${hash2}\"}]}")
    SESSION_ID=$(echo "$resp" | jq -r '.result[1].ubus_rpc_session // empty')
    if [[ -z "$SESSION_ID" || "$SESSION_ID" == "$anon" ]]; then
        echo "ERROR: HTTP login failed" >&2
        echo "Response: $resp" >&2
        exit 1
    fi
}

# ── Parse /proc/stat aggregate cpu line ──────────────────────────────────────
# Returns: user nice system idle iowait irq softirq steal (space-separated)
parse_cpu_line() {
    local text="$1"
    echo "$text" | grep '^cpu ' | awk '{print $2, $3, $4, $5, $6, $7, $8, $9}'
}

# Count cores from /proc/stat (lines matching cpu0, cpu1, ...)
count_cores() {
    local text="$1"
    echo "$text" | grep -c '^cpu[0-9]'
}

# Compute CPU% from two samples: (1 - delta_idle / delta_total) * 100
compute_cpu_pct() {
    local before="$1" after="$2"
    read -ra b <<< "$before"
    read -ra a <<< "$after"

    local b_total=0 a_total=0
    for i in "${!b[@]}"; do
        b_total=$((b_total + b[i]))
        a_total=$((a_total + a[i]))
    done

    local b_idle="${b[3]}" a_idle="${a[3]}"
    local d_total=$((a_total - b_total))
    local d_idle=$((a_idle - b_idle))

    if [[ $d_total -eq 0 ]]; then
        echo "0.0"
        return
    fi

    awk "BEGIN { printf \"%.1f\", (1 - ${d_idle}/${d_total}) * 100 }"
}

# ── SSH helper ───────────────────────────────────────────────────────────────
ssh_read_proc_stat() {
    ssh "${SSH_OPTS[@]}" -i "$SSH_KEY" -p "$SSH_PORT" "root@${HOST}" cat /proc/stat
}

# ── HTTP read /proc/stat via file.read ───────────────────────────────────────
http_read_proc_stat() {
    local resp
    resp=$(ubus_call "file" "read" '{"path":"/proc/stat"}')
    if ubus_ok "$resp"; then
        echo "$resp" | jq -r '.result[1].data // empty'
    fi
    # Returns empty if permission denied (error code 6)
}

# ── HTTP system.info ─────────────────────────────────────────────────────────
http_system_info() {
    ubus_call "system" "info" '{}'
}

# ── Main ─────────────────────────────────────────────────────────────────────
echo "Logging in via HTTP..."
http_login
echo "Session: ${SESSION_ID:0:8}..."
echo

# Get core count from first SSH read
initial_stat=$(ssh_read_proc_stat)
CORES=$(count_cores "$initial_stat")
echo "Detected ${CORES} CPU cores"

# Probe whether file.read is available over HTTP
FILE_READ_OK=false
probe=$(ubus_call "file" "read" '{"path":"/proc/stat"}')
if ubus_ok "$probe"; then
    FILE_READ_OK=true
    echo "HTTP file.read: available"
else
    echo "HTTP file.read: denied (ACL restriction) — skipping HTTP /proc/stat delta"
fi
echo

divider="────────────────────────────────────────"

for ((i = 1; i <= ITERATIONS; i++)); do
    echo "═══ Round $i / $ITERATIONS ═══"
    echo

    # Fresh login for each round to avoid session expiry
    http_login >/dev/null

    # ── Sample 1 ──────────────────────────────────────────────────────────
    ssh_stat1=$(ssh_read_proc_stat)
    ssh_cpu1=$(parse_cpu_line "$ssh_stat1")

    http_cpu1=""
    if $FILE_READ_OK; then
        http_stat1=$(http_read_proc_stat)
        http_cpu1=$(parse_cpu_line "$http_stat1")
    fi

    # Grab system.info load average (right after login while session is fresh)
    sysinfo=$(http_system_info)
    load_raw=$(echo "$sysinfo" | jq -r '.result[1].load[0] // empty')

    # ── Wait ──────────────────────────────────────────────────────────────
    echo "  Sampling... (2s sleep)"
    sleep 2

    # ── Sample 2 ──────────────────────────────────────────────────────────
    ssh_stat2=$(ssh_read_proc_stat)
    ssh_cpu2=$(parse_cpu_line "$ssh_stat2")

    http_cpu2=""
    if $FILE_READ_OK; then
        http_stat2=$(http_read_proc_stat)
        http_cpu2=$(parse_cpu_line "$http_stat2")
    fi

    # ── Compute deltas ────────────────────────────────────────────────────
    ssh_pct=$(compute_cpu_pct "$ssh_cpu1" "$ssh_cpu2")

    http_pct="N/A (file.read denied)"
    if $FILE_READ_OK && [[ -n "$http_cpu1" && -n "$http_cpu2" ]]; then
        http_pct="$(compute_cpu_pct "$http_cpu1" "$http_cpu2")%"
    fi

    # Load average calculations
    if [[ -n "$load_raw" && "$load_raw" != "null" ]]; then
        load_avg=$(awk "BEGIN { printf \"%.2f\", ${load_raw} / 65536 }")
        load_pct_raw=$(awk "BEGIN { printf \"%.1f\", (${load_raw} / 65536) * 100 }")
        load_pct_fixed=$(awk "BEGIN { v = (${load_raw} / 65536 / ${CORES}) * 100; if (v > 100) v = 100; printf \"%.1f\", v }")
    else
        load_avg="N/A"
        load_pct_raw="N/A"
        load_pct_fixed="N/A"
    fi

    # ── Print table ───────────────────────────────────────────────────────
    echo
    echo "  CPU Usage Comparison"
    echo "  $divider"
    printf "  %-34s %s%%\n" "SSH /proc/stat (delta):" "$ssh_pct"
    printf "  %-34s %s\n"   "HTTP /proc/stat (delta):" "$http_pct"
    printf "  %-34s %s%% (raw, load_avg=%s)\n" "HTTP load avg (old formula):" "$load_pct_raw" "$load_avg"
    printf "  %-34s %s%% (÷${CORES} cores, capped)\n" "HTTP load avg (fixed formula):" "$load_pct_fixed"
    printf "  %-34s %s\n" "Core count:" "$CORES"
    printf "  %-34s %s\n" "load[0] raw:" "$load_raw"
    echo "  $divider"
    echo

    if [[ $i -lt $ITERATIONS ]]; then
        sleep 1
    fi
done

echo "Done."
echo "• SSH /proc/stat delta is the ground truth."
echo "• The fixed load avg formula (÷cores, capped) should be a rough ballpark."
echo "• The old formula (no core division) shows the broken 100%+ value."
