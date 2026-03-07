#!/bin/bash
set -e

# ── Colors ───────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}[*]${NC} $1"; }
ok()    { echo -e "${GREEN}[+]${NC} $1"; }
warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
fail()  { echo -e "${RED}[-]${NC} $1" >&2; exit 1; }

# ── Usage ────────────────────────────────────────────────────────────
ROUTER_PASSWORD="${1:?Usage: ./setup.sh <router-password> <agent-password>}"
AGENT_PASSWORD="${2:?Usage: ./setup.sh <router-password> <agent-password>}"

GATEWAY=192.168.0.1
AGENT_PORT=9090
SSH_PORT=2222
TARGET=aarch64-unknown-linux-musl
BINARY=target/$TARGET/release/zte-agent
REMOTE_BIN=/data/zte-agent
STARTUP_SCRIPT=/data/local/tmp/start_zte_agent.sh
BINARY_CHANGED=false

# ── Step 0: Prerequisites ───────────────────────────────────────────
info "Checking prerequisites..."
MISSING=()
for cmd in adb cargo curl python3 aarch64-linux-musl-gcc; do
    command -v "$cmd" >/dev/null 2>&1 || MISSING+=("$cmd")
done
if [ ${#MISSING[@]} -ne 0 ]; then
    fail "Missing required tools: ${MISSING[*]}
  Install them before running this script:
    adb                       — Android SDK Platform Tools
                                macOS: brew install android-platform-tools
                                Linux: sudo apt install android-tools-adb
    cargo                     — Rust toolchain (https://rustup.rs)
    aarch64-linux-musl-gcc    — musl cross-linker
                                macOS: brew install filosottile/musl-cross/musl-cross
                                Linux: sudo apt install musl-tools gcc-aarch64-linux-gnu
    curl                      — HTTP client (usually pre-installed)
    python3                   — Python 3 (usually pre-installed)"
fi

if ! rustup target list --installed 2>/dev/null | grep -q aarch64-unknown-linux-musl; then
    info "Adding Rust cross-compilation target..."
    rustup target add aarch64-unknown-linux-musl
fi
ok "All prerequisites found."

# ── Helper: SHA-256 ──────────────────────────────────────────────────
sha256() {
    echo -n "$1" | shasum -a 256 2>/dev/null | awk '{print $1}' \
        || echo -n "$1" | sha256sum | awk '{print $1}'
}

sha256_file() {
    shasum -a 256 "$1" 2>/dev/null | awk '{print $1}' \
        || sha256sum "$1" | awk '{print $1}'
}

upper() {
    echo "$1" | tr '[:lower:]' '[:upper:]'
}

# ── Helper: ubus JSON-RPC call ───────────────────────────────────────
ubus_call() {
    local session="$1" object="$2" method="$3" params="$4"
    local ts
    ts=$(date +%s)
    curl -sf "http://$GATEWAY/ubus/?t=$ts" \
        -H 'Content-Type: application/json' \
        -d "[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"call\",\"params\":[\"$session\",\"$object\",\"$method\",$params]}]"
}

# ── Transport detection ──────────────────────────────────────────────
SSH_CMD="ssh -p $SSH_PORT -o StrictHostKeyChecking=no -o ConnectTimeout=3 root@$GATEWAY"
USE_SSH=false

if $SSH_CMD "echo ok" >/dev/null 2>&1; then
    USE_SSH=true
    ok "SSH reachable — using wireless deploy."
else
    warn "SSH not reachable — falling back to ADB."

# ── Steps 1-2: Enable ADB + connect ──────────────────────────────────
if adb devices 2>/dev/null | grep -qw device; then
    ok "ADB already connected, skipping web auth."
else
    info "Authenticating with router web interface..."

    ANON_SESSION="00000000000000000000000000000000"

    # Get salt (with safe JSON extraction)
    SALT_RESP=$(ubus_call "$ANON_SESSION" "zwrt_web" "web_login_info" '{}')
    SALT=$(echo "$SALT_RESP" | python3 -c '
import sys, json
try:
    print(json.load(sys.stdin)[0]["result"][1]["zte_web_sault"])
except Exception:
    pass
' 2>/dev/null)

    if [ -z "$SALT" ]; then
        fail "Failed to extract salt. Is the router reachable at $GATEWAY?"
    fi

    # Hash password
    PASS_HASH=$(upper "$(sha256 "$ROUTER_PASSWORD")")
    LOGIN_HASH=$(upper "$(sha256 "${PASS_HASH}${SALT}")")

    # Login (with safe JSON extraction)
    LOGIN_RESP=$(ubus_call "$ANON_SESSION" "zwrt_web" "web_login" "{\"password\":\"$LOGIN_HASH\"}")
    SESSION=$(echo "$LOGIN_RESP" | python3 -c '
import sys, json
try:
    print(json.load(sys.stdin)[0]["result"][1]["ubus_rpc_session"])
except Exception:
    pass
' 2>/dev/null)

    if [ -z "$SESSION" ] || [ "$SESSION" = "null" ]; then
        fail "Login failed. Check your router password."
    fi
    ok "Logged in to router (session: ${SESSION:0:8}...)."

    # Set USB mode to debug
    info "Enabling ADB (USB debug mode)..."
    ubus_call "$SESSION" "zwrt_bsp.usb" "set" '{"mode":"debug"}' >/dev/null
    ok "USB debug mode enabled."

    # Wait for ADB device
    info "Waiting for ADB device (plug USB cable if not connected)..."
    if ! timeout 30 adb wait-for-device 2>/dev/null; then
        adb wait-for-device &
        ADB_PID=$!
        for i in $(seq 1 30); do
            if ! kill -0 "$ADB_PID" 2>/dev/null; then break; fi
            sleep 1
        done
        if kill -0 "$ADB_PID" 2>/dev/null; then
            kill "$ADB_PID" 2>/dev/null
            fail "ADB device not found after 30s. Check USB connection."
        fi
    fi
    ok "ADB device connected."
fi
fi

# ── Helper: remote command / push ────────────────────────────────────
rcmd() {
    if [ "$USE_SSH" = true ]; then
        $SSH_CMD "$@"
    else
        adb shell "$@"
    fi
}

rpush() {
    local src="$1" dst="$2"
    if [ "$USE_SSH" = true ]; then
        cat "$src" | $SSH_CMD "cat > $dst && chmod +x $dst"
    else
        adb push "$src" "$dst" && adb shell "chmod +x $dst"
    fi
}

# ── Step 3: Build zte-agent ─────────────────────────────────────────
info "Building zte-agent (this may take a few minutes on first run)..."
cargo build --release --target "$TARGET" -p zte-agent
ok "Build complete."

# ── Step 4: Push binary ─────────────────────────────────────────────
info "Checking zte-agent binary..."
LOCAL_SHA=$(sha256_file "$BINARY")
REMOTE_SHA=$(rcmd "sha256sum $REMOTE_BIN 2>/dev/null" | awk '{print $1}')
if [ "$LOCAL_SHA" = "$REMOTE_SHA" ]; then
    ok "Binary unchanged, skipping push."
else
    info "Stopping running agent before push..."
    rcmd "killall zte-agent 2>/dev/null; sleep 1"
    info "Pushing zte-agent to device..."
    rpush "$BINARY" "$REMOTE_BIN"
    BINARY_CHANGED=true
    ok "Binary deployed to $REMOTE_BIN."
fi

# ── Step 5: Create startup script ───────────────────────────────────
# Escape single quotes for safe embedding in sh single-quoted string
SAFE_PASSWORD=$(printf '%s' "$AGENT_PASSWORD" | sed "s/'/'\\\\''/g")

if rcmd "grep -qF '${SAFE_PASSWORD}' $STARTUP_SCRIPT 2>/dev/null"; then
    ok "Startup script already up to date."
else
    info "Creating startup script..."
    cat > /tmp/start_zte_agent.sh <<BOOT
#!/bin/sh
export ZTE_AGENT_PASSWORD='${SAFE_PASSWORD}'
nohup /data/zte-agent >/tmp/zte-agent.log 2>&1 &
BOOT
    rpush /tmp/start_zte_agent.sh "$STARTUP_SCRIPT"
    rm /tmp/start_zte_agent.sh
    ok "Startup script created at $STARTUP_SCRIPT."
fi

# ── Step 6: Update rc.local for boot persistence ────────────────────
info "Configuring auto-start on boot..."
RC_LINE="sh $STARTUP_SCRIPT"
if rcmd "grep -qF '$RC_LINE' /etc/rc.local 2>/dev/null"; then
    ok "rc.local already configured."
else
    rcmd "grep -q '^exit 0' /etc/rc.local \
        && sed -i '/^exit 0/i $RC_LINE' /etc/rc.local \
        || echo '$RC_LINE' >> /etc/rc.local"
    ok "Added zte-agent to /etc/rc.local."
fi

# ── Step 7: Start agent ─────────────────────────────────────────────
info "Checking agent status..."
AGENT_RUNNING=false
curl -sf "http://$GATEWAY:$AGENT_PORT/api/auth/login" \
    -H 'Content-Type: application/json' \
    -d "{\"password\":\"$AGENT_PASSWORD\"}" >/dev/null 2>&1 && AGENT_RUNNING=true

if [ "$BINARY_CHANGED" = true ] || [ "$AGENT_RUNNING" = false ]; then
    info "Starting zte-agent..."
    rcmd "killall zte-agent 2>/dev/null; true"
    sleep 1
    rcmd "sh $STARTUP_SCRIPT"
    ok "Agent (re)started."
else
    ok "Agent already running with current binary, skipping restart."
fi

# ── Step 8: Verify ──────────────────────────────────────────────────
info "Verifying agent is running..."
sleep 2

if [ "$USE_SSH" = true ]; then
    VERIFY_URL="http://$GATEWAY:$AGENT_PORT/api/auth/login"
else
    adb forward tcp:19090 tcp:$AGENT_PORT
    VERIFY_URL="http://127.0.0.1:19090/api/auth/login"
fi

TOKEN=$(curl -sf "$VERIFY_URL" \
    -H 'Content-Type: application/json' \
    -d "{\"password\":\"$AGENT_PASSWORD\"}" \
    | python3 -c 'import sys,json; print(json.load(sys.stdin)["data"]["token"])')

if [ "$USE_SSH" != true ]; then
    adb forward --remove tcp:19090 2>/dev/null || true
fi

if [ -z "$TOKEN" ] || [ "$TOKEN" = "null" ]; then
    fail "Agent started but login verification failed."
fi
ok "Agent is running and authenticated."

# ── Step 9: Optional SSH setup ──────────────────────────────────────
if [ "$USE_SSH" = true ]; then
    ok "SSH already configured, skipping SSH setup."
else
echo ""
echo -e "${CYAN}Set up SSH for wireless deploys? (y/N)${NC}"
read -r SETUP_SSH

if [ "$SETUP_SSH" = "y" ] || [ "$SETUP_SSH" = "Y" ]; then
    SSH_KEY="$HOME/.ssh/id_ed25519"
    SSH_PUB="$SSH_KEY.pub"

    # Generate SSH key if needed
    if [ ! -f "$SSH_KEY" ]; then
        info "Generating SSH key..."
        ssh-keygen -t ed25519 -f "$SSH_KEY" -N ""
        ok "SSH key generated at $SSH_KEY."
    else
        ok "SSH key already exists at $SSH_KEY."
    fi

    # Install dropbear on device
    info "Setting up dropbear SSH server..."

    # Check if dropbear is already present
    if adb shell "test -x /usr/sbin/dropbear" 2>/dev/null; then
        ok "Dropbear already installed."
    else
        info "Downloading dropbear for aarch64..."
        DROPBEAR_URL="https://downloads.openwrt.org/releases/23.05.4/targets/armsr/armv8/packages/dropbear_2022.83-1_aarch64_generic.ipk"
        TMPDIR=$(mktemp -d)
        curl -sfL "$DROPBEAR_URL" -o "$TMPDIR/dropbear.ipk" || fail "Failed to download dropbear."
        adb push "$TMPDIR/dropbear.ipk" /tmp/dropbear.ipk
        adb shell "opkg install /tmp/dropbear.ipk 2>/dev/null || true"
        adb shell "rm -f /tmp/dropbear.ipk"
        rm -rf "$TMPDIR"
        ok "Dropbear installed."
    fi

    # Set up authorized_keys
    info "Configuring SSH keys..."
    adb shell "mkdir -p /etc/dropbear && chmod 700 /etc/dropbear"
    PUBKEY=$(cat "$SSH_PUB")
    if adb shell "grep -qF '$PUBKEY' /etc/dropbear/authorized_keys 2>/dev/null"; then
        ok "SSH key already authorized."
    else
        adb shell "echo '$PUBKEY' >> /etc/dropbear/authorized_keys"
        ok "SSH key added to authorized_keys."
    fi
    adb shell "chmod 600 /etc/dropbear/authorized_keys"

    # Create dropbear startup script
    DROPBEAR_STARTUP=/data/local/tmp/start_dropbear.sh
    if adb shell "test -x $DROPBEAR_STARTUP" 2>/dev/null; then
        ok "Dropbear startup script already exists."
    else
        adb shell "cat > $DROPBEAR_STARTUP" <<'DBBOOT'
#!/bin/sh
/usr/sbin/dropbear -p 2222 -R
DBBOOT
        adb shell "chmod +x $DROPBEAR_STARTUP"
        ok "Dropbear startup script created."
    fi

    # Add to rc.local if not already there
    DB_RC_LINE="sh $DROPBEAR_STARTUP"
    if adb shell "grep -qF '$DB_RC_LINE' /etc/rc.local 2>/dev/null"; then
        ok "Dropbear rc.local entry already configured."
    else
        adb shell "grep -q '^exit 0' /etc/rc.local \
            && sed -i '/^exit 0/i $DB_RC_LINE' /etc/rc.local \
            || echo '$DB_RC_LINE' >> /etc/rc.local"
        ok "Added dropbear to /etc/rc.local."
    fi

    # Start dropbear now
    if adb shell "pidof dropbear" >/dev/null 2>&1; then
        ok "Dropbear already running on port $SSH_PORT."
    else
        info "Starting dropbear..."
        adb shell "sh $DROPBEAR_STARTUP"
        ok "Dropbear started on port $SSH_PORT."
    fi

    # Verify SSH
    info "Verifying SSH connection..."
    if ssh -p "$SSH_PORT" -o StrictHostKeyChecking=no -o ConnectTimeout=5 "root@$GATEWAY" "echo ok" >/dev/null 2>&1; then
        ok "SSH connection verified."
    else
        warn "SSH connection could not be verified. You may need to reboot the router."
    fi

    echo ""
    ok "SSH is configured. You can now use ./deploy.sh for wireless deploys:"
    echo "    ssh -p $SSH_PORT root@$GATEWAY"
fi
fi

# ── Done ─────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}Setup complete!${NC}"
echo ""
echo "  Agent API:  http://$GATEWAY:$AGENT_PORT"
echo "  Password:   $AGENT_PASSWORD"
echo "  Deploy:     ./deploy.sh $AGENT_PASSWORD"
echo ""
echo "  Point the iOS/Android companion app at http://$GATEWAY:$AGENT_PORT"
