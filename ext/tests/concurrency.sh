#!/usr/bin/env bash
#
# Run tests/concurrency.php against a daemon with a deliberately tiny
# `max-workers`, so the ceiling section actually exercises the boundary.
#
#     tests/concurrency.sh [max-workers]        # default 4
#
# Why a dedicated daemon: `max-workers` is iceoryx2's `max_clients`, fixed when
# the service is created. Proving the boundary against the default of 64 would
# mean holding 65 PHP processes attached at once; against 4 it costs five. The
# daemon runs under its own instance name so it cannot collide with a development
# daemon already serving `default`.
#
# Environment:
#   AEROSPIKE_HOSTS       seed for the throwaway daemon (default 127.0.0.1:3000),
#                         along with the rest of the AEROSPIKE_* cluster variables
#                         listed in tools/cluster-toml.sh
#   AEROSPIKE_NAMESPACE   passed through to the script (default test)
#   PHP_EXTENSION         path to the built extension (default target/release/*)
#   DAEMON                path to aerospike-php-daemon (default ../target/release/…)

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ext_dir="$(dirname "$here")"
# The workspace root is `aerospike-php/`, one level above `ext/`, and that is where
# the daemon builds to. (It used to be two levels up, when these crates were members
# of the Rust client's workspace.)
workspace="$(cd "$ext_dir/.." && pwd)"

ceiling="${1:-4}"
instance="ceiling$$"

# The extension: whichever of the three platform names is there.
extension="${PHP_EXTENSION:-}"
if [[ -z "$extension" ]]; then
  for candidate in libaerospike_php.dylib libaerospike_php.so aerospike_php.dll; do
    if [[ -f "$ext_dir/target/release/$candidate" ]]; then
      extension="$ext_dir/target/release/$candidate"
      break
    fi
  done
fi
if [[ ! -f "$extension" ]]; then
  echo "no built extension found; run 'cargo build --release' in $ext_dir" >&2
  exit 1
fi

daemon="${DAEMON:-$workspace/target/release/aerospike-php-daemon}"
if [[ ! -x "$daemon" ]]; then
  echo "no daemon at $daemon; run 'cargo build --release -p aerospike-php-daemon'" >&2
  exit 1
fi

work="$(mktemp -d)"
config="$work/daemon.toml"
log="$work/daemon.log"

# The cluster table comes from the shared emitter, so this daemon reaches the
# same server the rest of the suite does — including a secured or NAT'ed one.
{
  cat <<TOML
[daemon]
instance = "$instance"
max-workers = $ceiling
# info, not warn: the readiness line this script waits for is logged at info,
# and guessing a sleep instead would report a slow start as a test failure.
log_level = "info"

TOML
  "$workspace/tools/cluster-toml.sh" "$instance"
} > "$config"

cleanup() {
  local status=$?
  if [[ -n "${daemon_pid:-}" ]]; then
    kill "$daemon_pid" 2>/dev/null || true
    wait "$daemon_pid" 2>/dev/null || true
  fi
  if (( status != 0 )) && [[ -s "$log" ]]; then
    echo "--- daemon log ---" >&2
    cat "$log" >&2
  fi
  rm -rf "$work"
  exit $status
}
trap cleanup EXIT

echo "starting a daemon with max-workers = $ceiling on instance '$instance'"
"$daemon" --config "$config" > "$log" 2>&1 &
daemon_pid=$!

# Wait for it to say it is serving rather than sleeping a guessed interval: a
# daemon that failed to start must be reported as that, not as a test failure.
for _ in $(seq 1 100); do
  if grep -q "serving '$instance'" "$log" 2>/dev/null; then
    break
  fi
  if ! kill -0 "$daemon_pid" 2>/dev/null; then
    echo "the daemon exited before it began serving" >&2
    exit 1
  fi
  sleep 0.1
done
if ! grep -q "serving '$instance'" "$log" 2>/dev/null; then
  echo "the daemon did not begin serving within 10s" >&2
  exit 1
fi

AEROSPIKE_INSTANCE="$instance" \
AEROSPIKE_MAX_WORKERS="$ceiling" \
AEROSPIKE_NAMESPACE="${AEROSPIKE_NAMESPACE:-test}" \
CONCURRENCY_WORKERS="${CONCURRENCY_WORKERS:-$ceiling}" \
  php -d extension="$extension" "$here/concurrency.php"
