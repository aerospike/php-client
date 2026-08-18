<?php

/**
 * Run every example, and fail if any of them does.
 *
 *     php -d extension=<lib> examples/run-all.php          # or: make examples
 *
 * This is the PHP counterpart of what the Rust client does by including its examples
 * in the integration test suite: an example that is never executed rots silently as
 * the API moves, and an example is the first thing a new user copies.
 *
 * An example that cannot run here — a transaction without a strong-consistency
 * namespace, a path expression against a server older than 8.1.1 — prints `SKIPPED`
 * and exits 0. That is a pass, not a failure, and the summary counts it separately so
 * a skip cannot hide behind one.
 *
 * Environment: as the examples themselves, plus
 *   AEROSPIKE_SC_NAMESPACE  namespace for `transaction.php` (default: unset, so it skips)
 *   PHP_EXTENSION           path to the extension, for the child processes
 */

declare(strict_types=1);

$dir = __DIR__;

if (!extension_loaded('aerospike-php')) {
    fwrite(STDERR, "the aerospike-php extension is not loaded; pass -d extension=<lib>\n");
    exit(1);
}

/**
 * The library the child processes should load.
 *
 * Each example runs in its own process — that is the point, since one is expected to
 * be run on its own — so each needs the extension named again. There is no reliable
 * way to ask PHP where the currently loaded one came from, so this takes it from
 * `PHP_EXTENSION` (which `make examples` sets) and otherwise looks where a release
 * build in this repository puts it.
 */
function extension_path(): string
{
    $given = getenv('PHP_EXTENSION');
    if (is_string($given) && $given !== '') {
        return $given;
    }
    foreach (['dylib', 'so'] as $suffix) {
        $candidate = __DIR__ . "/../ext/target/release/libaerospike_php.$suffix";
        if (is_file($candidate)) {
            return $candidate;
        }
    }
    fwrite(STDERR, "cannot find the extension. Set PHP_EXTENSION, or build it:\n"
        . "  make build-ext\n");
    exit(1);
}

$extension = extension_path();

// `transaction.php` needs a strong-consistency namespace, which is usually not the
// one everything else uses. Give it its own, when there is one.
$scNamespace = getenv('AEROSPIKE_SC_NAMESPACE') ?: null;

$examples = array_values(array_filter(
    array_map('basename', glob("$dir/*.php") ?: []),
    // `_bootstrap.php` is a library and this file is the runner.
    fn(string $name) => !str_starts_with($name, '_') && $name !== 'run-all.php',
));
sort($examples);

$passed = [];
$skipped = [];
$failed = [];

foreach ($examples as $example) {
    $env = '';
    if ($example === 'transaction.php' && $scNamespace !== null) {
        $env = 'AEROSPIKE_NAMESPACE=' . escapeshellarg($scNamespace) . ' ';
    }
    // The child inherits this process's extension, so the same library is used.
    $command = $env . escapeshellcmd(PHP_BINARY)
        . ' -d extension=' . escapeshellarg($extension)
        . ' ' . escapeshellarg("$dir/$example") . ' 2>&1';

    $output = [];
    $status = 0;
    exec($command, $output, $status);
    $text = implode("\n", $output);

    if ($status !== 0) {
        $failed[$example] = $text;
        printf("FAIL  %-28s exit %d\n", $example, $status);
        continue;
    }
    if (str_contains($text, 'SKIPPED:')) {
        $skipped[$example] = trim(substr($text, strpos($text, 'SKIPPED:') + 8));
        printf("skip  %-28s %s\n", $example, $skipped[$example]);
        continue;
    }
    $passed[] = $example;
    printf("ok    %-28s %d lines of output\n", $example, count($output));
}

echo "\n";
printf("%d passed, %d skipped, %d failed\n", count($passed), count($skipped), count($failed));

if ($failed !== []) {
    foreach ($failed as $example => $text) {
        echo "\n──── $example ────\n$text\n";
    }
    exit(1);
}
