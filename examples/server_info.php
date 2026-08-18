<?php

/**
 * The info protocol: build, namespaces, statistics, and the cluster's nodes.
 *
 * Port of the Rust client's `server_info` example.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

$client = example_client();

section('the daemon');
// `ping()` is the one command that does not touch the database: it asks the daemon
// about itself. Version equality between the two halves is not negotiable, so this
// is what you check first when something is inexplicably not working.
$info = $client->ping();
out('daemon version', $info->version());
out('configured instances', $info->instances());

section('the cluster');
foreach ($client->nodes() as $node) {
    out('node', $node->name() . ' at ' . $node->address()
        . ' (server ' . $node->version() . ', ' . ($node->isActive() ? 'active' : 'inactive') . ')');
}

section('info commands');
// Several commands in one round trip. The answers come back keyed by command.
$answers = $client->info(null, ['build', 'edition', 'namespaces']);
out('build', $answers['build'] ?? '?');
out('edition', $answers['edition'] ?? '?');
out('namespaces', $answers['namespaces'] ?? '?');

section('statistics — a large semicolon-separated list');
$stats = $client->info(null, ['statistics'])['statistics'] ?? '';
$fields = explode(';', $stats);
out('fields returned', count($fields));
foreach (array_slice($fields, 0, 5) as $field) {
    out('  ' . $field);
}
out('…');

section('a namespace-scoped command');
// Any info command the server accepts works, including ones with arguments — which
// is how you read a namespace's configuration, such as whether it is
// strong-consistency (required for transactions).
$ns = example_namespace();
$nsInfo = $client->info(null, ["namespace/$ns"])["namespace/$ns"] ?? '';
foreach (explode(';', $nsInfo) as $field) {
    if (str_starts_with($field, 'strong-consistency=') || str_starts_with($field, 'objects=')) {
        out('  ' . $field);
    }
}
