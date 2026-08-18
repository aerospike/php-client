<?php

/**
 * Shared setup for the examples: connect, and a little output formatting.
 *
 * Each example is standalone — `php -d extension=… examples/crud.php` — and every
 * one is also run end to end by `examples/run-all.php`, which is what `make examples`
 * does. That mirrors the Rust client, where the examples are compiled and executed by
 * the integration test suite so they cannot rot as the API moves.
 *
 * Environment:
 *   AEROSPIKE_INSTANCE   daemon instance to attach to (default "default")
 *   AEROSPIKE_NAMESPACE  namespace for the example records (default "test")
 */

declare(strict_types=1);

/** The client every example uses. */
function example_client(): Aerospike\Client
{
    if (!extension_loaded('aerospike-php')) {
        fwrite(STDERR, "the aerospike-php extension is not loaded. Try:\n"
            . "  php -d extension=ext/target/release/libaerospike_php.dylib " . ($_SERVER['argv'][0] ?? '<example>') . "\n");
        exit(1);
    }
    return new Aerospike\Client(getenv('AEROSPIKE_INSTANCE') ?: 'default');
}

function example_namespace(): string
{
    return getenv('AEROSPIKE_NAMESPACE') ?: 'test';
}

/** A section heading, so the output of a long example stays readable. */
function section(string $title): void
{
    echo "\n--- $title ---\n";
}

/** One result line. */
function out(string $label, mixed $value = null): void
{
    if (func_num_args() === 1) {
        echo "  $label\n";
        return;
    }
    echo "  $label: ", is_string($value) ? $value : json_encode($value), "\n";
}

/**
 * An example that cannot run here says so and exits 0.
 *
 * Exit 0 matters: `run-all.php` treats a non-zero exit as a failure, and a server
 * without strong consistency or without 8.1.1 is not a failure of the example.
 */
function skip(string $why): never
{
    echo "  SKIPPED: $why\n";
    exit(0);
}

/** The server's build version, for the examples that need a minimum. */
function server_version(Aerospike\Client $client): string
{
    return $client->info(null, ['build'])['build'] ?? '0.0.0';
}

/** True when the server is at least `$required` (dotted numeric compare). */
function server_at_least(Aerospike\Client $client, string $required): bool
{
    $have = preg_replace('/[^0-9.].*$/', '', server_version($client));
    return version_compare($have, $required, '>=');
}

/**
 * True when `$ns` accepts a **finite** TTL.
 *
 * A namespace whose reaper is off (`nsup-period=0`) refuses any write that sets an
 * expiration — result code 22, `FailForbidden` — unless it was configured with
 * `allow-ttl-without-nsup`. Both are ordinary configurations, so an example that
 * demonstrates a TTL asks first rather than dying on a development cluster.
 */
function namespace_allows_ttl(Aerospike\Client $client, string $ns): bool
{
    $info = $client->info(null, ["namespace/$ns"])["namespace/$ns"] ?? '';
    return !preg_match('/(^|;)nsup-period=0(;|$)/', $info)
        || (bool) preg_match('/(^|;)allow-ttl-without-nsup=true(;|$)/', $info);
}

/** True when `$ns` is a strong-consistency namespace, which MRT requires. */
function namespace_is_sc(Aerospike\Client $client, string $ns): bool
{
    $info = $client->info(null, ["namespace/$ns"])["namespace/$ns"] ?? '';
    return str_contains($info, 'strong-consistency=true');
}
