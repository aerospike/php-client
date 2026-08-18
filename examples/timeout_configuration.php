<?php

/**
 * Timeouts: socket, total, and where each one applies.
 *
 * Port of the Rust client's `timeout_configuration` example.
 *
 * Note that a PHP worker has a *third* deadline the Rust client has no equivalent
 * for: the daemon's own `default_timeout`, which applies when a policy names none.
 * `null` as a policy means "use the daemon's configured settings", so the timeouts
 * below are what you set when one command needs to differ from that default.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{AerospikeException, Bin, Key, ReadPolicy, Status};

$client = example_client();
$ns = example_namespace();
$key = new Key($ns, 'timeout_demo', 'record');

$client->put(null, $key, [new Bin('data', 'test value')]);
out('test record written');

/** Time a read under a given policy, and report what happened. */
function timed_read(Aerospike\Client $client, ?ReadPolicy $policy, Key $key, string $what): void
{
    $started = microtime(true);
    try {
        $record = $client->get($policy, $key);
        printf("  %s: read %d bin(s) in %.2f ms\n",
            $what, count($record->bins()), (microtime(true) - $started) * 1000);
    } catch (AerospikeException $e) {
        printf("  %s: %s after %.2f ms — %s\n", $what,
            $e->getStatus() === Status::Timeout ? 'TIMED OUT' : 'failed',
            (microtime(true) - $started) * 1000, $e->getMessage());
    }
}

section("the daemon's own defaults");
timed_read($client, null, $key, 'null policy');

section('no timeout at all');
// Zero means "no limit". Fine for a one-off script; a bad idea in a web request,
// where a hung command holds a worker that a user is waiting on.
timed_read($client, new ReadPolicy(totalTimeoutMs: 0, socketTimeoutMs: 0), $key, 'both zero');

section('socket and total together');
// socketTimeout bounds one idle socket read; totalTimeout bounds the whole command
// including retries. The total is the one that bounds what a caller waits for.
timed_read($client, new ReadPolicy(totalTimeoutMs: 5000, socketTimeoutMs: 2000), $key,
    'socket 2s, total 5s');

section('a timeout at the edge of what the round trip needs');
// A 1ms total against a *local* cluster usually succeeds — the round trip above is
// well under a millisecond — and against a remote one it will not. That is the point:
// a timeout is only meaningful relative to the latency you actually have, so measure
// before choosing one. Whichever way it goes here, note that a timeout is reported as
// `Status::Timeout` and never as a missing record.
timed_read($client, new ReadPolicy(totalTimeoutMs: 1, socketTimeoutMs: 1), $key, 'total 1ms');

section('retries');
// maxRetries is counted *within* the total timeout: a command does not get
// maxRetries × totalTimeout, it gets totalTimeout overall. So raising retries
// without raising the total buys nothing.
timed_read($client, new ReadPolicy(totalTimeoutMs: 5000, maxRetries: 5), $key,
    '5 retries inside a 5s total');

$client->delete(null, $key);
