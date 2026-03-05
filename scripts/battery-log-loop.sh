#!/bin/bash
INTERVAL=${1:-300}
while true; do
    cargo run -p zte-cli -- settings device battery-log --ssh
    echo "--- Next log in ${INTERVAL}s ($(date)) ---"
    sleep "$INTERVAL"
done
