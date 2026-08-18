<?php

/**
 * Multi-record transactions: commit and abort.
 *
 * Port of the Rust client's `transaction` example. Needs Aerospike 8.0+ and a
 * **strong-consistency namespace** — this skips cleanly without one, which is the
 * common case on a development cluster.
 *
 * Set `AEROSPIKE_NAMESPACE` to an SC namespace to see it run.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{Bin, Key, WritePolicy};

$client = example_client();
$ns = example_namespace();

if (!server_at_least($client, '8.0')) {
    skip('multi-record transactions need server 8.0+, this is ' . server_version($client));
}
if (!namespace_is_sc($client, $ns)) {
    skip("namespace '$ns' is not strong-consistency; transactions require one");
}

$a = new Key($ns, 'txn_demo', 'account-a');
$b = new Key($ns, 'txn_demo', 'account-b');

/*
 * Deletes here must be *durable*, and that is not a preference.
 *
 * A strong-consistency namespace forbids a non-durable delete outright — the server
 * answers result code 22, `FailForbidden`. Since a transaction requires an SC
 * namespace, every delete in this example needs `durableDelete: true`. The cost is a
 * tombstone, which is what makes the delete durable in the first place.
 */
$durable = new WritePolicy(durableDelete: true);
$client->delete($durable, $a);
$client->delete($durable, $b);

section('seed two accounts outside any transaction');
$client->put(null, $a, [new Bin('balance', 100)]);
$client->put(null, $b, [new Bin('balance', 0)]);
out('A', $client->get(null, $a)->bin('balance'));
out('B', $client->get(null, $b)->bin('balance'));

section('commit — transfer 30 from A to B, atomically');
$txn = $client->beginTransaction();
out('transaction id', $txn->id());

// A command joins the transaction by carrying it on its policy.
$inTxn = new WritePolicy(txn: $txn);
$client->put($inTxn, $a, [new Bin('balance', 70)]);
$client->put($inTxn, $b, [new Bin('balance', 30)]);

out('commit status', $txn->commit()->name);
out('A after commit', $client->get(null, $a)->bin('balance'));
out('B after commit', $client->get(null, $b)->bin('balance'));

section('abort — a failed business rule rolls everything back');
$txn = $client->beginTransaction();
$client->put(new WritePolicy(txn: $txn), $a, [new Bin('balance', -1000)]);

// Pretend validation rejected the transfer. Abort rather than commit.
out('abort status', $txn->abort()->name);
out('A after abort (unchanged)', $client->get(null, $a)->bin('balance'));

section('an unfinished transaction rolls back by itself');
// The object aborts itself when it is destroyed, so a request that throws part-way
// through does not leave record locks behind. The daemon's idle sweep and the
// server's own transaction timeout are the second and third lines of defence.
$doomed = $client->beginTransaction();
$client->put(new WritePolicy(txn: $doomed), $b, [new Bin('balance', 999)]);
unset($doomed);
out('B after dropping the transaction without committing', $client->get(null, $b)->bin('balance'));

$client->delete($durable, $a);
$client->delete($durable, $b);
