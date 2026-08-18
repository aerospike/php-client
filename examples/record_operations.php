<?php

/**
 * Single-record operations and write-policy behaviour.
 *
 * Port of the Rust client's `record_operations` example, which itself ports the
 * Java client's Add, Append, Prepend, Touch, Expire, Generation, Replace,
 * StoreKey and DeleteBin examples.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{AerospikeException, Bin, Bins, Expiration, GenerationPolicy, Key, Op,
    RecordExistsAction, Status, WritePolicy};

/*
 * Two ways to test a failure, and which one to use is not arbitrary.
 *
 * `getStatus()` returns an `Aerospike\Status` — a real enum, so it can be `match`ed
 * exhaustively. A handful of outcomes are promoted to their own case because callers
 * branch on them: `Status::RecordNotFound` is one. For those, matching the status is
 * the whole story and `getResultCode()` is `null` — there is no *extra* server code
 * to report beyond the classification itself.
 *
 * Everything else the server rejects arrives as `Status::Server`, and the specific
 * reason is `getResultCode()`: the number Aerospike's own documentation lists. A
 * generation conflict is one of those, hence the constant below. (`Aerospike\ResultCode`
 * has the full set as constants, but it lives in the optional 1.x compatibility layer
 * and an example should not need it.)
 */
const GENERATION_ERROR = 3;

$client = example_client();
$ns = example_namespace();
$key = new Key($ns, 'record_ops', 'demo');
$client->delete(null, $key);

section('add — integer arithmetic on a bin');
$client->put(null, $key, [new Bin('count', 10)]);
$client->add(null, $key, [new Bin('count', 5)]);
out('10 + 5', $client->get(null, $key, Bins::some(['count']))->bin('count'));

section('append and prepend — string concatenation');
$client->put(null, $key, [new Bin('greet', 'World')]);
$client->prepend(null, $key, [new Bin('greet', 'Hello, ')]);
$client->append(null, $key, [new Bin('greet', '!')]);
out('greet', $client->get(null, $key, Bins::some(['greet']))->bin('greet'));

section('expire — write with a TTL and read it back');
// A namespace with its reaper off refuses a finite TTL outright, so the example
// says so and carries on rather than failing on a development cluster.
$ttlAllowed = namespace_allows_ttl($client, $ns);
$ttlPolicy = new WritePolicy(expiration: Expiration::seconds(120));
if ($ttlAllowed) {
    $client->put($ttlPolicy, $key, [new Bin('ttl-bin', 1)]);
    out('ttl seconds', $client->getHeader(null, $key)->ttl());
} else {
    out("namespace '$ns' does not allow a ttl (nsup-period=0), so this part is skipped");
}

section('touch — reset the TTL, bump the generation, change no data');
$client->touch($ttlAllowed ? $ttlPolicy : null, $key);
out('generation', $client->getHeader(null, $key)->generation());

section('generation — optimistic concurrency (compare and set)');
$generation = $client->getHeader(null, $key)->generation();
$cas = new WritePolicy(
    generationPolicy: GenerationPolicy::ExpectGenEqual,
    generation: $generation,
);
$client->put($cas, $key, [new Bin('cas-bin', 1)]);
out('CAS write with the matching generation succeeded');

// The same policy again now carries a stale generation, so the server refuses it.
try {
    $client->put($cas, $key, [new Bin('cas-bin', 2)]);
    out('UNEXPECTED: the stale CAS write was accepted');
} catch (AerospikeException $e) {
    // A generation conflict is a plain server rejection, so match the result code.
    out('stale CAS write rejected', $e->getStatus() === Status::Server
        && $e->getResultCode() === GENERATION_ERROR
            ? 'Status::Server with result code 3, a generation error'
            : 'unexpectedly: ' . $e->getMessage());
}

section('replace — RecordExistsAction replaces the whole record');
$replace = new WritePolicy(recordExistsAction: RecordExistsAction::Replace);
$client->put($replace, $key, [new Bin('only-bin', 'left')]);
$record = $client->get(null, $key);
out('bins now', array_keys($record->bins()));

$missing = new Key($ns, 'record_ops', 'missing');
$client->delete(null, $missing);
try {
    $client->put(new WritePolicy(recordExistsAction: RecordExistsAction::ReplaceOnly),
        $missing, [new Bin('b', 1)]);
    out('UNEXPECTED: replace-only created a missing record');
} catch (AerospikeException $e) {
    // "No such record" is promoted to its own status, so match that instead — and
    // note that `getResultCode()` is null here, for the reason given at the top.
    out('replace-only on a missing record rejected', $e->getStatus() === Status::RecordNotFound
        ? 'Status::RecordNotFound (getResultCode() is null — the status says it all)'
        : 'unexpectedly: ' . $e->getMessage());
}

section('store key — ask the server to keep the user key');
$client->put(new WritePolicy(sendKey: true), $key, [new Bin('k-bin', 1)]);
out('written with sendKey = true; a scan of this set now returns the user key');

section('delete a bin — by writing null to it');
$client->put(null, $key, [new Bin('doomed', 42)]);
$client->put(null, $key, [new Bin('doomed', null)]);
out("'doomed' still present", $client->get(null, $key)->has('doomed'));

section('operate — put, add and read in one atomic call');
$result = $client->operate(null, $key, [
    Op::put(new Bin('count', 1)),
    Op::add(new Bin('count', 41)),
    Op::getBin('count'),
]);
out('1 + 41', $result->bin('count'));

$client->delete(null, $key);
