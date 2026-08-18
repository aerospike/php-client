#!/usr/bin/env bash
#
# Run a command against a daemon started just for it, then stop the daemon.
#
#     tools/with-daemon.sh <instance> <command> [args...]
#
# Why this exists: the wakeup mode is a **compile-time** feature on both halves, so
# testing the other mode means the daemon serving the live suites has to be the
# other build too. Telling someone to restart it by hand is a step that gets skipped,
# and a mismatched pair does not report an error — the worker simply never learns its
# reply is ready. So the mode being tested brings its own daemon.
#
# The instance name is an argument rather than fixed, so this cannot collide with a
# development daemon already serving `default`.
#
# Environment:
#   AEROSPIKE_*       the cluster to point the throwaway daemon at; see
#                     tools/cluster-toml.sh for the full list
#   DAEMON            path to aerospike-php-daemon (default target/release/…)

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$here")"

instance="${1:?usage: with-daemon.sh <instance> <command> [args...]}"
shift
[ $# -gt 0 ] || { echo "with-daemon.sh: no command given" >&2; exit 2; }

daemon="${DAEMON:-$root/target/release/aerospike-php-daemon}"
[ -x "$daemon" ] || { echo "with-daemon.sh: no daemon at $daemon" >&2; exit 1; }

work="$(mktemp -d)"
config="$work/aerospike-daemon.toml"
log="$work/daemon.log"

{
    printf '[daemon]\ninstance = "%s"\n\n' "$instance"
    "$here/cluster-toml.sh" "$instance"
} > "$config"

pid=""
cleanup() {
    local status=$?
    if [ -n "$pid" ] && kill -0 "$pid" 2> /dev/null; then
        # SIGTERM, so the daemon drains and deregisters its shared-memory ports. A
        # SIGKILL would leave the service behind for the next run to trip over.
        kill "$pid" 2> /dev/null || true
        wait "$pid" 2> /dev/null || true
    fi
    [ $status -eq 0 ] || { echo "--- daemon log ---" >&2; cat "$log" >&2 || true; }
    rm -rf "$work"
}
trap cleanup EXIT

"$daemon" --config "$config" > "$log" 2>&1 &
pid=$!

# Wait for the line the daemon prints once it is actually serving, rather than
# sleeping a guessed interval. A daemon that dies on startup — a cluster that is not
# there, a service name already taken — is caught by the liveness check.
for _ in $(seq 1 100); do
    grep -q "serving '$instance'" "$log" && break
    kill -0 "$pid" 2> /dev/null || { echo "with-daemon.sh: the daemon exited during startup" >&2; exit 1; }
    sleep 0.1
done
grep -q "serving '$instance'" "$log" || { echo "with-daemon.sh: the daemon never began serving" >&2; exit 1; }

echo "  [with-daemon] instance '$instance', $(grep -o 'wakeup: [a-z-]*' "$log" | head -1)"

AEROSPIKE_INSTANCE="$instance" "$@"
