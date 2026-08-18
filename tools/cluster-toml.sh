#!/usr/bin/env bash
#
# Print the `[cluster.<name>]` table for the cluster the tests run against.
#
#     tools/cluster-toml.sh <instance-name>
#
# Every script here that starts a throwaway daemon needs the same table, and the
# cluster under test is not the same one twice: a local container, a NAT'ed VM that
# only answers on its `services-alternate` addresses, a secured cluster in CI. This
# is the one place that turns the environment into configuration, so a new knob is
# added once rather than in each caller — and so a caller cannot quietly disagree
# with `daemon/tests/ipc_roundtrip.rs`, which reads the same variables.
#
# Environment:
#   AEROSPIKE_HOSTS                    seed host(s) (default 127.0.0.1:3000)
#   AEROSPIKE_USE_SERVICES_ALTERNATE   tend the alternate node list (true/false)
#   AEROSPIKE_USER, AEROSPIKE_PASSWORD, AEROSPIKE_AUTH_MODE   for a secured cluster
#
# An unset *or empty* variable is omitted: `make` exports these unconditionally, so
# empty is how "not given" arrives.

set -euo pipefail

name="${1:?usage: cluster-toml.sh <instance-name>}"

printf '[cluster.%s]\n' "$name"
printf 'host = "%s"\n' "${AEROSPIKE_HOSTS:-127.0.0.1:3000}"

if [ -n "${AEROSPIKE_USE_SERVICES_ALTERNATE:-}" ]; then
    case "$(printf '%s' "$AEROSPIKE_USE_SERVICES_ALTERNATE" | tr '[:upper:]' '[:lower:]')" in
        1 | true | yes | on) printf 'use-services-alternate = true\n' ;;
        *) printf 'use-services-alternate = false\n' ;;
    esac
fi

[ -n "${AEROSPIKE_USER:-}" ]      && printf 'user = "%s"\n' "$AEROSPIKE_USER"
[ -n "${AEROSPIKE_PASSWORD:-}" ]  && printf 'password = "%s"\n' "$AEROSPIKE_PASSWORD"
[ -n "${AEROSPIKE_AUTH_MODE:-}" ] && printf 'auth = "%s"\n' "$AEROSPIKE_AUTH_MODE"

exit 0
