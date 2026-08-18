<?php
/**
 * End-to-end smoke test for the Aerospike PHP extension.
 *
 * Plain PHP, no framework. Exits 0 when everything passed and 1 otherwise.
 *
 *   php -d extension=$(pwd)/target/release/libaerospike_php.dylib tests/smoke.php
 *
 * Requires `aerospike-php-daemon` to be running, configured for the instance
 * under test, and built from **the same version** as this extension — the two
 * are matched by version and cannot talk to each other otherwise. Environment
 * overrides:
 *
 *   AEROSPIKE_INSTANCE   daemon instance name          (default: default)
 *   AEROSPIKE_NAMESPACE  namespace to write to         (default: test)
 *   AEROSPIKE_SET        set to write to               (default: smoke)
 */

declare(strict_types=1);

use Aerospike\AbortStatus;
use Aerospike\AdminPolicy;
use Aerospike\Bin;
use Aerospike\CommitStatus;
use Aerospike\Privilege;
use Aerospike\PrivilegeCode;
use Aerospike\Transaction;
use Aerospike\TxnRollPolicy;
use Aerospike\TxnState;
use Aerospike\TxnVerifyPolicy;
use Aerospike\CollectionIndex;
use Aerospike\IndexType;
use Aerospike\Task;
use Aerospike\TaskStatus;
use Aerospike\Filter;
use Aerospike\PartitionFilter;
use Aerospike\QueryPolicy;
use Aerospike\RecordSet;
use Aerospike\Statement;
use Aerospike\BatchDelete;
use Aerospike\BatchRead;
use Aerospike\BatchWrite;
use Aerospike\BinPolicy;
use Aerospike\BinWriteMode;
use Aerospike\Bins;
use Aerospike\BitOp;
use Aerospike\BitOverflow;
use Aerospike\BitResize;
use Aerospike\Blob;
use Aerospike\CommitLevel;
use Aerospike\ExpOp;
use Aerospike\Exp;
use Aerospike\ExpBit;
use Aerospike\ExpHll;
use Aerospike\ExpList;
use Aerospike\ExpMap;
use Aerospike\ExpStr;
use Aerospike\ExpType;
use Aerospike\RegexFlag;
use Aerospike\ExpPath;
use Aerospike\LoopVarPart;
use Aerospike\SelectFlag;
use Aerospike\ModifyFlag;
use Aerospike\Expiration;
use Aerospike\Expression;
use Aerospike\GenerationPolicy;
use Aerospike\GeoJson;
use Aerospike\Ctx;
use Aerospike\HllOp;
use Aerospike\Key;
use Aerospike\ListOp;
use Aerospike\ListOrder;
use Aerospike\ListPolicy;
use Aerospike\ListReturn;
use Aerospike\MapOp;
use Aerospike\MapOrder;
use Aerospike\MapPolicy;
use Aerospike\MapReturn;
use Aerospike\MapWriteMode;
use Aerospike\Op;
use Aerospike\OrderedMap;
use Aerospike\ReadModeAP;
use Aerospike\ReadModeSC;
use Aerospike\ReadPolicy;
use Aerospike\Record;
use Aerospike\RecordExistsAction;
use Aerospike\Replica;
use Aerospike\SortedMap;
use Aerospike\Status;
use Aerospike\WritePolicy;

$instance  = getenv('AEROSPIKE_INSTANCE') ?: 'default';
$namespace = getenv('AEROSPIKE_NAMESPACE') ?: 'test';
$set       = getenv('AEROSPIKE_SET') ?: 'smoke';

$passed  = 0;
$failed  = 0;
$skipped = 0;

function ok(string $what): void
{
    global $passed;
    $passed++;
    echo "  ok    $what\n";
}

function fail(string $what, string $why): void
{
    global $failed;
    $failed++;
    echo "  FAIL  $what\n        $why\n";
}

/**
 * Something this build cannot exercise, as opposed to something that failed.
 *
 * Mostly the server: a filter needs 8.1.3, a durable delete needs Enterprise.
 * Reporting that as a failure would be a lie in one direction, and passing
 * silently would be a lie in the other.
 */
function skip(string $what, string $why): void
{
    global $skipped;
    $skipped++;
    echo "  skip  $what\n        $why\n";
}

/** Whether a failure means "the daemon does not serve that verb yet". */
function unsupported(Aerospike\AerospikeException $e): bool
{
    return $e->getStatus() === Status::InvalidRequest
        && str_contains($e->getMessage(), 'unknown opcode');
}

/**
 * Whether every node runs at least $version.
 *
 * The server gates features by release — path expressions need 8.1.1, string
 * expressions 8.1.3 — so which of them this file may assert is the cluster's
 * answer, not a constant. Only the first three components are compared: the
 * fourth is a build number, which no feature gate is stated in.
 */
/**
 * Read until the answer settles, then return it — or return the last answer
 * anyway once the wait runs out, so the assertion that follows reports the real
 * mismatch rather than a timeout.
 *
 * Security changes are the one place the server answers before what it just did
 * is visible: a role read straight back after `createRole` can be missing
 * (InvalidRole) or carry no quota yet, and how long that takes belongs to the
 * cluster — one node or twenty. Retrying the *read* is what the server's own
 * tools do; asserting on the first answer pins the test to timing.
 */
function eventually(callable $read, callable $settled, float $seconds = 10.0)
{
    $deadline = microtime(true) + $seconds;
    while (true) {
        $expired = microtime(true) >= $deadline;
        try {
            $answer = $read();
            if ($settled($answer) || $expired) {
                return $answer;
            }
        } catch (Aerospike\AerospikeException $e) {
            // The change is not visible yet — unless it never will be.
            if ($expired) {
                throw $e;
            }
        }
        usleep(100_000);
    }
}

function server_at_least(Aerospike\Client $client, string $version): bool
{
    foreach ($client->nodes() as $node) {
        $running = implode('.', array_slice(explode('.', $node->version()), 0, 3));
        if (version_compare($running, $version, '<')) {
            return false;
        }
    }
    return true;
}

/** How a failure reads in a test's explanation. */
function describe(Aerospike\AerospikeException $e): string
{
    return $e->getStatus()->value . ': ' . $e->getMessage();
}

/** Assert that $actual equals $expected, structurally and by type. */
function is_same(string $what, $expected, $actual): void
{
    if ($expected === $actual) {
        ok($what);
        return;
    }
    fail($what, sprintf(
        'expected %s, got %s',
        var_export($expected, true),
        var_export($actual, true)
    ));
}

/**
 * Sort the keys of every map, recursively, leaving lists alone.
 *
 * Aerospike stores an unordered map key-ordered on the server, so a map comes
 * back in the server's key order rather than in PHP insertion order. That is
 * the server's behaviour faithfully reported, not a conversion bug — the wire
 * contract carries map entries as an ordered vector precisely so that
 * server-side ordering survives. List order, by contrast, is data and must be
 * preserved exactly, so lists are deliberately not touched here.
 */
function canonical($value)
{
    if (!is_array($value)) {
        return $value;
    }
    $out = [];
    foreach ($value as $k => $item) {
        $out[$k] = canonical($item);
    }
    if (!array_is_list($out)) {
        ksort($out);
    }
    return $out;
}

/** Compare a stored value with what came back, modulo map key order. */
function is_same_value(string $what, $expected, $actual): void
{
    if (canonical($expected) === canonical($actual)) {
        ok($what);
        return;
    }
    fail($what, sprintf(
        'expected %s, got %s',
        var_export($expected, true),
        var_export($actual, true)
    ));
}

function is_true(string $what, bool $condition, string $why = ''): void
{
    $condition ? ok($what) : fail($what, $why !== '' ? $why : 'condition was false');
}

/**
 * Assert that PHP itself refuses the call, before the extension sees it.
 *
 * This is what the object API buys: a wrong argument is a `TypeError` from the
 * engine, at the call site, rather than a message from the daemon a round trip
 * later. A test that only checked "it failed somehow" would pass even if the
 * types had been given up on, so the *class* of the failure is the assertion.
 */
function is_type_error(string $what, callable $call): void
{
    try {
        $call();
        fail($what, 'PHP accepted it');
    } catch (TypeError | ValueError | ArgumentCountError $e) {
        ok($what);
    } catch (Throwable $e) {
        fail($what, 'expected a TypeError, got ' . get_class($e) . ': ' . $e->getMessage());
    }
}

/**
 * Assert that the call is rejected as a programming error of some kind.
 *
 * Weaker than `is_type_error` on purpose, for the two cases where the engine
 * or ext-php-rs picks the class and it is not a `TypeError`: an unknown named
 * argument is a plain `Error`, and instantiating a class with no constructor is
 * an `Exception`. What matters is that neither is an `AerospikeException` — a
 * mistake in the calling code must not arrive dressed as a database failure.
 */
function is_rejected(string $what, string $expected, callable $call): void
{
    try {
        $call();
        fail($what, 'PHP accepted it');
    } catch (Aerospike\AerospikeException $e) {
        fail($what, 'expected a plain PHP error, got an AerospikeException: ' . $e->getMessage());
    } catch (Throwable $e) {
        is_true(
            $what,
            str_contains($e->getMessage(), $expected),
            'expected a message mentioning "' . $expected . '", got ' . $e->getMessage()
        );
    }
}

/** Assert that the extension refuses something PHP's types cannot catch. */
function is_client_error(string $what, string $expected, callable $call): void
{
    try {
        $call();
        fail($what, 'it was accepted');
    } catch (Aerospike\AerospikeException $e) {
        // CLIENT, not SERVER: nothing was sent, which is the point.
        is_true(
            $what,
            $e->getStatus() === Status::Client && str_contains($e->getMessage(), $expected),
            'expected a CLIENT failure mentioning "' . $expected . '", got ' . describe($e)
        );
    }
}

function section(string $name): void
{
    echo "\n$name\n";
}

/** A key nobody else in this run will use. */
function fresh_key(string $what): Key
{
    global $namespace, $set;
    return new Key(
        $namespace,
        $set,
        "smoke-$what-" . getmypid() . '-' . bin2hex(random_bytes(4))
    );
}

// ===== The extension must be loaded at all ==================================

section('extension');

if (!extension_loaded('aerospike-php')) {
    fwrite(STDERR, "aerospike-php is not loaded. Run with:\n"
        . "  php -d extension=\$(pwd)/target/release/libaerospike_php.dylib tests/smoke.php\n");
    exit(1);
}
ok('extension is loaded');

foreach ([
    'Client', 'Key', 'Bin', 'Bins', 'Record', 'DaemonInfo',
    'ReadPolicy', 'WritePolicy', 'Expiration',
    'Operation', 'Op', 'ListOp', 'MapOp', 'BitOp', 'HllOp', 'ExpOp',
    'ListPolicy', 'MapPolicy', 'BinPolicy', 'Ctx', 'Expression',
    'BatchRow', 'BatchRead', 'BatchWrite', 'BatchDelete', 'BatchUdf', 'BatchResult',
    'OrderedMap', 'SortedMap',
    'AerospikeException', 'Blob', 'GeoJson', 'Hll', 'Infinity', 'Wildcard',
] as $class) {
    is_true("Aerospike\\$class is a class", class_exists("Aerospike\\$class"));
}

// Real PHP enums, not class constants: that is what makes them matchable and
// type-checked at the call boundary.
foreach ([
    'Replica' => 5, 'ReadModeAP' => 2, 'ReadModeSC' => 4,
    'RecordExistsAction' => 5, 'GenerationPolicy' => 3, 'CommitLevel' => 2,
    'Status' => 12, 'ListOrder' => 2, 'ListReturn' => 8, 'MapOrder' => 3,
    'MapReturn' => 12, 'MapWriteMode' => 3, 'BinWriteMode' => 3, 'BitResize' => 4,
    'BitOverflow' => 3, 'CollectionIndex' => 4, 'TaskStatus' => 3, 'IndexType' => 4,
    'UdfLanguage' => 1, 'TxnState' => 4, 'CommitStatus' => 4, 'AbortStatus' => 4,
] as $enum => $cases) {
    $name = "Aerospike\\$enum";
    is_true("Aerospike\\$enum is an enum with $cases cases", enum_exists($name)
        && count((new ReflectionEnum($name))->getCases()) === $cases);
}
is_same('enum cases are string-backed', 'MASTER_PROLES', Replica::MasterProles->value);
is_same('an enum can be built from its backing value', Replica::Master, Replica::from('MASTER'));

// A method that *returns* an enum case has to hand back a `Case<T>`, not a bare
// enum: a case object is an immortal singleton, and returning one without taking a
// reference lets PHP decrement it until it is freed and its memory reused by an
// unrelated case. The symptom is remote from the cause — a class-constant read in
// some other file starts yielding the wrong enum — so this hammers the returning
// methods and then checks the singletons are still themselves.
$before = [Replica::Master, ReadModeAP::One, ListOrder::Ordered, MapReturn::KeyValue];
for ($i = 0; $i < 500; $i++) {
    ReadModeAP::one();
    ListOrder::Ordered();
    MapReturn::KeyValue();
    (new WritePolicy(replica: Replica::Master))->replica();
}
gc_collect_cycles();
is_same(
    'returning an enum case 2000 times does not corrupt the case singletons',
    $before,
    [Replica::Master, ReadModeAP::One, ListOrder::Ordered, MapReturn::KeyValue]
);

is_true(
    'Aerospike\\AerospikeException extends Exception',
    is_subclass_of('Aerospike\\AerospikeException', 'Exception')
);

// A read policy must have no way to express a write-only setting. This is the
// structural half of the read/write split: the daemon still rejects such a
// field, but PHP should never be able to build one.
foreach (['recordExistsAction', 'generationPolicy', 'generation', 'expiration',
          'commitLevel', 'durableDelete', 'respondPerEachOp', 'sendKey'] as $writeOnly) {
    is_true(
        "ReadPolicy cannot express $writeOnly",
        !method_exists(ReadPolicy::class, $writeOnly)
            && method_exists(WritePolicy::class, $writeOnly)
    );
}

// Objects the daemon produces must not be constructible: a Record nobody read
// would report a generation and a TTL that mean nothing.
foreach (['Record', 'DaemonInfo'] as $produced) {
    is_rejected(
        "Aerospike\\$produced cannot be constructed from PHP",
        'cannot instantiate',
        function () use ($produced) {
            $class = "Aerospike\\$produced";
            new $class();
        }
    );
}

// ===== Connect, PING, and the version handshake =============================

section("ping (instance \"$instance\")");

$client = new Aerospike\Client($instance);
is_same('client reports its instance', $instance, $client->instance());

try {
    $info = $client->ping();
} catch (Aerospike\AerospikeException $e) {
    fwrite(STDERR, "\nCould not reach the daemon:\n  " . $e->getMessage() . "\n\n"
        . 'status = ' . $e->getStatus()->value . "\n"
        . "Start aerospike-php-daemon (same version as this extension, configured with a\n"
        . "[cluster.$instance] section) and retry.\n");
    exit(1);
}

is_true('ping returns a DaemonInfo', $info instanceof Aerospike\DaemonInfo);
is_true('the daemon reports a version', $info->version() !== '');
// The lock-step rule, end to end: the daemon that answered must be this exact
// version. It could not have answered otherwise — the service name they meet on
// carries the version — so this also pins that the two are wired to one source.
is_same(
    'the daemon is exactly this extension version',
    phpversion('aerospike-php'),
    $info->version()
);
is_true('the daemon serves the instance under test', $info->serves($instance));
is_true('instances() lists it too', in_array($instance, $info->instances(), true));

echo '        daemon ' . $info->version() . ', instances: '
    . implode(', ', $info->instances()) . "\n";

/*
 * Whether $namespace accepts a **finite** TTL.
 *
 * A namespace whose reaper is off (`nsup-period=0`) refuses any write that sets
 * an expiration -- result code 22, FailForbidden -- unless it was configured with
 * `allow-ttl-without-nsup`. Both are legitimate configurations, so which of the
 * TTL assertions below can run is the cluster's answer, not a constant. A probe
 * that cannot be read means "assume yes": a suite that skipped on an unreadable
 * answer would turn a real TTL regression into a silent pass.
 */
$namespaceConfig = $client->info(null, ["namespace/$namespace"])["namespace/$namespace"] ?? '';
$finiteTtls = !preg_match('/(^|;)nsup-period=0(;|$)/', $namespaceConfig)
    || (bool) preg_match('/(^|;)allow-ttl-without-nsup=true(;|$)/', $namespaceConfig);
$noFiniteTtl = "namespace '$namespace' refuses a finite ttl: its reaper is off "
    . '(nsup-period=0) and allow-ttl-without-nsup is not set';

// ===== put + get round trip =================================================

section('put / get round trip');

$key = fresh_key('main');

$values = [
    'name'    => 'Alice',
    'age'     => 30,
    'height'  => 1.75,
    'active'  => true,
    'nothing' => null,
    // Sequential 0..n-1 keys, so this must come back as a packed array.
    'scores'  => [10, 20, 30],
    // String keys, so this must come back as an associative array.
    'prefs'   => ['theme' => 'dark', 'lang' => 'en'],
    // Nesting must survive in both directions.
    'nested'  => ['list' => [1, ['deep' => 'value']], 'n' => 7],
    // Non-sequential integer keys are a map, not a list.
    'sparse'  => [5 => 'five', 9 => 'nine'],
];

$bins = [];
foreach ($values as $name => $value) {
    $bins[] = new Bin($name, $value);
}

is_same('a Bin reports its name', 'name', $bins[0]->name());
is_same('a Bin reports its value', 'Alice', $bins[0]->value());

try {
    $client->put(null, $key, $bins);
    ok('put succeeded');
} catch (Aerospike\AerospikeException $e) {
    fail('put succeeded', describe($e));
    echo "\n$passed passed, $failed failed\n";
    exit(1);
}

try {
    $record = $client->get(null, $key);
} catch (Aerospike\AerospikeException $e) {
    fail('get succeeded', describe($e));
    echo "\n$passed passed, $failed failed\n";
    exit(1);
}

is_true('get returns a Record', $record instanceof Record);

$got = $record->bins();

// A null bin is not stored at all, so it must not come back.
$expected = $values;
unset($expected['nothing']);

foreach ($expected as $bin => $want) {
    is_same_value("bin \"$bin\" round trips", $want, $got[$bin] ?? '<<missing>>');
}

// Record::bin() and Record::bins() must agree, or one of them is lying.
is_same('Record::bin() reads one bin', 'Alice', $record->bin('name'));
is_same('Record::has() sees a bin that is there', true, $record->has('age'));
is_same('Record::has() is false for an absent bin', false, $record->has('nothing'));
is_same('Record::bin() is null for an absent bin', null, $record->bin('nothing'));
is_same('Record::count() counts the bins', count($expected), $record->count());
is_same_value('Record::binNames() names them', array_keys($expected), $record->binNames());

// List order is data, so unlike map key order it must survive exactly.
is_same('list order is preserved exactly', [10, 20, 30], $got['scores'] ?? null);
is_same(
    'a list stays a packed array',
    true,
    array_is_list($got['scores'] ?? null) && array_is_list($got['nested']['list'] ?? null)
);
// A map comes back in the server's key order, which for an unordered
// Aerospike map is sorted by key rather than PHP's insertion order.
is_same(
    'a map comes back in server key order',
    ['lang' => 'en', 'theme' => 'dark'],
    $got['prefs'] ?? null
);
is_true(
    'a null bin was not stored',
    !array_key_exists('nothing', $got),
    'bin "nothing" came back as ' . var_export($got['nothing'] ?? null, true)
);

is_true(
    'generation is a positive int',
    $record->generation() >= 1,
    var_export($record->generation(), true)
);
is_true(
    'ttl is an int or null',
    $record->ttl() === null || is_int($record->ttl()),
    var_export($record->ttl(), true)
);

echo '        generation ' . $record->generation()
    . ', ttl ' . ($record->ttl() === null ? 'never expires' : $record->ttl() . 's') . "\n";

// ===== Which bins come back =================================================

section('bin selection');

try {
    $partial = $client->get(null, $key, Bins::some(['name', 'age']));
    is_same_value(
        'only the requested bins come back',
        ['name' => 'Alice', 'age' => 30],
        $partial->bins()
    );

    $metaOnly = $client->get(null, $key, Bins::none());
    is_same('Bins::none() reads metadata only', [], $metaOnly->bins());
    is_true(
        'a metadata-only read still reports a generation',
        $metaOnly->generation() >= 1
    );

    $all = $client->get(null, $key, Bins::all());
    is_same('Bins::all() is the same as no selector', $record->count(), $all->count());
} catch (Aerospike\AerospikeException $e) {
    fail('bin selection', describe($e));
}

// The trap the class removes: an empty selection is not "no bins", it is a list
// that was computed and came out empty, and answering with no bins would hide
// that.
is_client_error('Bins::some([]) is refused', 'Bins::none()', fn() => Bins::some([]));
is_same('Bins::all() knows what it is', true, Bins::all()->isAll());
is_same('Bins::none() knows what it is', true, Bins::none()->isNone());
is_same_value('Bins::some() reports its names', ['a', 'b'], Bins::some(['a', 'b'])->names());
is_same('Bins::all() names no bins', null, Bins::all()->names());

// ===== A missing record is null, not an exception ===========================

section('missing records');

try {
    $missing = $client->get(null, fresh_key('absent'));
    is_same('a missing record reads as null', null, $missing);
} catch (Aerospike\AerospikeException $e) {
    fail('a missing record reads as null', 'threw instead: ' . describe($e));
}

// ===== Keys =================================================================

section('keys');

$intKey = new Key($namespace, $set, random_int(1, PHP_INT_MAX));
try {
    $client->put(null, $intKey, [new Bin('kind', 'int-key')]);
    is_same(
        'an integer key round trips',
        'int-key',
        $client->get(null, $intKey)?->bin('kind')
    );
} catch (Aerospike\AerospikeException $e) {
    fail('an integer key round trips', describe($e));
}

// A key is a value object: built once, readable, and reusable across commands.
is_same('Key reports its namespace', $namespace, $intKey->namespace());
is_same('Key reports its set', $set, $intKey->setName());
is_true('Key reports its user key', is_int($intKey->userKey()));
is_true('Key stringifies for logs', str_starts_with((string) $intKey, "$namespace:$set:"));

$nullSet = new Key($namespace, '', 'in-the-null-set');
try {
    $client->put(null, $nullSet, [new Bin('kind', 'null-set')]);
    is_same(
        'the null set is a legal set',
        'null-set',
        $client->get(null, $nullSet)?->bin('kind')
    );
    $client->delete(null, $nullSet);
} catch (Aerospike\AerospikeException $e) {
    fail('the null set is a legal set', describe($e));
}

is_client_error(
    'a Key without a namespace is refused',
    'needs a namespace',
    fn() => new Key('', $set, 'x')
);
is_client_error(
    'a float user key is refused',
    'key',
    fn() => new Key($namespace, $set, 1.5)
);

// ===== Deleting a bin by writing null =======================================

section('bin deletion');

try {
    $client->put(null, $key, [new Bin('name', null)]);
    $after = $client->get(null, $key);
    is_true(
        'writing null deletes the bin',
        !$after->has('name'),
        'bin "name" is still ' . var_export($after->bin('name'), true)
    );
    is_same('other bins survived the delete', 30, $after->bin('age'));
} catch (Aerospike\AerospikeException $e) {
    fail('writing null deletes the bin', describe($e));
}

// ===== Values PHP can hold but Aerospike cannot =============================

section('rejected values');

foreach (
    [
        'an object'          => new stdClass(),
        'a non-UTF-8 string' => "\xff\xfe",
    ] as $what => $value
) {
    is_client_error("$what is rejected, naming the bin", 'bin "bad"', fn() => new Bin('bad', $value));
}

// The conversion happens when the Bin is built, so the failure names the bin
// even before a command exists to send it in.
ok('a bad value fails at the Bin, not at the call');

// ===== Value shapes PHP has no literal for ==================================

section('blobs, GeoJSON and HLL');

// Deliberately not valid UTF-8: a bare PHP string like this is refused (see
// above), and wrapping it is the only way to store it.
$binary  = "\x00\xff\xfe binary \x01";
$geojson = '{"type":"Point","coordinates":[13.4050,52.5200]}';
$blobKey = fresh_key('blob');

try {
    $client->put(null, $blobKey, [
        new Bin('data', new Blob($binary)),
        new Bin('where', new GeoJson($geojson)),
    ]);
    $wrapped = $client->get(null, $blobKey);

    is_true(
        'a blob comes back as Aerospike\\Blob',
        $wrapped->bin('data') instanceof Blob,
        'got ' . var_export($wrapped->bin('data'), true)
    );
    if ($wrapped->bin('data') instanceof Blob) {
        is_same('blob bytes round trip exactly', $binary, $wrapped->bin('data')->bytes());
        is_same('Blob::length() is the byte count', strlen($binary), $wrapped->bin('data')->length());
        is_same('(string) $blob is its bytes', $binary, (string) $wrapped->bin('data'));
    }

    is_true(
        'GeoJSON comes back as Aerospike\\GeoJson',
        $wrapped->bin('where') instanceof GeoJson,
        'got ' . var_export($wrapped->bin('where'), true)
    );
    if ($wrapped->bin('where') instanceof GeoJson) {
        // Compared as documents, not as strings: the server is entitled to
        // normalise the JSON it stores, and only the geometry is the data.
        is_same_value(
            'the GeoJSON document round trips',
            json_decode($geojson, true),
            json_decode($wrapped->bin('where')->json(), true)
        );
    }
} catch (Aerospike\AerospikeException $e) {
    fail('a blob and a GeoJSON document round trip', describe($e));
}

// A binary record key, which is the other half of the same gap: PHP cannot say
// "these bytes are a key" without the wrapper either.
try {
    $rawKey = new Key($namespace, $set, new Blob("\x01\x02\xff\xfe"));
    $client->put(null, $rawKey, [new Bin('kind', 'blob-key')]);
    $keyed = $client->get(null, new Key($namespace, $set, new Blob("\x01\x02\xff\xfe")));
    is_same('a binary record key round trips', 'blob-key', $keyed?->bin('kind'));
} catch (Aerospike\AerospikeException $e) {
    fail('a binary record key round trips', describe($e));
}

// An HLL sketch can only come from the server's own HLL operations, which are a
// later phase, so only the wrapper itself is checked here.
$sketch = new Aerospike\Hll("\x00\x01\x02");
is_same('Hll wraps its bytes', "\x00\x01\x02", $sketch->bytes());
is_same('Hll::length() is the byte count', 3, $sketch->length());

// ===== exists / delete ======================================================

section('exists and delete');

$gone = fresh_key('gone');

try {
    $client->put(null, $gone, [new Bin('x', 1)]);
    is_same('exists sees a record that is there', true, $client->exists(null, $gone));
    is_same('delete reports that the record existed', true, $client->delete(null, $gone));
    is_same('exists sees it gone', false, $client->exists(null, $gone));
    // Deleting what is already absent leaves the end state that was asked for,
    // so it is an answer rather than a failure.
    is_same('deleting an absent record reports false', false, $client->delete(null, $gone));
    is_same('a deleted record reads as null', null, $client->get(null, $gone));
    is_same(
        'exists is false for a key never written',
        false,
        $client->exists(null, fresh_key('never'))
    );
} catch (Aerospike\AerospikeException $e) {
    unsupported($e)
        ? skip('exists and delete', 'the daemon does not serve these verbs yet: ' . $e->getMessage())
        : fail('exists and delete', describe($e));
}

// ===== touch ================================================================

section('touch');

$touched = fresh_key('touch');

try {
    $client->put(null, $touched, [new Bin('x', 1)]);
    $before = $client->get(null, $touched);

    // The expiration is what a touch is usually *for*, and a namespace that
    // refuses one still has to bump the generation — so the TTL assertions skip
    // there and the rest of the section runs either way.
    $client->touch(
        $finiteTtls ? new WritePolicy(expiration: Expiration::seconds(7200)) : null,
        $touched
    );
    $after = $client->get(null, $touched);

    is_true(
        'touch bumped the generation',
        $after->generation() > $before->generation(),
        'generation went from ' . $before->generation() . ' to ' . $after->generation()
    );
    if ($finiteTtls) {
        is_true(
            'touch set the ttl from the policy expiration',
            is_int($after->ttl()) && $after->ttl() > 7100 && $after->ttl() <= 7200,
            'ttl is ' . var_export($after->ttl(), true)
        );
    } else {
        skip('touch set the ttl from the policy expiration', $noFiniteTtl);
    }

    // dontUpdate() is the case that leaves the TTL alone, which is exactly what
    // distinguishes it from namespaceDefault().
    $client->touch(new WritePolicy(expiration: Expiration::dontUpdate()), $touched);
    $kept = $client->get(null, $touched);
    if ($finiteTtls) {
        is_true(
            'Expiration::dontUpdate() leaves the ttl alone',
            is_int($kept->ttl()) && $kept->ttl() > 7100 && $kept->ttl() <= 7200,
            'ttl is ' . var_export($kept->ttl(), true)
        );
    } else {
        is_same('Expiration::dontUpdate() leaves the ttl alone', $after->ttl(), $kept->ttl());
    }
    is_true(
        'a dontUpdate() touch still bumped the generation',
        $kept->generation() > $after->generation(),
        'generation went from ' . $after->generation() . ' to ' . $kept->generation()
    );

    // Unlike delete() and exists(), there is no record whose life could be
    // extended, so this is a failure rather than an answer.
    try {
        $client->touch(null, fresh_key('absent-touch'));
        fail('touching a missing record throws', 'it succeeded');
    } catch (Aerospike\AerospikeException $e) {
        is_true(
            'touching a missing record throws',
            in_array($e->getStatus(), [Status::RecordNotFound, Status::Server], true),
            describe($e)
        );
    }
} catch (Aerospike\AerospikeException $e) {
    unsupported($e)
        ? skip('touch', 'the daemon does not serve TOUCH yet: ' . $e->getMessage())
        : fail('touch', describe($e));
}

// ===== add / append / prepend ===============================================

section('add, append and prepend');

$counter = fresh_key('counter');

try {
    $client->put(null, $counter, [
        new Bin('views', 10),
        new Bin('seconds', 0.5),
        new Bin('word', 'B'),
    ]);

    $client->add(null, $counter, [new Bin('views', 5), new Bin('seconds', 0.25)]);
    $summed = $client->get(null, $counter);
    is_same('add sums an integer bin', 15, $summed->bin('views'));
    is_same('add sums a double bin', 0.75, $summed->bin('seconds'));

    // A negative delta is how a counter is decremented; there is no separate
    // verb for it.
    $client->add(null, $counter, [new Bin('views', -3)]);
    is_same('a negative delta subtracts', 12, $client->get(null, $counter)->bin('views'));

    $client->append(null, $counter, [new Bin('word', ' C')]);
    $client->prepend(null, $counter, [new Bin('word', 'A ')]);
    is_same(
        'append and prepend both landed, in order',
        'A B C',
        $client->get(null, $counter)->bin('word')
    );

    // Adding to a record that does not exist creates it, so a counter needs no
    // initialisation.
    $freshCounter = fresh_key('fresh');
    $client->add(null, $freshCounter, [new Bin('hits', 1)]);
    is_same(
        'add creates the record and the bin',
        1,
        $client->get(null, $freshCounter)->bin('hits')
    );

    // Refused here rather than by the server, which would say only
    // "PARAMETER_ERROR" and name neither the bin nor the reason.
    is_client_error(
        'add refuses a non-numeric delta, naming the bin',
        'bin "views"',
        fn() => $client->add(null, $counter, [new Bin('views', 'lots')])
    );
    is_client_error(
        'append refuses a non-string value, naming the bin',
        'bin "word"',
        fn() => $client->append(null, $counter, [new Bin('word', 7)])
    );
} catch (Aerospike\AerospikeException $e) {
    unsupported($e)
        ? skip('add, append and prepend', 'the daemon does not serve these verbs yet: ' . $e->getMessage())
        : fail('add, append and prepend', describe($e));
}

// ===== Policies that reach the server =======================================

section('policies');

$guarded = fresh_key('policy');

try {
    $client->put(null, $guarded, [new Bin('x', 1)]);
} catch (Aerospike\AerospikeException $e) {
    fail('the policy fixture was written', describe($e));
}

// A TTL asked for on the write itself, read back through get().
try {
    if ($finiteTtls) {
        $client->put(new WritePolicy(expiration: Expiration::seconds(3600)), $guarded, [new Bin('x', 1)]);
        $ttl = $client->get(null, $guarded)->ttl();
        is_true(
            'a put expiration is the ttl a get reports',
            is_int($ttl) && $ttl > 3500 && $ttl <= 3600,
            'ttl is ' . var_export($ttl, true)
        );
    } else {
        skip('a put expiration is the ttl a get reports', $noFiniteTtl);
    }

    // never() is the other end of the same field, and the reason these are named
    // cases: a get reports it as null rather than as a number of seconds. Every
    // namespace accepts it — it asks the reaper for nothing.
    $client->put(new WritePolicy(expiration: Expiration::never()), $guarded, [new Bin('x', 1)]);
    is_same('Expiration::never() reads back as a null ttl', null, $client->get(null, $guarded)->ttl());

    // Back to a finite TTL, so the record does not outlive the test run.
    if ($finiteTtls) {
        $client->put(new WritePolicy(expiration: Expiration::seconds(3600)), $guarded, [new Bin('x', 1)]);
    }
} catch (Aerospike\AerospikeException $e) {
    fail('a put expiration is the ttl a get reports', describe($e));
}

// A policy that reaches the server: CreateOnly on a record that exists must be
// refused, with the server's own KEY_EXISTS_ERROR (5).
try {
    $client->put(
        new WritePolicy(recordExistsAction: RecordExistsAction::CreateOnly),
        $guarded,
        [new Bin('x', 9)]
    );
    fail('RecordExistsAction::CreateOnly is enforced', 'the write was applied');
} catch (Aerospike\AerospikeException $e) {
    is_same('RecordExistsAction::CreateOnly is enforced by the server', 5, $e->getResultCode());
    is_same('a server refusal is Status::Server', Status::Server, $e->getStatus());
}

// A filter, which needs server 8.1.3 or later. Filtered out is an *error* here —
// result code 27 — not an empty result, so it is control flow a caller catches.
try {
    $matched = $client->get(new ReadPolicy(filter: '$.x == 1'), $guarded);
    is_same('a read whose filter matches returns the record', 1, $matched?->bin('x'));

    try {
        $client->get(new ReadPolicy(filter: '$.x == 999'), $guarded);
        fail('a read whose filter rejects the record throws', 'it returned a record');
    } catch (Aerospike\AerospikeException $e) {
        is_same('a filtered-out read reports FILTERED_OUT (27)', 27, $e->getResultCode());
        is_same('a filtered-out read is a SERVER failure', Status::Server, $e->getStatus());
    }

    try {
        $client->add(new WritePolicy(filter: '$.x == 999'), $guarded, [new Bin('x', 100)]);
        fail('a write whose filter rejects the record throws', 'it was applied');
    } catch (Aerospike\AerospikeException $e) {
        is_same('a filtered-out write reports FILTERED_OUT (27)', 27, $e->getResultCode());
    }
    is_same('the filtered-out write did not land', 1, $client->get(null, $guarded)->bin('x'));
} catch (Aerospike\AerospikeException $e) {
    // An older server rejects the filter itself rather than applying it.
    skip('expression-language filters', describe($e));
}

// A write guarded by the wrong generation must be refused by the *server*, with
// its own result code — 3, GENERATION_ERROR — not by a client-side guess.
try {
    $generation = $client->get(null, $guarded)->generation();

    try {
        $client->add(
            new WritePolicy(
                generationPolicy: GenerationPolicy::ExpectGenEqual,
                generation: $generation + 7
            ),
            $guarded,
            [new Bin('x', 1)]
        );
        fail('a generation-guarded write is refused', 'it was applied');
    } catch (Aerospike\AerospikeException $e) {
        is_same('a generation-guarded write reports the server status', Status::Server, $e->getStatus());
        is_same('a generation-guarded write reports GENERATION_ERROR (3)', 3, $e->getResultCode());
        is_same(
            'the guarded write did not land',
            $generation,
            $client->get(null, $guarded)->generation()
        );
    }

    // The same guard with the *right* generation must go through, or the test
    // above would pass for the wrong reason.
    $client->add(
        new WritePolicy(
            generationPolicy: GenerationPolicy::ExpectGenEqual,
            generation: $generation
        ),
        $guarded,
        [new Bin('x', 1)]
    );
    is_same('the same guard with the right generation applies', 2, $client->get(null, $guarded)->bin('x'));
} catch (Aerospike\AerospikeException $e) {
    fail('generation-guarded writes', describe($e));
}

// Every shared field, on a read, in one policy: these have no observable effect
// on a single-node cluster, so what is being tested is that they are accepted
// and carried rather than rejected.
try {
    is_same(
        'a fully populated read policy is accepted',
        true,
        $client->exists(
            new ReadPolicy(
                totalTimeoutMs: 500,
                socketTimeoutMs: 250,
                maxRetries: 1,
                sleepBetweenRetriesMs: 10,
                replica: Replica::Master,
                readModeAp: ReadModeAP::One,
                readModeSc: ReadModeSC::Session,
                useCompression: false
            ),
            $guarded
        )
    );
} catch (Aerospike\AerospikeException $e) {
    fail('a fully populated read policy is accepted', describe($e));
}

// And every write-only field, on a write.
try {
    $client->put(
        new WritePolicy(
            totalTimeoutMs: 500,
            replica: Replica::Sequence,
            recordExistsAction: RecordExistsAction::Update,
            // Every other field here is the client's; the expiration is the one
            // the *namespace* can refuse, so it follows what it allows.
            expiration: $finiteTtls ? Expiration::seconds(3600) : Expiration::never(),
            commitLevel: CommitLevel::CommitAll,
            respondPerEachOp: false,
            sendKey: true
        ),
        $guarded,
        [new Bin('x', 2)]
    );
    ok('a fully populated write policy is accepted');
} catch (Aerospike\AerospikeException $e) {
    fail('a fully populated write policy is accepted', describe($e));
}

// A policy object is immutable and reusable: the same instance must work twice,
// which is what makes hoisting one into a static safe.
$reused = new ReadPolicy(totalTimeoutMs: 1000);
try {
    $client->exists($reused, $guarded);
    $client->exists($reused, $guarded);
    ok('one policy instance can be reused');
    is_same('a policy reports what it was given', 1000, $reused->totalTimeoutMs());
    is_same('an unset policy field reads as null', null, $reused->maxRetries());
} catch (Aerospike\AerospikeException $e) {
    fail('one policy instance can be reused', describe($e));
}

// ===== operate: several operations, one record, in order ====================

section('operate');

$ops = fresh_key('ops');

try {
    // Order matters and is the whole point: the put lands before the list is
    // appended to, and the size is read after.
    $record = $client->operate(null, $ops, [
        Op::put(new Bin('name', 'Alice')),
        Op::put(new Bin('views', 1)),
        ListOp::appendItems('items', [3, 1, 2]),
        ListOp::size('items'),
    ]);
    is_true('operate returns a Record', $record instanceof Record);

    // Two operations wrote to "items" and both reported a size, so the bin's
    // value is the list of their results in operation order.
    is_same_value(
        'two results for one bin come back as a list, in order',
        [3, 3],
        $record->bin('items')
    );
    is_same('an operation that reports nothing contributes no bin', false, $record->has('name'));

    // The record now exists and holds what the operations wrote.
    $stored = $client->get(null, $ops);
    is_same('a written bin landed', 'Alice', $stored->bin('name'));
    is_same_value('the list landed in insertion order', [3, 1, 2], $stored->bin('items'));
} catch (Aerospike\AerospikeException $e) {
    unsupported($e)
        ? skip('operate', 'the daemon does not serve OPERATE yet: ' . $e->getMessage())
        : fail('operate', describe($e));
}

try {
    // A read-modify-write in one round trip, atomically.
    $record = $client->operate(null, $ops, [
        Op::add(new Bin('views', 5)),
        ListOp::sort('items'),
        ListOp::getRange('items', 0, null),
        Op::getBin('views'),
    ]);
    is_same('add through operate accumulates', 6, $record->bin('views'));
    is_same_value('sort ordered the list in place', [1, 2, 3], $record->bin('items'));

    // Every index and rank may count from the end.
    $record = $client->operate(null, $ops, [
        ListOp::getByIndex('items', 0, ListReturn::Values),
        ListOp::getByRank('items', -1, ListReturn::Values),
    ]);
    is_same_value('index 0 and rank -1 are the ends of the list', [1, 3], $record->bin('items'));

    // A count of null means "to the end", which is the pair of client
    // constructors this API collapses into one method.
    $record = $client->operate(null, $ops, [
        ListOp::getByIndexRange('items', 1, null, ListReturn::Values),
        ListOp::getByIndexRange('items', 1, 1, ListReturn::Values),
    ]);
    is_same_value(
        'a null count reads to the end, a given count reads that many',
        [[2, 3], [2]],
        $record->bin('items')
    );

    // Inverted selects everything the range did not.
    $record = $client->operate(null, $ops, [
        ListOp::getByValueRange('items', 2, null, ListReturn::Values),
        ListOp::getByValueRange('items', 2, null, ListReturn::Values, inverted: true),
    ]);
    is_same_value(
        'inverted returns everything outside the range',
        [[2, 3], [1]],
        $record->bin('items')
    );

    // The return type decides the shape, not just the contents.
    $record = $client->operate(null, $ops, [
        ListOp::getByValueRange('items', 1, null, ListReturn::Count),
        ListOp::getByValueRange('items', 1, null, ListReturn::Index),
        ListOp::getByValue('items', 99, ListReturn::Exists),
    ]);
    is_same_value(
        'each return type has its own shape',
        [3, [0, 1, 2], false],
        $record->bin('items')
    );
} catch (Aerospike\AerospikeException $e) {
    fail('list operations', describe($e));
}

// A list nested inside a map, reached with a context path.
$nested = fresh_key('nested');
try {
    $client->put(null, $nested, [new Bin('profile', ['roles' => ['reader']])]);

    $record = $client->operate(null, $nested, [
        ListOp::append('profile', 'admin')->context([Ctx::mapKey('roles')]),
        ListOp::size('profile')->context([Ctx::mapKey('roles')]),
    ]);
    is_same_value('a nested list grew', [2, 2], $record->bin('profile'));

    $stored = $client->get(null, $nested)->bin('profile');
    is_same_value('the path reached the nested list', ['reader', 'admin'], $stored['roles'] ?? null);
} catch (Aerospike\AerospikeException $e) {
    fail('a context path reaches a nested list', describe($e));
}

// A list policy: ordered storage, and what the three write flags actually do to
// a batch containing a duplicate. The three outcomes are worth pinning, because
// only one of them is the one people expect.
$policed = fresh_key('policed');
try {
    // addUnique alone: the duplicate makes the server refuse the operation.
    try {
        $client->operate(null, $policed, [
            ListOp::appendItems(
                'sorted',
                [5, 1, 5, 3],
                new ListPolicy(order: ListOrder::Ordered, addUnique: true)
            ),
        ]);
        fail('addUnique alone refuses a batch with a duplicate', 'it was accepted');
    } catch (Aerospike\AerospikeException $e) {
        is_same(
            'addUnique alone refuses a batch with a duplicate (result code 26)',
            26,
            $e->getResultCode()
        );
    }

    // noFail turns that refusal into silence — and discards the *whole* batch,
    // not just the duplicate. This is the trap: nothing is written and nothing
    // is reported.
    $client->operate(null, $policed, [
        ListOp::appendItems(
            'sorted',
            [5, 1, 5, 3],
            new ListPolicy(order: ListOrder::Ordered, addUnique: true, noFail: true)
        ),
    ]);
    is_same_value(
        'noFail without partial discards the whole batch, silently',
        [],
        $client->get(null, $policed)?->bin('sorted') ?? []
    );

    // partial is what keeps the acceptable items.
    $policy = new ListPolicy(
        order: ListOrder::Ordered,
        addUnique: true,
        noFail: true,
        partial: true
    );
    $client->operate(null, $policed, [ListOp::appendItems('sorted', [5, 1, 5, 3], $policy)]);
    is_same_value(
        'noFail with partial keeps the unique items, in order',
        [1, 3, 5],
        $client->get(null, $policed)->bin('sorted')
    );

    is_same(
        'ListPolicy reports what it was given',
        true,
        $policy->isOrdered() && $policy->addUnique() && $policy->noFail() && $policy->partial()
    );
    is_same('an unset ListPolicy flag is false', false, (new ListPolicy())->addUnique());
} catch (Aerospike\AerospikeException $e) {
    fail('a list policy is honoured', describe($e));
}

// Remove operations, and the write/read split.
try {
    $record = $client->operate(null, $policed, [
        ListOp::removeByValue('sorted', 3, ListReturn::Values),
        ListOp::getRange('sorted', 0, null),
    ]);
    is_same_value('removeByValue returns what it removed', [[3], [1, 5]], $record->bin('sorted'));

    is_same('a read-only operate is not a write', false, ListOp::size('sorted')->isWrite());
    is_same('an append is a write', true, ListOp::append('sorted', 1)->isWrite());
    is_same('an operation reports its bin', 'sorted', ListOp::size('sorted')->bin());
    is_same('a whole-record read names no bin', null, Op::get()->bin());
} catch (Aerospike\AerospikeException $e) {
    fail('remove operations', describe($e));
}

// An operate on a record that does not exist, and only reads, is null — the
// same answer get() gives.
try {
    is_same(
        'a read-only operate on a missing record is null',
        null,
        $client->operate(null, fresh_key('absent-ops'), [ListOp::size('items')])
    );
} catch (Aerospike\AerospikeException $e) {
    fail('a read-only operate on a missing record is null', describe($e));
}

// ===== the two map kinds a PHP array cannot express =========================

section('OrderedMap and SortedMap');

$kinds = fresh_key('map-kinds');

try {
    $client->put(null, $kinds, [
        new Bin('sorted', new SortedMap(['b' => 2, 'a' => 1])),
        new Bin('ordered', new OrderedMap(['z' => 26, 'y' => 25])),
        new Bin('plain', ['q' => 1]),
    ]);
    $record = $client->get(null, $kinds);

    // A key-ordered map is the one kind the server keeps, so it comes back as
    // itself — and in the server's order, not the order it was written in.
    is_true(
        'a SortedMap round trips as a SortedMap',
        $record->bin('sorted') instanceof SortedMap,
        'got ' . get_debug_type($record->bin('sorted'))
    );
    is_same_value(
        'a SortedMap comes back in the server key order',
        ['a' => 1, 'b' => 2],
        $record->bin('sorted')->toArray()
    );
    is_same_value(
        'a SortedMap iterates in that order too',
        ['a', 'b'],
        iterator_to_array((function ($m) {
            foreach ($m as $k => $_) {
                yield $k;
            }
        })($record->bin('sorted')), false)
    );

    // Insertion order is not a thing the server stores, so an OrderedMap is
    // written as an unordered map and reads back as a plain array. Reported
    // rather than pretended otherwise.
    is_same('an OrderedMap reads back as a plain array', 'array', get_debug_type($record->bin('ordered')));
    is_same('a plain array is still a plain array', 'array', get_debug_type($record->bin('plain')));
} catch (Aerospike\AerospikeException $e) {
    fail('the map kinds round trip', describe($e));
}

// The API surface: get/set, ArrayAccess, Iterator, Countable.
$map = new SortedMap(['b' => 2]);
$map->set('a', 1);
$map['c'] = 3;

is_same('set and get agree', 1, $map->get('a'));
is_same('ArrayAccess reads what set wrote', 3, $map['c']);
is_same('count() counts the entries', 3, count($map));
is_same('has() finds a key', true, $map->has('b'));
is_same('isset() agrees with has()', false, isset($map['nope']));
is_same('get() of a missing key is null', null, $map->get('nope'));
is_same_value('keys() is in order', ['b', 'a', 'c'], $map->keys());
is_same_value('values() is in the same order', [2, 1, 3], $map->values());
is_same_value('toArray() is the whole map', ['b' => 2, 'a' => 1, 'c' => 3], $map->toArray());

// Setting an existing key changes the value and leaves it where it was:
// changing a value is not a reordering.
$map->set('b', 20);
is_same_value(
    'setting an existing key replaces it in place',
    ['b' => 20, 'a' => 1, 'c' => 3],
    $map->toArray()
);

is_same('remove() reports that the key was there', true, $map->remove('a'));
is_same('remove() of a missing key is false', false, $map->remove('a'));
unset($map['c']);
is_same_value('unset() removes through ArrayAccess', ['b' => 20], $map->toArray());
is_same('an empty map knows it', true, (new OrderedMap())->isEmpty());

foreach ([SortedMap::class, OrderedMap::class] as $class) {
    $instance = new $class(['x' => 1]);
    is_true("$class is an Iterator, ArrayAccess and Countable", $instance instanceof Iterator
        && $instance instanceof ArrayAccess
        && $instance instanceof Countable);
}

// The second thing an array cannot do: a key that is not an int or a string.
// `foreach` hands it back as it is, because Iterator::key() is not restricted
// the way an array key is.
$binaryKeyed = fresh_key('binary-keys');
try {
    $keyed = new SortedMap();
    $keyed->set(new Blob("\x00\xff"), 'bytes');
    $keyed->set(7, 'seven');
    $client->put(null, $binaryKeyed, [new Bin('m', $keyed)]);

    $back = $client->get(null, $binaryKeyed)->bin('m');
    $types = [];
    foreach ($back as $key => $_) {
        $types[] = get_debug_type($key);
    }
    is_same_value(
        'a binary map key survives the round trip and iterates as a Blob',
        ['int', 'Aerospike\Blob'],
        $types
    );

    // …and toArray() refuses it rather than flattening it, because the caller
    // is holding the faithful thing and would be throwing it away.
    is_client_error(
        'toArray() refuses a key a PHP array cannot hold',
        'binary key',
        fn() => $back->toArray()
    );
} catch (Aerospike\AerospikeException $e) {
    fail('a binary map key survives', describe($e));
}

// Accepted wherever a map is: as a bin value, nested, and as operation items.
try {
    $nested = fresh_key('nested-kinds');
    $client->put(null, $nested, [
        new Bin('outer', ['inner' => new SortedMap(['k' => 'v'])]),
    ]);
    $inner = $client->get(null, $nested)->bin('outer')['inner'] ?? null;
    is_true(
        'a SortedMap nested inside an array round trips',
        $inner instanceof SortedMap,
        'got ' . get_debug_type($inner)
    );

    $items = fresh_key('putitems-kinds');
    $record = $client->operate(null, $items, [
        MapOp::putItems('m', new SortedMap(['k' => 1])),
        MapOp::size('m'),
    ]);
    is_same_value('putItems accepts a SortedMap', [1, 1], $record->bin('m'));
} catch (Aerospike\AerospikeException $e) {
    fail('the map kinds are accepted wherever a map is', describe($e));
}

is_client_error(
    'a map argument refuses something that is not a map',
    'expected a map',
    fn() => MapOp::putItems('m', 'not a map')
);

// ===== map operations =======================================================

section('map operations');

$maps = fresh_key('maps');

try {
    $client->delete(null, $maps);

    $record = $client->operate(null, $maps, [
        MapOp::putItems('scores', ['alice' => 10, 'bob' => 20, 'carol' => 5]),
        MapOp::size('scores'),
        MapOp::getByKey('scores', 'bob', MapReturn::Value),
    ]);
    is_same_value(
        'putItems, size and getByKey each report their own result',
        [3, 3, 20],
        $record->bin('scores')
    );

    // A map entry has two halves, and the return type picks which comes back.
    $record = $client->operate(null, $maps, [
        MapOp::getByRank('scores', -1, MapReturn::Key),
        MapOp::getByRank('scores', -1, MapReturn::Value),
        MapOp::getByRank('scores', -1, MapReturn::KeyValue),
    ]);
    is_same_value(
        'the return type chooses the key, the value, or both',
        ['bob', 20, [['bob', 20]]],
        $record->bin('scores')
    );

    // Ranges over keys and over values are different questions.
    $record = $client->operate(null, $maps, [
        MapOp::getByKeyRange('scores', 'b', null, MapReturn::Key),
        MapOp::getByValueRange('scores', 6, null, MapReturn::Key),
        MapOp::getByValueRange('scores', 6, null, MapReturn::Key, inverted: true),
    ]);
    is_same_value(
        'key ranges and value ranges select different entries',
        [['bob', 'carol'], ['alice', 'bob'], ['carol']],
        $record->bin('scores')
    );

    // increment and decrement land on the value, creating the entry if needed.
    $record = $client->operate(null, $maps, [
        MapOp::incrementValue('scores', 'alice', 5),
        MapOp::decrementValue('scores', 'bob', 8),
        MapOp::incrementValue('scores', 'dave', 1),
    ]);
    is_same_value(
        'increment and decrement report the new values',
        [15, 12, 1],
        $record->bin('scores')
    );

    // Removing by key returns what it removed, and the map shrinks.
    $record = $client->operate(null, $maps, [
        MapOp::removeByKey('scores', 'dave', MapReturn::Value),
        MapOp::size('scores'),
    ]);
    is_same_value('removeByKey returns what it removed', [1, 3], $record->bin('scores'));

    // A whole map read as a map, not as pairs.
    $record = $client->operate(null, $maps, [
        MapOp::getByKeyRange('scores', null, null, MapReturn::UnorderedMap),
    ]);
    is_same_value(
        'an unbounded key range with UnorderedMap reads the whole map',
        ['alice' => 15, 'bob' => 12, 'carol' => 5],
        $record->bin('scores')
    );
} catch (Aerospike\AerospikeException $e) {
    unsupported($e)
        ? skip('map operations', 'the daemon does not serve them yet: ' . $e->getMessage())
        : fail('map operations', describe($e));
}

// A map policy: an ordered map whose writes must not create keys.
$ordered = fresh_key('ordered-map');
try {
    $client->operate(null, $ordered, [
        MapOp::putItems(
            'm',
            ['b' => 2, 'a' => 1],
            new MapPolicy(order: MapOrder::KeyOrdered)
        ),
    ]);
    // A key-ordered map comes back as a SortedMap, not an array: the ordering
    // is real storage, and handing back a plain array would lose it on the
    // next write.
    is_same_value(
        'a key-ordered map comes back in key order, as a SortedMap',
        ['a' => 1, 'b' => 2],
        $client->get(null, $ordered)->bin('m')->toArray()
    );

    // UpdateOnly refuses a key that is not there — and with noFail it refuses
    // silently, which is the same trap the list write flags have.
    try {
        $client->operate(null, $ordered, [
            MapOp::put('m', 'c', 3, new MapPolicy(writeMode: MapWriteMode::UpdateOnly)),
        ]);
        fail('MapWriteMode::UpdateOnly refuses a missing key', 'it was accepted');
    } catch (Aerospike\AerospikeException $e) {
        is_true(
            'MapWriteMode::UpdateOnly refuses a missing key',
            $e->getResultCode() !== null,
            describe($e)
        );
    }

    $client->operate(null, $ordered, [
        MapOp::put(
            'm',
            'c',
            3,
            new MapPolicy(writeMode: MapWriteMode::UpdateOnly, noFail: true)
        ),
    ]);
    is_same_value(
        'with noFail the refusal is silent and nothing is written',
        ['a' => 1, 'b' => 2],
        $client->get(null, $ordered)->bin('m')->toArray()
    );

    // CreateOnly is the other half of the same idea.
    $client->operate(null, $ordered, [
        MapOp::put('m', 'c', 3, new MapPolicy(writeMode: MapWriteMode::CreateOnly)),
    ]);
    is_same_value(
        'CreateOnly writes a key that was not there',
        ['a' => 1, 'b' => 2, 'c' => 3],
        $client->get(null, $ordered)->bin('m')->toArray()
    );
} catch (Aerospike\AerospikeException $e) {
    fail('a map policy is honoured', describe($e));
}

// A map nested inside a map, and a list inside a map — the same path mechanism.
$deep = fresh_key('deep');
try {
    $client->put(null, $deep, [new Bin('tree', ['a' => ['x' => 1]])]);

    $record = $client->operate(null, $deep, [
        MapOp::put('tree', 'y', 2)->context([Ctx::mapKey('a')]),
        MapOp::size('tree')->context([Ctx::mapKey('a')]),
    ]);
    is_same_value('a nested map grew', [2, 2], $record->bin('tree'));
    is_same_value(
        'the path reached the nested map',
        ['a' => ['x' => 1, 'y' => 2]],
        $client->get(null, $deep)->bin('tree')
    );

    // mapKeyCreate builds the missing level on the way down.
    $record = $client->operate(null, $deep, [
        MapOp::put('tree', 'z', 3)->context([Ctx::mapKeyCreate('b', MapOrder::KeyOrdered)]),
    ]);
    is_same_value(
        'mapKeyCreate created the missing level',
        ['a' => ['x' => 1, 'y' => 2], 'b' => ['z' => 3]],
        $client->get(null, $deep)->bin('tree')
    );
} catch (Aerospike\AerospikeException $e) {
    fail('a context path reaches a nested map', describe($e));
}

// ===== bitwise operations ===================================================

section('bitwise operations');

$bits = fresh_key('bits');

try {
    // 0x01 0x02 0x03 0x04 — five bits set in all.
    $client->put(null, $bits, [new Bin('flags', new Blob("\x01\x02\x03\x04"))]);

    $record = $client->operate(null, $bits, [
        BitOp::count('flags', 0, 32),
        BitOp::getInt('flags', 0, 8, false),
        BitOp::lscan('flags', 0, 32, true),
    ]);
    is_same_value(
        'count, getInt and lscan read the blob as bits',
        [5, 1, 7],
        $record->bin('flags')
    );

    // A write in the middle of a read sequence, and the read after it sees it.
    $record = $client->operate(null, $bits, [
        BitOp::set('flags', 0, 8, new Blob("\xff")),
        BitOp::getInt('flags', 0, 8, false),
        BitOp::count('flags', 0, 32),
    ]);
    is_same_value(
        'a bit write is visible to the read after it',
        [255, 12],
        $record->bin('flags')
    );
    is_same(
        'an operation that reports nothing adds no result',
        2,
        count($record->bin('flags'))
    );

    // The units differ between operations, and the names say which: resize is
    // in bytes, everything else in bits.
    $client->operate(null, $bits, [BitOp::resize('flags', 6, BitResize::AtEnd)]);
    is_same(
        'resize works in bytes',
        6,
        strlen($client->get(null, $bits)->bin('flags')->bytes())
    );

    // Arithmetic on a bit field, and what happens when it does not fit.
    $client->put(null, $bits, [new Bin('counter', new Blob("\x00"))]);
    $record = $client->operate(null, $bits, [
        BitOp::add('counter', 0, 8, 200, false),
        BitOp::getInt('counter', 0, 8, false),
    ]);
    is_same('add lands on the integer in the field', 200, $record->bin('counter'));

    try {
        $client->operate(null, $bits, [BitOp::add('counter', 0, 8, 100, false)]);
        fail('overflow fails by default', 'it was accepted');
    } catch (Aerospike\AerospikeException $e) {
        is_true(
            'overflow fails by default, rather than silently wrapping',
            $e->getResultCode() !== null,
            describe($e)
        );
    }

    $client->operate(null, $bits, [
        BitOp::add('counter', 0, 8, 100, false, overflow: BitOverflow::Saturate),
    ]);
    is_same(
        'Saturate clamps instead of failing',
        255,
        $client->operate(null, $bits, [BitOp::getInt('counter', 0, 8, false)])->bin('counter')
    );

    $client->operate(null, $bits, [
        BitOp::add('counter', 0, 8, 1, false, overflow: BitOverflow::Wrap),
    ]);
    is_same(
        'Wrap comes back round to zero',
        0,
        $client->operate(null, $bits, [BitOp::getInt('counter', 0, 8, false)])->bin('counter')
    );
} catch (Aerospike\AerospikeException $e) {
    unsupported($e)
        ? skip('bitwise operations', 'the daemon does not serve them yet: ' . $e->getMessage())
        : fail('bitwise operations', describe($e));
}

// ===== HyperLogLog operations ===============================================

section('HyperLogLog operations');

$sketch = fresh_key('hll');

try {
    $record = $client->operate(null, $sketch, [
        HllOp::add('visitors', ['u1', 'u2', 'u3'], indexBitCount: 12),
        HllOp::getCount('visitors'),
        HllOp::describe('visitors'),
    ]);
    is_same_value(
        'add reports the registers it changed, and the count follows',
        [3, 3, [12, 0]],
        $record->bin('visitors')
    );

    // Adding what is already there changes nothing — the whole point of the
    // structure. The count is an estimate, but at this size it is exact.
    $record = $client->operate(null, $sketch, [
        HllOp::add('visitors', ['u1', 'u2']),
        HllOp::getCount('visitors'),
    ]);
    is_same_value('re-adding known values does not raise the count', [0, 3], $record->bin('visitors'));

    // Two sketches, and the set arithmetic between them.
    $other = fresh_key('hll-other');
    $client->operate(null, $other, [
        HllOp::add('visitors', ['u3', 'u4'], indexBitCount: 12),
    ]);
    $second = $client->get(null, $other)->bin('visitors');
    is_true('a sketch reads back as Aerospike\\Hll', $second instanceof Aerospike\Hll);

    $record = $client->operate(null, $sketch, [
        HllOp::getUnionCount('visitors', [$second]),
        HllOp::getIntersectCount('visitors', [$second]),
        HllOp::getSimilarity('visitors', [$second]),
    ]);
    $results = $record->bin('visitors');
    is_same('the union of {u1,u2,u3} and {u3,u4} is 4', 4, $results[0]);
    is_same('the intersection is 1', 1, $results[1]);
    is_true(
        'the similarity is between 0 and 1',
        is_float($results[2]) && $results[2] > 0.0 && $results[2] <= 1.0,
        var_export($results[2], true)
    );

    // Folding down loses precision, which is the trade it exists to make.
    $client->operate(null, $sketch, [HllOp::fold('visitors', 8)]);
    is_same_value(
        'fold reduces the index bits',
        [8, 0],
        $client->operate(null, $sketch, [HllOp::describe('visitors')])->bin('visitors')
    );
} catch (Aerospike\AerospikeException $e) {
    unsupported($e)
        ? skip('HyperLogLog operations', 'the daemon does not serve them yet: ' . $e->getMessage())
        : fail('HyperLogLog operations', describe($e));
}

// ===== the expression builder ===============================================

section('the expression builder');

/*
 * `Aerospike\Exp` and its five collection siblings build an expression tree that
 * the **client** packs. That is the difference from `Expression::ael()`, which is
 * text the server compiles and needs server 8.1.3: a built expression works
 * against every server this client supports.
 *
 * These assertions run the expression as a policy filter and ask whether the read
 * came through, which is the shortest way to make the server *evaluate* it. A
 * filter that matches lets the read through; one that does not is result code 27.
 */
$built = fresh_key('built-exp');
$client->put(new WritePolicy(sendKey: true), $built, [
    new Bin('age', 30),
    new Bin('name', '  Alice  '),
    new Bin('score', 950.5),
    new Bin('scores', [100, 950, 500]),
    new Bin('attrs', new SortedMap(['tier' => 'gold', 'level' => 7])),
    new Bin('nested', new SortedMap(['history' => [1, 2, 3, 4]])),
    new Bin('flags', new Blob("\x0F\x00")),
]);

/** Whether the server's evaluation of $exp let the read through. */
$passes = function ($exp) use ($client, $built): bool {
    try {
        return $client->get(new ReadPolicy(filterExp: $exp), $built) !== null;
    } catch (Aerospike\AerospikeException $e) {
        if ($e->getResultCode() === 27) {
            return false;
        }
        throw $e;
    }
};

// ----- the general surface -----
is_true('a built filter that matches lets the read through',
    $passes(Exp::gt(Exp::intBin('age'), Exp::intVal(21))));
is_true('and one that does not is filtered out',
    !$passes(Exp::gt(Exp::intBin('age'), Exp::intVal(99))));
is_true('and of several conditions', $passes(Exp::and([
    Exp::gt(Exp::intBin('age'), Exp::intVal(21)),
    Exp::eq(Exp::stringBin('name'), Exp::stringVal('  Alice  ')),
])));
is_true('or', $passes(Exp::or([
    Exp::eq(Exp::stringBin('name'), Exp::stringVal('Bob')),
    Exp::eq(Exp::intBin('age'), Exp::intVal(30)),
])));
is_true('not inverts', !$passes(Exp::not(Exp::eq(Exp::intBin('age'), Exp::intVal(30)))));
is_true('arithmetic evaluates on the server', $passes(Exp::eq(
    Exp::numAdd([Exp::intBin('age'), Exp::intVal(12)]), Exp::intVal(42))));
is_true('a float bin compares as a float',
    $passes(Exp::gt(Exp::floatBin('score'), Exp::floatVal(900.0))));
is_true('binExists', $passes(Exp::binExists('name')));
is_true('binExists on an absent bin', !$passes(Exp::binExists('nope')));
is_true('regexCompare honours its flags', $passes(
    Exp::regexCompare('^ *ali', RegexFlag::ICASE, Exp::stringBin('name'))));
is_true('record metadata reads', $passes(Exp::eq(Exp::setName(), Exp::stringVal($set))));
is_true('keyExists sees a key stored with sendKey', $passes(Exp::keyExists()));
is_true('cond picks a branch', $passes(Exp::eq(
    Exp::cond([
        Exp::ge(Exp::intBin('age'), Exp::intVal(65)), Exp::stringVal('senior'),
        Exp::ge(Exp::intBin('age'), Exp::intVal(18)), Exp::stringVal('adult'),
        Exp::stringVal('minor'),
    ]),
    Exp::stringVal('adult'))));
is_true('let binds a variable the body uses', $passes(Exp::let([
    Exp::def('a', Exp::intBin('age')),
    Exp::gt(Exp::var('a'), Exp::intVal(21)),
])));
is_true('inList compares against a list literal',
    $passes(Exp::inList(Exp::intBin('age'), Exp::listVal([10, 20, 30]))));
is_true('int bitwise', $passes(Exp::eq(
    Exp::intAnd([Exp::intBin('age'), Exp::intVal(0xFF)]), Exp::intVal(30))));

// ----- collections -----
is_true('a list size', $passes(Exp::eq(
    ExpList::size(Exp::listBin('scores')), Exp::intVal(3))));
is_true('a list element by index', $passes(Exp::eq(
    ExpList::getByIndex(ListReturn::Values, ExpType::Integer, Exp::intVal(1),
        Exp::listBin('scores')),
    Exp::intVal(950))));
is_true('a list count over a half-open range', $passes(Exp::eq(
    ExpList::getByValueRange(ListReturn::Count, Exp::intVal(400), null,
        Exp::listBin('scores')),
    Exp::intVal(2))));
is_true('an inverted return type selects the complement', $passes(Exp::eq(
    ExpList::getByValueRange(ListReturn::Count, Exp::intVal(400), null,
        Exp::listBin('scores'), true),
    Exp::intVal(1))));
is_true('a modify expression composes with a read', $passes(Exp::eq(
    ExpList::size(ExpList::append(null, Exp::intVal(1), Exp::listBin('scores'))),
    Exp::intVal(4))));
is_true('a map value by key', $passes(Exp::eq(
    ExpMap::getByKey(MapReturn::Value, ExpType::Text, Exp::stringVal('tier'),
        Exp::mapBin('attrs')),
    Exp::stringVal('gold'))));
is_true('a context path reaches a nested collection', $passes(Exp::eq(
    ExpList::size(Exp::mapBin('nested'))->context([Ctx::mapKey('history')]),
    Exp::intVal(4))));

// ----- blobs, sketches and strings -----
is_true('bit count over a range', $passes(Exp::eq(
    ExpBit::count(Exp::intVal(0), Exp::intVal(16), Exp::blobBin('flags')),
    Exp::intVal(4))));
is_true('bit getInt reads a field', $passes(Exp::eq(
    ExpBit::getInt(Exp::intVal(0), Exp::intVal(8), false, Exp::blobBin('flags')),
    Exp::intVal(15))));
// The string operations are compiled by the server and it learned them in
// 8.1.3, which answers an older cluster with INVALID_REQUEST rather than a
// filtered-out read — so this asks first instead of catching a fatal.
$stringOpsSupported = server_at_least($client, '8.1.3');
if (!$stringOpsSupported) {
    skip('string expressions', 'ExpStr needs server 8.1.3 or later');
} else {
    is_true('a string length', $passes(Exp::eq(
        ExpStr::strlen(Exp::stringBin('name')), Exp::intVal(9))));
    is_true('string operations chain', $passes(Exp::eq(
        ExpStr::strlen(ExpStr::trim(false, Exp::stringBin('name'))), Exp::intVal(5))));
    is_true('caseFold then startsWith', $passes(ExpStr::startsWith(
        Exp::stringVal('ali'),
        ExpStr::caseFold(false, ExpStr::trim(false, Exp::stringBin('name'))))));
}

// ----- path expressions, which fan out -----
/*
 * Everything above addresses one node. A path expression addresses every node its
 * path reaches, because the path contains a fan-out step. Needs server 8.1.1, so
 * this block self-skips against an older cluster rather than failing.
 */
// Asked here rather than reusing $nodes: the cluster-management section that
// populates it runs much later in this file, and a check that depends on
// statement order is a check that breaks when a section moves.
$pathsSupported = server_at_least($client, '8.1.1');
if (!$pathsSupported) {
    skip('path expressions', 'the fan-out over a collection needs server 8.1.1 or later');
} else {
    $books = fresh_key('path-exp');
    $client->put(null, $books, [new Bin('books', new SortedMap([
        'a' => new SortedMap(['price' => 10]),
        'b' => new SortedMap(['price' => 30]),
        'c' => new SortedMap(['price' => 50]),
    ]))]);
    $onBooks = function ($exp) use ($client, $books): bool {
        try {
            return $client->get(new ReadPolicy(filterExp: $exp), $books) !== null;
        } catch (Aerospike\AerospikeException $e) {
            if ($e->getResultCode() === 27) {
                return false;
            }
            throw $e;
        }
    };

    $prices = ExpPath::selectValues(ExpType::ListType, Exp::mapBin('books'),
        [Ctx::allChildren(), Ctx::mapKey('price')]);
    is_true('a fan-out reaches every child',
        $onBooks(Exp::eq(ExpList::size($prices), Exp::intVal(3))));
    is_true('selectByPath with the VALUE flag is the same operation',
        $onBooks(Exp::eq(ExpList::size(ExpPath::selectByPath(ExpType::ListType,
            SelectFlag::VALUE, Exp::mapBin('books'),
            [Ctx::allChildren(), Ctx::mapKey('price')])), Exp::intVal(3))));
    is_true('selectMapKeys collects keys rather than values',
        $onBooks(Exp::eq(ExpList::size(ExpPath::selectMapKeys(ExpType::ListType,
            Exp::mapBin('books'), [Ctx::allChildren()])), Exp::intVal(3))));

    // The filter runs per child, and the loop variable is the child being tested.
    $over = fn(int $limit) => ExpList::size(ExpPath::selectValues(
        ExpType::ListType, Exp::mapBin('books'), [
            Ctx::allChildrenWithFilter(Exp::gt(
                ExpMap::getByKey(MapReturn::Value, ExpType::Integer,
                    Exp::stringVal('price'),
                    Exp::loopVar(ExpType::MapType, LoopVarPart::Value)),
                Exp::intVal($limit))),
            Ctx::mapKey('price'),
        ]));
    is_true('a filtered fan-out visits only matching children',
        $onBooks(Exp::eq($over(20), Exp::intVal(2))));
    is_true('and a stricter filter selects fewer',
        $onBooks(Exp::eq($over(40), Exp::intVal(1))));
    is_true('a loop variable can read the child key',
        $onBooks(Exp::eq(ExpList::size(ExpPath::selectValues(ExpType::ListType,
            Exp::mapBin('books'), [
                Ctx::allChildrenWithFilter(Exp::eq(
                    Exp::loopVar(ExpType::Text, LoopVarPart::MapKey),
                    Exp::stringVal('b'))),
                Ctx::mapKey('price'),
            ])), Exp::intVal(1))));
    is_true('selectMatchingTree keeps the structure rather than flattening it',
        $onBooks(Exp::eq(ExpMap::size(ExpPath::selectMatchingTree(ExpType::MapType,
            Exp::mapBin('books'), [Ctx::allChildren(), Ctx::mapKey('price')])),
            Exp::intVal(3))));
    is_true('modify replaces every selected node',
        $onBooks(Exp::eq(ExpList::getByRank(ListReturn::Values, ExpType::Integer,
            Exp::intVal(-1),
            ExpPath::selectValues(ExpType::ListType,
                ExpPath::modify(ExpType::MapType, Exp::mapBin('books'), Exp::intVal(1),
                    [Ctx::allChildren(), Ctx::mapKey('price')]),
                [Ctx::allChildren(), Ctx::mapKey('price')])),
            Exp::intVal(1))));
    is_true('remove drops every selected node',
        $onBooks(Exp::eq(ExpList::size(ExpPath::selectValues(ExpType::ListType,
            ExpPath::remove(ExpType::MapType, Exp::mapBin('books'),
                [Ctx::allChildren(), Ctx::mapKey('price')]),
            [Ctx::allChildren(), Ctx::mapKey('price')])), Exp::intVal(0))));

    is_client_error(
        'a path expression with no path is refused',
        'fan-out is the point',
        fn() => ExpPath::selectValues(ExpType::ListType, Exp::mapBin('books'), [])
    );
    is_client_error(
        'a text filter cannot go inside a packed fan-out',
        'packed expression tree',
        fn() => Ctx::allChildrenWithFilter(Expression::ael('$.price:INT > 1'))
    );
    is_true('the path flags are the server\'s numbers',
        SelectFlag::VALUE === 1 && SelectFlag::MAP_KEY === 2
        && SelectFlag::NO_FAIL === 0x10 && ModifyFlag::DEFAULT === 0);

    $client->delete(null, $books);
}

// ----- what the builder refuses -----
is_client_error(
    'a text expression cannot be an operand of a built one',
    'cannot be combined',
    fn() => Exp::not(Expression::ael('$.age:INT > 1'))
);
is_client_error(
    'a packed expression cannot be an operand either',
    'already complete',
    fn() => Exp::not(Expression::base64('kQE='))
);
is_client_error(
    'cond without a default is refused',
    'default is missing',
    fn() => Exp::cond([Exp::boolVal(true), Exp::intVal(1)])
);
is_client_error(
    'let without a body is refused',
    'nothing uses them',
    fn() => Exp::let([Exp::def('a', Exp::intVal(1)), Exp::def('b', Exp::intVal(2))])
);
is_client_error(
    'digestModulo(0) is refused',
    'divide by zero',
    fn() => Exp::digestModulo(0)
);
is_client_error(
    'an empty regex is refused',
    'matches everything',
    fn() => Exp::regexCompare('', 0, Exp::stringBin('name'))
);
is_client_error(
    'context() on a non-collection expression is refused',
    'not one',
    fn() => Exp::intVal(1)->context([Ctx::mapKey('x')])
);
is_client_error(
    'an empty operand list is refused',
    'at least one expression',
    fn() => Exp::and([])
);
is_type_error(
    'an operand list must hold expressions',
    fn() => Exp::and([Exp::intVal(1), 'nope'])
);
is_true('a built expression reports itself as built',
    Exp::intVal(1)->isBuilt() && !Expression::ael('$.a:INT')->isBuilt());
is_true('and has no source text, because it never was text',
    Exp::intVal(1)->source() === '');

$client->delete(null, $built);

// ===== expression operations ================================================

section('expression operations');

$exprs = fresh_key('expressions');

try {
    $client->put(null, $exprs, [
        new Bin('price', 10),
        new Bin('quantity', 3),
        new Bin('name', 'widget'),
    ]);

    // A computed bin, and a computed value that is only read — both evaluated
    // by the server, inside the same atomic operate() as everything else.
    $record = $client->operate(null, $exprs, [
        ExpOp::write('total', Expression::ael('$.price:INT * $.quantity:INT')),
        ExpOp::read('doubled', Expression::ael('$.total:INT * 2')),
    ]);
    is_same('an expression read returns its result under the name given', 60, $record->bin('doubled'));
    is_same('an expression write lands in the bin', 30, $client->get(null, $exprs)->bin('total'));

    // The lesson this family teaches: an expression operation has no comparison
    // to infer a bin's type from, so a bare path is a parameter error and an
    // annotated one is not. `$.price > 5` in a filter needs no annotation
    // because the `> 5` says what the bin is.
    is_same(
        'an annotated path reads the bin',
        10,
        $client->operate(null, $exprs, [ExpOp::read('p', Expression::ael('$.price:INT'))])->bin('p')
    );
    try {
        $client->operate(null, $exprs, [ExpOp::read('p', Expression::ael('$.price'))]);
        fail('an unannotated path in a read expression is refused', 'it was accepted');
    } catch (Aerospike\AerospikeException $e) {
        is_same(
            'an unannotated path in a read expression is a parameter error (4)',
            4,
            $e->getResultCode()
        );
    }

    // Strings work the same way, with their own annotation.
    is_same(
        'a string bin needs :STRING',
        'widget',
        $client->operate(null, $exprs, [ExpOp::read('n', Expression::ael('$.name:STRING'))])->bin('n')
    );

    // A constant expression needs nothing to infer from at all.
    is_same(
        'a constant expression needs no annotation',
        1,
        $client->operate(null, $exprs, [ExpOp::read('one', Expression::ael('1'))])->bin('one')
    );

    // CreateOnly refuses a bin that is already there.
    try {
        $client->operate(null, $exprs, [
            ExpOp::write(
                'total',
                Expression::ael('$.price:INT'),
                writeMode: BinWriteMode::CreateOnly
            ),
        ]);
        fail('an expression write honours CreateOnly', 'it was accepted');
    } catch (Aerospike\AerospikeException $e) {
        is_true(
            'an expression write honours CreateOnly',
            $e->getResultCode() !== null,
            describe($e)
        );
    }
} catch (Aerospike\AerospikeException $e) {
    // AEL needs server 8.1.3; the daemon refuses an older cluster by name.
    unsupported($e) || str_contains($e->getMessage(), '8.1.3')
        ? skip('expression operations', $e->getMessage())
        : fail('expression operations', describe($e));
}

// The two ways to name an expression are kept apart, and the extension refuses
// an empty one before anything is sent.
is_same('Expression::ael() is AEL', true, Expression::ael('$.a:INT')->isAel());
is_same('Expression::base64() is not', false, Expression::base64('kQE=')->isAel());
is_same('an expression keeps its source', '$.a:INT', Expression::ael('$.a:INT')->source());
is_client_error('an empty expression is refused', 'needs a source', fn() => Expression::ael('  '));
is_client_error(
    'an empty packed expression is refused',
    'needs bytes',
    fn() => Expression::base64('')
);

// Passing the source text where packed bytes belong is the likely mistake, and
// the daemon says so rather than letting the server reject it obscurely.
try {
    $client->operate(null, $exprs, [
        ExpOp::read('x', Expression::base64('$.price:INT')),
    ]);
    fail('source text passed as base64 is refused', 'it was accepted');
} catch (Aerospike\AerospikeException $e) {
    is_true(
        'source text passed as base64 is refused, saying what was expected',
        str_contains($e->getMessage(), 'source text'),
        describe($e)
    );
}

// ===== batch ================================================================

section('batch');

$row = fn(string $name) => new Key($namespace, $set, "smoke-batch-" . getmypid() . "-$name");

try {
    $client->put(null, $row('a'), [new Bin('name', 'Alice'), new Bin('hits', 1)]);
    $client->put(null, $row('b'), [new Bin('name', 'Bob'), new Bin('items', [1, 2, 3])]);
    $client->put(null, $row('d'), [new Bin('x', 1)]);

    // Five kinds of work in one call, against five keys.
    $results = $client->batch(null, [
        BatchRead::all($row('a')),
        BatchRead::some($row('b'), ['name']),
        BatchRead::ops($row('b'), [ListOp::size('items')]),
        BatchWrite::ops($row('a'), [Op::add(new Bin('hits', 10))]),
        BatchDelete::key($row('d')),
        BatchRead::all($row('missing')),
    ]);

    is_same('batch answers one row per request row, in order', 6, count($results));
    is_true('every answer is a BatchResult', $results[0] instanceof Aerospike\BatchResult);

    is_same_value('a full read row reads every bin', ['name' => 'Alice', 'hits' => 1], $results[0]->record()->bins());
    is_same_value('a selective read row reads what it asked for', ['name' => 'Bob'], $results[1]->record()->bins());
    is_same('a read-ops row can run collection reads', 3, $results[2]->record()->bin('items'));

    // The write and the delete really happened.
    is_same('a write row applied', 11, $client->get(null, $row('a'))->bin('hits'));
    is_same('a delete row applied', false, $client->exists(null, $row('d')));

    // The surprise worth knowing: in a batch, a record that is not there is a
    // *row failure* with result code 2 — not the null that get() returns. The
    // batch itself still succeeded.
    is_same('a missing record is a failed row, not a failed batch', false, $results[5]->isOk());
    is_same('a missing record in a batch is result code 2', 2, $results[5]->resultCode());
    is_same('a failed row carries no record', null, $results[5]->record());
    is_same('a read row is never in doubt', false, $results[5]->isInDoubt());

    // …and the rows around it are unaffected, which is the whole point.
    is_true('the other rows succeeded anyway', $results[0]->isOk() && $results[4]->isOk());
} catch (Aerospike\AerospikeException $e) {
    unsupported($e)
        ? skip('batch', 'the daemon does not serve BATCH yet: ' . $e->getMessage())
        : fail('batch', describe($e));
}

// Per-row policies, including a filter that rejects one row and leaves the rest.
try {
    $client->put(null, $row('p'), [new Bin('n', 1)]);
    $client->put(null, $row('q'), [new Bin('n', 99)]);

    $results = $client->batch(null, [
        BatchRead::all($row('p'), filter: '$.n:INT == 1'),
        BatchRead::all($row('q'), filter: '$.n:INT == 1'),
    ]);
    is_same('a row its filter accepts succeeds', true, $results[0]->isOk());
    is_same('a row its filter rejects reports 27', 27, $results[1]->resultCode());

    // A write row's own policy: CreateOnly on a record that exists.
    $results = $client->batch(null, [
        BatchWrite::ops(
            $row('p'),
            [Op::put(new Bin('n', 2))],
            recordExistsAction: RecordExistsAction::CreateOnly
        ),
        BatchRead::all($row('p')),
    ]);
    is_same('a per-row CreateOnly is enforced', 5, $results[0]->resultCode());
    is_same('and the read row still ran', 1, $results[1]->record()->bin('n'));
} catch (Aerospike\AerospikeException $e) {
    str_contains($e->getMessage(), '8.1.3')
        ? skip('per-row batch filters', $e->getMessage())
        : fail('per-row batch policies', describe($e));
}

// Header-only rows: "which of these keys exist", in one round trip.
try {
    $results = $client->batch(null, [
        BatchRead::header($row('a')),
        BatchRead::header($row('missing')),
    ]);
    is_same_value('a header row reads no bins', [], $results[0]->record()->bins());
    is_true('a header row still reports a generation', $results[0]->record()->generation() >= 1);
    is_same('a header row for a missing key fails with 2', 2, $results[1]->resultCode());
} catch (Aerospike\AerospikeException $e) {
    fail('header-only batch rows', describe($e));
}

// What a batch row refuses before anything is sent.
is_true('a batch row knows whether it writes', BatchWrite::ops($row('a'), [Op::touch()])->isWrite()
    && !BatchRead::all($row('a'))->isWrite());
is_same('a batch row reports its key', (string) $row('a'), (string) BatchRead::all($row('a'))->key());
is_client_error(
    'a write in a read row is refused, naming the alternative',
    'BatchWrite::ops()',
    fn() => BatchRead::ops($row('a'), [Op::touch()])
);
is_client_error(
    'an empty batch is refused',
    'at least one row',
    fn() => $client->batch(null, [])
);
is_client_error(
    'a write row with no operations is refused',
    'at least one operation',
    fn() => BatchWrite::ops($row('a'), [])
);
is_type_error(
    'batch rows cannot be a list of something else',
    fn() => $client->batch(null, [BatchRead::all($row('a')), 'nope'])
);
is_type_error(
    'a batch row needs a Key, not a string',
    fn() => BatchRead::all('test:batch:a')
);

// ===== What the types refuse before anything is sent ========================

section('type safety');

// Each of these was a *runtime* error under the associative-array policy: a
// misspelled key, a bad enum name, a string where a bool belonged. They are now
// refused by the engine, at the call, which is the whole point of the object
// API.
// A misspelled field is the mistake the associative-array policy could only
// catch at runtime, by comparing against a list of names. Now PHP catches it
// itself: an unknown named argument is not a thing a caller can pass.
is_rejected(
    'a misspelled policy field is not a named argument',
    'Unknown named parameter $durableDeletes',
    // @phpstan-ignore-next-line — the point is that this does not exist
    fn() => new WritePolicy(durableDeletes: true)
);
is_type_error(
    'an enum cannot be given a string',
    fn() => new WritePolicy(replica: 'MASTER')
);
is_type_error(
    'an enum cannot be given an int',
    fn() => new ReadPolicy(readModeSc: 2)
);
is_type_error(
    'a bool field cannot be given a string',
    fn() => new WritePolicy(durableDelete: 'false')
);
is_type_error(
    'an int field cannot be given a float',
    fn() => new ReadPolicy(totalTimeoutMs: 500.5)
);
is_type_error(
    'an expiration cannot be given a bare int',
    fn() => new WritePolicy(expiration: 3600)
);
is_type_error(
    'a read policy cannot be passed to a write',
    fn() => $client->put(new ReadPolicy(), $guarded, [new Bin('x', 1)])
);
is_type_error(
    'a write policy cannot be passed to a read',
    fn() => $client->get(new WritePolicy(), $guarded)
);
is_type_error(
    'a key cannot be a bare string',
    fn() => $client->get(null, 'not-a-key')
);
is_type_error(
    'bins cannot be a name => value array',
    fn() => $client->put(null, $guarded, ['x' => 1])
);
is_type_error(
    'a bin selector cannot be an array of names',
    fn() => $client->get(null, $guarded, ['x'])
);
is_type_error(
    'the policy argument is not optional, only nullable',
    fn() => $client->exists($guarded)
);

// And the guards the type system cannot express, which stay runtime errors with
// a message that says what to write instead.
is_client_error(
    'a negative expiration is refused, naming the alternatives',
    'never()',
    fn() => Expiration::seconds(-1)
);
is_client_error(
    'a negative timeout is refused',
    'totalTimeoutMs',
    fn() => new ReadPolicy(totalTimeoutMs: -1)
);
is_client_error(
    'an empty filter is refused',
    'filters nothing',
    fn() => new ReadPolicy(filter: '   ')
);
is_client_error(
    'a generation without a guard is refused',
    'generationPolicy',
    fn() => new WritePolicy(generation: 4)
);
is_client_error(
    'a guard without a generation is refused',
    'generation:',
    fn() => new WritePolicy(generationPolicy: GenerationPolicy::ExpectGenEqual)
);
is_client_error(
    'a write with no bins is refused',
    'at least one',
    fn() => $client->put(null, $guarded, [])
);

// The same guarantees on the operation surface.
is_type_error(
    'ops cannot be a list of something else',
    fn() => $client->operate(null, $ops, [Op::get(), 'nope'])
);
is_type_error(
    'a list policy cannot be given an int',
    fn() => ListOp::append('items', 1, 5)
);
is_type_error(
    'a return type cannot be given a string',
    fn() => ListOp::getByIndex('items', 0, 'VALUES')
);
is_type_error(
    'a context step cannot be a bare string',
    fn() => ListOp::size('items')->context(['roles'])
);
// The two families' policies and return types are separate types, so one
// cannot be used where the other belongs — the mistake a shared "CDT policy"
// would have allowed.
is_type_error(
    'a list policy cannot be passed to a map operation',
    fn() => MapOp::put('m', 'k', 1, new ListPolicy())
);
is_type_error(
    'a map policy cannot be passed to a list operation',
    fn() => ListOp::append('items', 1, new MapPolicy())
);
is_type_error(
    'a list return type cannot be passed to a map operation',
    fn() => MapOp::getByKey('m', 'k', ListReturn::Values)
);
is_client_error(
    'operate with no operations is refused',
    'at least one operation',
    fn() => $client->operate(null, $ops, [])
);
is_client_error(
    'a context path cannot be attached to a scalar operation',
    'collection operation',
    fn() => Op::get()->context([Ctx::mapKey('roles')])
);
is_client_error(
    'an empty context path is refused',
    'at least one step',
    fn() => ListOp::size('items')->context([])
);

// ===== Scans and queries ====================================================

echo "\nscan and query\n";

/**
 * A set of this section's own, seeded with records that carry their user keys.
 *
 * Its own set because a scan sees everything in one, and the rest of this file
 * writes to the shared set. `sendKey` because without it the server stores only
 * a digest, and a scanned record then has no key to report — which is worth
 * demonstrating rather than working around.
 */
$scanSet   = $set . 'scan';
$scanCount = 25;
$scanKeys  = [];
for ($i = 0; $i < $scanCount; $i++) {
    $scanKeys[$i] = new Key($namespace, $scanSet, $i);
    $client->put(
        new WritePolicy(sendKey: true),
        $scanKeys[$i],
        [new Bin('n', $i), new Bin('name', "user$i")]
    );
}

$statement = new Statement($namespace, $scanSet);
is_true('a statement with no filter is a scan', $statement->isScan());
is_same('a statement reports its namespace', $namespace, $statement->namespace());
is_same('a statement reports its set', $scanSet, $statement->setName());

// A page of four cannot hold twenty-five records, so this exercises the paging
// that `foreach` is meant to hide.
$seen    = [];
$digests = [];
$keys    = [];
foreach ($client->query(new QueryPolicy(pageSize: 4), null, $statement) as $position => $record) {
    $seen[]    = $record->bin('n');
    $digests[] = bin2hex((string) $record->digest());
    $keys[]    = $record->key()?->userKey();
    if (count($seen) === 1) {
        is_same('a scan counts positions from zero', 0, $position);
    }
}
sort($seen);
is_same('a paged scan returns every record', range(0, $scanCount - 1), $seen);
is_same(
    'a paged scan returns each record once',
    $scanCount,
    count(array_unique($digests))
);
sort($keys);
is_same('records written with sendKey come back with their keys', range(0, $scanCount - 1), $keys);
is_true(
    'a scanned record has a 20-byte digest',
    strlen((string) $client->query(null, null, $statement)->current()->digest()) === 20
);

// A single-record read has neither: the caller already has the key it asked
// with, and the server does not send the digest back.
$one = $client->get(null, $scanKeys[0]);
is_true('a single-record read has no key on the record', $one->key() === null);
is_true('a single-record read has no digest on the record', $one->digest() === null);

$narrow = new Statement($namespace, $scanSet, Bins::some(['n']));
$binNames = [];
foreach ($client->query(new QueryPolicy(pageSize: 10), null, $narrow) as $record) {
    $binNames[] = $record->binNames();
}
is_same('a scan can select bins', array_fill(0, $scanCount, ['n']), $binNames);

$capped = $client->query(new QueryPolicy(maxRecords: 5, pageSize: 100), null, $statement);
is_same('maxRecords stops a scan early', 5, count(iterator_to_array($capped)));
is_true('a scan that hit its ceiling holds no cursor', !$capped->isOpen());

$metaOnly = $client->query(new QueryPolicy(includeBinData: false), null, $statement);
$empty = 0;
$identified = 0;
foreach ($metaOnly as $record) {
    if ($record->count() === 0) {
        $empty++;
    }
    if (strlen((string) $record->digest()) === 20 && $record->generation() >= 1) {
        $identified++;
    }
}
is_same('includeBinData: false returns no bins', $scanCount, $empty);
is_same('includeBinData: false still returns identity and metadata', $scanCount, $identified);

// One scan divided four ways by partition range: together they cover the ring
// exactly once, which is how a scan is parallelised across workers.
$quarters = [];
foreach ([0, 1024, 2048, 3072] as $begin) {
    foreach ($client->query(
        new QueryPolicy(pageSize: 100, includeBinData: false),
        PartitionFilter::byRange($begin, 1024),
        $statement
    ) as $record) {
        $quarters[] = bin2hex((string) $record->digest());
    }
}
is_same('four partition ranges cover the ring once', $scanCount, count($quarters));
is_same('no two partition ranges return the same record', $scanCount, count(array_unique($quarters)));
is_true('PartitionFilter::all() covers everything', PartitionFilter::all()->isAll());
is_true('a partition range does not', !PartitionFilter::byRange(0, 1)->isAll());

// A traversal is a position, not a collection: reading it twice cannot start
// again, and saying so is better than silently continuing from the middle.
$once = $client->query(new QueryPolicy(pageSize: 100), null, $statement);
foreach ($once as $record) {
    // Consume it.
}
is_client_error(
    'a RecordSet cannot be iterated twice',
    'only be iterated once',
    function () use ($once) {
        foreach ($once as $record) {
        }
    }
);

// Abandoning a scan releases the daemon's cursor rather than leaving it to
// expire, and `close()` is idempotent.
$abandoned = $client->query(new QueryPolicy(pageSize: 1), null, $statement);
$read = 0;
foreach ($abandoned as $record) {
    if (++$read === 2) {
        break;
    }
}
is_true('an unfinished scan holds a cursor', $abandoned->isOpen());
is_same('a scan reports how many records it has produced', 2, $abandoned->seen());
$abandoned->close();
is_true('closing a scan releases its cursor', !$abandoned->isOpen());
$abandoned->close();
is_true('closing a closed scan is not an error', !$abandoned->isOpen());
is_true('a closed scan has nothing at its position', !$abandoned->valid());

// The central risk of a daemon-held cursor: one that nothing closes. Dropping
// the object has to release it, or a request that abandons scans leaks them
// until they expire.
//
// Decisive rather than indicative: more traversals than the daemon's default
// ceiling of 1024 open cursors. If dropping did not release them, the 1025th
// would be refused. It costs a couple of seconds, which is what proving this
// is worth.
$abandonedCount = 1100;
$opened = 0;
try {
    for ($i = 0; $i < $abandonedCount; $i++) {
        $leaky = $client->query(new QueryPolicy(pageSize: 1), null, $statement);
        $leaky->current();
        unset($leaky);          // no close(): the destructor has to do it
        $opened++;
    }
    ok("dropping a RecordSet releases its cursor ($opened abandoned)");
} catch (Aerospike\AerospikeException $e) {
    fail(
        'dropping a RecordSet releases its cursor',
        "the daemon ran out of cursors after $opened: " . describe($e)
    );
}

// A query needs an index. There is no way to create one yet, so what is
// asserted is that the *absence* is reported as the server's own error rather
// than silently degrading into a full scan.
$indexed = new Statement($namespace, $scanSet, filter: Filter::range('n', 0, 10));
is_true('a query on an unindexed bin is refused', !$indexed->isScan());
try {
    iterator_to_array($client->query(null, null, $indexed));
    fail('a query without an index reports the server error', 'the query succeeded');
} catch (Aerospike\AerospikeException $e) {
    // 201 is INDEX_NOTFOUND. Anything else — an empty result most of all —
    // would mean a query had quietly become a scan.
    is_same('a query without an index reports the server error', 201, $e->getResultCode());
}

// Filters build, and the three independent choices are all reachable.
is_true('a filter names its bin', Filter::equal('n', 1)->target() === 'n');
is_true('a filter can name an index instead', Filter::equalByIndex('n_idx', 1)->isByIndex());
is_true('a bin filter is not an index filter', !Filter::equal('n', 1)->isByIndex());
is_true(
    'a filter can cover list elements',
    Filter::equal('tags', 'urgent', CollectionIndex::ListElements)->target() === 'tags'
);
is_same(
    'a filter carries a context path',
    1,
    Filter::equal('m', 1, CollectionIndex::MapValues)->context([Ctx::mapKey('inner')])->contextDepth()
);
is_true(
    'a filter can name an expression index',
    Filter::range('c', 0, 10)->expression(Expression::ael('$.a + $.b'))->target() === 'c'
);
is_true('a geo region filter builds', Filter::geoWithinRegion('loc', '{"type":"Polygon"}')->target() === 'loc');
is_true('a geo radius filter builds', Filter::geoWithinRadius('loc', -122.4, 37.8, 1000.0)->target() === 'loc');
is_true('a geo contains filter builds', Filter::geoContains('region', '{"type":"Point"}')->target() === 'region');

foreach ($scanKeys as $key) {
    $client->delete(null, $key);
}

// ----- what a scan or query refuses -----------------------------------------

echo "\nscan and query type safety\n";

is_type_error(
    'query needs a Statement',
    fn() => $client->query(null, null, 'test')
);
is_type_error(
    'query will not take a ReadPolicy',
    fn() => $client->query(new ReadPolicy(), null, $statement)
);
is_type_error(
    'query will not take a Key as a partition filter',
    fn() => $client->query(null, $scanKeys[0], $statement)
);
is_type_error(
    'a Statement will not take a Filter as its bins',
    fn() => new Statement($namespace, $scanSet, Filter::equal('n', 1))
);
is_type_error(
    'a Statement will not take Bins as its filter',
    fn() => new Statement($namespace, $scanSet, null, Bins::all())
);
is_type_error(
    'a collection index is an enum, not a string',
    fn() => Filter::equal('n', 1, 'LIST')
);
is_client_error(
    'a Statement needs a namespace',
    'needs a namespace',
    fn() => new Statement('  ', $scanSet)
);
is_client_error(
    'a filter needs a bin name',
    'needs a bin name',
    fn() => Filter::equal('', 1)
);
is_client_error(
    'a backwards range is refused',
    '30 > 20',
    fn() => Filter::range('n', 30, 20)
);
is_client_error(
    'a filter cannot compare a float',
    'a float',
    fn() => Filter::equal('n', 1.5)
);
is_client_error(
    'a filter cannot compare an array',
    'a list',
    fn() => Filter::equal('n', [1, 2])
);
is_client_error(
    'latitude and longitude are checked, and their order stated',
    'longitude first',
    fn() => Filter::geoWithinRadius('loc', 37.8, -122.4, 1000.0)
);
is_client_error(
    'a radius must be positive',
    'metres',
    fn() => Filter::geoWithinRadius('loc', 0.0, 0.0, 0.0)
);
is_client_error(
    'a blank GeoJSON region is refused',
    'GeoJSON region',
    fn() => Filter::geoWithinRegion('loc', '   ')
);
is_client_error(
    'a negative partition id is refused',
    '4096',
    fn() => PartitionFilter::byId(-1)
);
is_client_error(
    'a partition id past the ring is refused',
    '4096',
    fn() => PartitionFilter::byId(4097)
);
is_client_error(
    'an empty partition range is refused',
    'at least one partition',
    fn() => PartitionFilter::byRange(0, 0)
);
is_client_error(
    'an empty filter context is refused',
    'at least one step',
    fn() => Filter::equal('n', 1)->context([])
);
is_client_error(
    'a negative maxRecords is refused',
    'must not be negative',
    fn() => new QueryPolicy(maxRecords: -1)
);
is_client_error(
    'a negative pageSize is refused',
    'pageSize',
    fn() => new QueryPolicy(pageSize: -1)
);
is_rejected(
    'a RecordSet cannot be constructed by hand',
    'cannot instantiate',
    fn() => new RecordSet()
);

// ===== Cluster management ===================================================

echo "\nnodes and info\n";

$nodes = $client->nodes();
is_true('the cluster has at least one node', count($nodes) >= 1);
$first = $nodes[0];
is_true('a node has a name', $first->name() !== '');
is_true('a node has an address', str_contains($first->address(), ':'));
is_true(
    'a node reports a four-part version',
    (bool) preg_match('/^\d+\.\d+\.\d+\.\d+$/', $first->version()),
    'got ' . $first->version()
);
is_true('a node the daemon routes to is active', $first->isActive());
is_true('a node stringifies as name@address', (string) $first === $first->name() . '@' . $first->address());

$info = $client->info(null, ['build', 'namespaces']);
is_same('info answers every command it was asked', ['build', 'namespaces'], array_keys($info));
is_true('info reports the server build', $info['build'] !== '', 'got ' . var_export($info['build'], true));
is_true(
    'info reports the namespace under test',
    in_array($namespace, explode(';', $info['namespaces']), true),
    'namespaces = ' . $info['namespaces']
);

// Asking a named node is what the per-node commands need; asking for `build`
// there too keeps the assertion about the plumbing rather than the content.
$fromNode = $client->info(null, ['build'], $first->name());
is_same('info can be aimed at one node', $info['build'], $fromNode['build']);

is_true(
    'an AdminPolicy carries only a timeout',
    (new AdminPolicy(timeoutMs: 5000))->timeoutMs() === 5000
);
is_true('an omitted AdminPolicy timeout is null', (new AdminPolicy())->timeoutMs() === null);

echo "\nsecondary indexes\n";

$indexSet  = $set . 'idx';
$indexName = 'php_age_idx';
$indexKeys = [];
for ($i = 0; $i < 10; $i++) {
    $indexKeys[$i] = new Key($namespace, $indexSet, $i);
    $client->put(null, $indexKeys[$i], [new Bin('age', 20 + $i), new Bin('name', "user$i")]);
}

// Drop first, in case a previous run left it behind: creating an index that
// already exists is an error, and a leftover index would make the run depend on
// how the last one ended.
try {
    $client->dropIndex(null, $namespace, $indexSet, $indexName)->waitTillComplete(5000, 200);
} catch (Aerospike\AerospikeException $e) {
    // Nothing to drop is the ordinary case.
}

$task = $client->createIndexOnBin(
    null, $namespace, $indexSet, 'age', $indexName, IndexType::Numeric
);
is_true('creating an index returns a task', $task instanceof Task);
is_same('a task describes the work it names', "creating index $namespace.$indexName", $task->describe());
try {
    $status = $task->waitTillComplete(15_000, 200);
    is_same('an index build completes', TaskStatus::Complete, $status);
    is_true('a completed task reports itself complete', $task->isComplete());
} catch (Aerospike\AerospikeException $e) {
    fail('an index build completes', describe($e));
}

// The point of the index: the query that was a server error in the scan section
// now runs.
$indexed = new Statement($namespace, $indexSet, filter: Filter::range('age', 22, 25));
$matched = [];
foreach ($client->query(null, null, $indexed) as $record) {
    $matched[] = $record->bin('age');
}
sort($matched);
is_same('a range query uses the index it was given', [22, 23, 24, 25], $matched);

$equal = new Statement($namespace, $indexSet, filter: Filter::equal('age', 27));
is_same('an equality query uses the index', 1, count(iterator_to_array($client->query(null, null, $equal))));

// A query naming the index rather than the bin has to reach the same index.
$byIndex = new Statement($namespace, $indexSet, filter: Filter::rangeByIndex($indexName, 22, 25));
is_same(
    'a query can name the index instead of the bin',
    4,
    count(iterator_to_array($client->query(null, null, $byIndex)))
);

$dropped = $client->dropIndex(null, $namespace, $indexSet, $indexName);
is_same('dropping an index describes itself as dropping', "dropping index $namespace.$indexName", $dropped->describe());
try {
    is_same('an index drop completes', TaskStatus::Complete, $dropped->waitTillComplete(10_000, 200));
} catch (Aerospike\AerospikeException $e) {
    fail('an index drop completes', describe($e));
}

echo "\nuser-defined functions\n";

$udfSource = file_get_contents(__DIR__ . '/udf/php_example.lua');
is_true('the test UDF module is readable', $udfSource !== false && $udfSource !== '');

$registered = $client->registerUdf(null, $udfSource, 'php_example.lua');
is_same(
    'registering a UDF describes itself',
    'registering UDF php_example.lua',
    $registered->describe()
);
try {
    is_same('a UDF registration completes', TaskStatus::Complete, $registered->waitTillComplete(10_000, 200));
} catch (Aerospike\AerospikeException $e) {
    fail('a UDF registration completes', describe($e));
}

$modules = $client->listUdf(null);
$names = array_map(fn($m) => $m->name(), $modules);
is_true('the registered module is listed', in_array('php_example.lua', $names, true), implode(', ', $names));
$mine = null;
foreach ($modules as $module) {
    if ($module->name() === 'php_example.lua') {
        $mine = $module;
    }
}
is_true('a listed module has a hash', $mine !== null && $mine->hash() !== '');
is_same('a listed module reports its language', 'LUA', $mine?->language());

// The package name drops the extension in a call, but not in a registration.
// That asymmetry is the server's, and getting it wrong is a rc-1300 error.
$udfKey = new Key($namespace, $set, 'udf-target');
$client->delete(null, $udfKey);

is_true(
    'a UDF that returns nothing gives null',
    $client->executeUdf(null, $udfKey, 'php_example', 'writeBin', ['greeting', 'hello']) === null
);
is_same('a UDF wrote the bin it was asked to', 'hello', $client->get(null, $udfKey)->bin('greeting'));
is_same(
    'a UDF returns what it read',
    'hello',
    $client->executeUdf(null, $udfKey, 'php_example', 'readBin', ['greeting'])
);

// Arguments and return values cross the daemon unchanged, for every shape a
// value can be.
foreach ([
    'an int'    => 42,
    'a float'   => 1.5,
    'a string'  => 'text',
    'a bool'    => true,
    'a list'    => [1, 2, 3],
    'a map'     => ['a' => 1],
] as $what => $value) {
    is_same_value(
        "a UDF echoes $what",
        $value,
        $client->executeUdf(null, $udfKey, 'php_example', 'echo', [$value])
    );
}
is_true(
    'a UDF called with no arguments works',
    $client->executeUdf(null, $udfKey, 'php_example', 'readBin') === null
);

// A UDF's own error is a server failure, not a success with a null.
try {
    $client->executeUdf(null, $udfKey, 'php_example', 'refuse');
    fail('a failing UDF throws', 'the call succeeded');
} catch (Aerospike\AerospikeException $e) {
    is_true(
        'a failing UDF throws',
        $e->getStatus() === Status::Server,
        describe($e)
    );
}

// A background query applies the function to a whole set without moving any
// record to the client.
$bgSet  = $set . 'bg';
$bgKeys = [];
for ($i = 0; $i < 10; $i++) {
    $bgKeys[$i] = new Key($namespace, $bgSet, $i);
    $client->put(null, $bgKeys[$i], [new Bin('hits', 1)]);
}
$background = $client->queryExecuteUdf(
    null,
    new Statement($namespace, $bgSet),
    'php_example',
    'incrementBin',
    ['hits', 10]
);
is_true('a background query returns a task', str_contains($background->describe(), 'background scan'));
try {
    is_same('a background query completes', TaskStatus::Complete, $background->waitTillComplete(15_000, 200));
    $bumped = 0;
    foreach ($bgKeys as $key) {
        if ($client->get(null, $key)?->bin('hits') === 11) {
            $bumped++;
        }
    }
    is_same('a background query touched every record', 10, $bumped);
} catch (Aerospike\AerospikeException $e) {
    fail('a background query completes', describe($e));
}

$removed = $client->removeUdf(null, 'php_example.lua');
is_same('removing a UDF describes itself', 'removing UDF php_example.lua', $removed->describe());
try {
    is_same('a UDF removal completes', TaskStatus::Complete, $removed->waitTillComplete(10_000, 200));
} catch (Aerospike\AerospikeException $e) {
    fail('a UDF removal completes', describe($e));
}
is_true(
    'a removed module is no longer listed',
    !in_array('php_example.lua', array_map(fn($m) => $m->name(), $client->listUdf(null)), true)
);

echo "\ntruncate\n";

$truncSet = $set . 'trunc';
for ($i = 0; $i < 5; $i++) {
    $client->put(null, new Key($namespace, $truncSet, $i), [new Bin('n', $i)]);
}
$before = new Statement($namespace, $truncSet);
is_same('the set to truncate has records', 5, count(iterator_to_array($client->query(null, null, $before))));

$client->truncate(null, $namespace, $truncSet, null);
// Truncation is immediate for reads even though the space is reclaimed in the
// background, so a scan straight afterwards must find nothing.
is_same(
    'truncate empties the set',
    0,
    count(iterator_to_array($client->query(null, null, new Statement($namespace, $truncSet))))
);
is_true(
    'a truncated record is gone',
    $client->get(null, new Key($namespace, $truncSet, 0)) === null
);

// The cutoff is asserted the deterministic way round: a cutoff *before* the
// records were written must leave every one of them.
//
// Not the other way round, because a cutoff of "now" is a race with the server's
// own clock — the server refuses a cutoff ahead of its clock, and a client
// computing one microseconds later can land there. A past cutoff needs no clock
// agreement and proves the same thing: the argument is applied rather than
// ignored.
$longAgo = (int) ((time() - 3600) * 1_000_000_000);
for ($i = 0; $i < 3; $i++) {
    $client->put(null, new Key($namespace, $truncSet, $i), [new Bin('n', $i)]);
}
$client->truncate(null, $namespace, $truncSet, $longAgo);
is_same(
    'truncate honours a cutoff, leaving newer records alone',
    3,
    count(iterator_to_array($client->query(null, null, new Statement($namespace, $truncSet))))
);
$client->truncate(null, $namespace, $truncSet, null);
is_same(
    'truncate with no cutoff takes the rest',
    0,
    count(iterator_to_array($client->query(null, null, new Statement($namespace, $truncSet))))
);

foreach ($indexKeys as $key) {
    $client->delete(null, $key);
}
foreach ($bgKeys as $key) {
    $client->delete(null, $key);
}
$client->delete(null, $udfKey);

// ----- what the management commands refuse -----------------------------------

echo "\nmanagement type safety\n";

is_type_error(
    'registerUdf needs a string source',
    fn() => $client->registerUdf(null, [], 'x.lua')
);
is_type_error(
    'registerUdf will not take a WritePolicy',
    fn() => $client->registerUdf(new WritePolicy(), 'x', 'x.lua')
);
is_type_error(
    'createIndexOnBin needs an IndexType',
    fn() => $client->createIndexOnBin(null, $namespace, $indexSet, 'age', 'i', 'NUMERIC')
);
is_type_error(
    'createIndexOnBin will not take a ReadPolicy',
    fn() => $client->createIndexOnBin(new ReadPolicy(), $namespace, $indexSet, 'age', 'i', IndexType::Numeric)
);
is_type_error(
    'createIndexUsingExpression needs an Expression',
    fn() => $client->createIndexUsingExpression(
        null, $namespace, $indexSet, 'i', IndexType::Numeric, null, '$.a'
    )
);
is_type_error(
    'info needs an array of commands',
    fn() => $client->info(null, 'build')
);
// The argument is checked before any round trip, which is why an
// already-finished task is a fine subject for it.
is_type_error(
    'a task timeout is an int, not a string',
    fn() => $removed->waitTillComplete('soon')
);
is_type_error(
    'a task poll interval is an int, not an enum',
    fn() => $removed->waitTillComplete(1000, TaskStatus::Complete)
);
is_client_error(
    'info with no commands is refused',
    'at least one command',
    fn() => $client->info(null, [])
);
is_client_error(
    'a negative truncate cutoff is refused',
    'must not be negative',
    fn() => $client->truncate(null, $namespace, $truncSet, -1)
);
is_rejected(
    'a Task cannot be constructed by hand',
    'cannot instantiate',
    fn() => new Task()
);
is_rejected(
    'a UdfModule cannot be constructed by hand',
    'cannot instantiate',
    fn() => new Aerospike\UdfModule()
);
is_rejected(
    'a Node cannot be constructed by hand',
    'cannot instantiate',
    fn() => new Aerospike\Node()
);

// A blank name is refused by the daemon before the cluster is asked, because the
// server's own complaint names neither the command nor the field.
foreach ([
    'a UDF registration needs a name' => fn() => $client->registerUdf(null, 'x', '  '),
    'a UDF removal needs a name'      => fn() => $client->removeUdf(null, ''),
    'an index create needs a name'    => fn() => $client->createIndexOnBin(
        null, $namespace, $indexSet, 'age', '', IndexType::Numeric
    ),
    'an index drop needs a name'      => fn() => $client->dropIndex(null, $namespace, $indexSet, ''),
] as $what => $call) {
    try {
        $call();
        fail($what, 'the call succeeded');
    } catch (Aerospike\AerospikeException $e) {
        is_true(
            $what,
            str_contains($e->getMessage(), 'needs a name'),
            describe($e)
        );
    }
}

// ===== Multi-record transactions ============================================

echo "\ntransactions\n";

/**
 * The namespace the transactional assertions use, and whether it is usable.
 *
 * Multi-record transactions require a **strong-consistency** namespace, and the
 * rest of this file requires one that is *not*: an SC namespace forbids
 * non-durable deletes, and durable deletes leave tombstones that change what
 * `delete()` and `exists()` report — which is exactly what those tests pin. So
 * the two halves can need two namespaces, and `AEROSPIKE_SC_NAMESPACE` names the
 * SC one.
 *
 * Defaults to `testsc`, the conventional name for one. A cluster that has no such
 * namespace answers the probe with nothing recognisable, so the block self-skips
 * and says why -- which is the right outcome rather than a failure.
 */
$scNamespace = getenv('AEROSPIKE_SC_NAMESPACE') ?: 'testsc';
$isStrongConsistency = (bool) preg_match(
    '/strong-consistency=true/',
    $client->info(null, ["namespace/$scNamespace"])["namespace/$scNamespace"] ?? ''
);
$mrtSupported = version_compare(explode('.', $nodes[0]->version())[0], '8', '>=');

$txn = $client->beginTransaction();
is_true('a transaction has an id', $txn->id() !== 0);
is_true('a new transaction is open', $txn->isOpen());
is_same('a new transaction is Open', TxnState::Open, $txn->state());
is_same('a transaction stringifies with its id', 'transaction ' . $txn->id(), (string) $txn);

// Two transactions are different transactions.
$other = $client->beginTransaction();
is_true('two transactions have different ids', $txn->id() !== $other->id());
is_same('aborting an empty transaction is Ok', AbortStatus::Ok, $other->abort());
is_true('an aborted transaction is not open', !$other->isOpen());
is_same('an aborted transaction reports Aborted', TxnState::Aborted, $other->state());
is_same('aborting twice is AlreadyAborted', AbortStatus::AlreadyAborted, $other->abort());
is_client_error(
    'committing an aborted transaction is refused',
    'was aborted and cannot be committed',
    fn() => $other->commit()
);

// A transaction the daemon has forgotten cannot be joined: the command must fail
// rather than quietly run outside it.
$forgotten = $client->beginTransaction();
$forgottenId = $forgotten->id();
$forgotten->abort();
try {
    $client->put(new WritePolicy(txn: $forgotten), new Key($namespace, $set, 'txn-late'), [new Bin('n', 1)]);
    fail('a command cannot join a finished transaction', 'the write succeeded');
} catch (Aerospike\AerospikeException $e) {
    is_true(
        'a command cannot join a finished transaction',
        $e->getStatus() === Status::TxnExpired,
        describe($e)
    );
    is_true(
        'and the message names the transaction and says the work must restart',
        str_contains($e->getMessage(), (string) $forgottenId)
            && str_contains($e->getMessage(), 'start again'),
        $e->getMessage()
    );
}

// A traversal names no keys, so it cannot be transactional — and the type system
// says so first: `query()` takes a `QueryPolicy`, which has no `txn` parameter at
// all. The daemon refuses one too, for a caller reaching it through the contract
// directly, which `daemon/tests/ipc_roundtrip.rs` covers.
is_rejected(
    'a QueryPolicy cannot carry a transaction',
    'txn',
    fn() => new QueryPolicy(txn: $txn)
);

// ----- the two policies that finish a transaction ---------------------------
//
// A commit is two batch commands — verify the versions of everything the
// transaction read, then roll its writes forward — so it takes two policies. An
// abort has nothing to verify, so it takes one.
$verify = new TxnVerifyPolicy(
    totalTimeoutMs: 30_000,
    socketTimeoutMs: 5_000,
    maxRetries: 8,
    sleepBetweenRetriesMs: 1_500,
    replica: Replica::Master,
    readModeSc: ReadModeSC::Linearize,
    useCompression: false,
);
is_same('a verify policy reports its total timeout', 30_000, $verify->totalTimeoutMs());
is_same('a verify policy reports its socket timeout', 5_000, $verify->socketTimeoutMs());
is_same('a verify policy reports its retries', 8, $verify->maxRetries());
is_same('a verify policy reports its retry pause', 1_500, $verify->sleepBetweenRetriesMs());
is_same('a verify policy reports its replica', Replica::Master, $verify->replica());
is_same('a verify policy reports its SC read mode', ReadModeSC::Linearize, $verify->readModeSc());
is_same('a verify policy reports its compression', false, $verify->useCompression());
is_true('an unset verify field reports null', $verify->readModeAp() === null);

$roll = new TxnRollPolicy(totalTimeoutMs: 30_000, maxRetries: 8);
is_same('a roll policy reports its total timeout', 30_000, $roll->totalTimeoutMs());
is_same('a roll policy reports its retries', 8, $roll->maxRetries());
is_true('an empty roll policy overrides nothing', (new TxnRollPolicy())->totalTimeoutMs() === null);

// Neither has `filter` or `txn`, and that is the design rather than an omission:
// a filter that skipped a record would mean not verifying or not rolling it, and
// the transaction being finished is the one whose method is being called.
foreach (['filter' => 'the filter', 'txn' => 'a transaction'] as $argument => $what) {
    is_rejected(
        "a verify policy cannot carry $what",
        $argument,
        fn() => new TxnVerifyPolicy(...[$argument => null])
    );
    is_rejected(
        "a roll policy cannot carry $what",
        $argument,
        fn() => new TxnRollPolicy(...[$argument => null])
    );
}
// And no write fields either: a roll moves writes the transaction already made,
// so a TTL would have nothing to apply to.
is_rejected(
    'a roll policy cannot carry an expiration',
    'expiration',
    fn() => new TxnRollPolicy(expiration: Expiration::seconds(60))
);

if (!$mrtSupported) {
    skip('transactional reads and writes', 'the server is older than 8.0');
} elseif (!$isStrongConsistency) {
    skip(
        'transactional reads and writes',
        "namespace '$scNamespace' is not configured with strong-consistency, which "
        . 'multi-record transactions require. Set AEROSPIKE_SC_NAMESPACE to one that is.'
    );
} else {
    // ---- Commit: two writes land together ----
    $a = new Key($scNamespace, $set, 'txn-a');
    $b = new Key($scNamespace, $set, 'txn-b');
    $client->put(null, $a, [new Bin('balance', 100)]);
    $client->put(null, $b, [new Bin('balance', 0)]);

    $transfer = $client->beginTransaction();
    $client->put(new WritePolicy(txn: $transfer), $a, [new Bin('balance', 70)]);
    $client->put(new WritePolicy(txn: $transfer), $b, [new Bin('balance', 30)]);
    // A read inside the transaction sees the transaction's own writes.
    is_same(
        'a read inside a transaction sees its writes',
        70,
        $client->get(new ReadPolicy(txn: $transfer), $a)->bin('balance')
    );
    is_same('committing succeeds', CommitStatus::Ok, $transfer->commit());
    is_same('a committed transaction reports Committed', TxnState::Committed, $transfer->state());
    is_same('committing twice is AlreadyCommitted', CommitStatus::AlreadyCommitted, $transfer->commit());
    is_same('the first write landed', 70, $client->get(null, $a)->bin('balance'));
    is_same('the second write landed', 30, $client->get(null, $b)->bin('balance'));

    // ---- Commit with explicit policies for both phases ----
    // Longer timeouts than the tuned defaults, which is the safe direction: a
    // commit that gives up leaves the transaction half-finished, holding locks.
    $explicit = $client->beginTransaction();
    $client->put(new WritePolicy(txn: $explicit), $a, [new Bin('balance', 55)]);
    $client->put(new WritePolicy(txn: $explicit), $b, [new Bin('balance', 45)]);
    is_same(
        'committing with both policies succeeds',
        CommitStatus::Ok,
        $explicit->commitWithPolicies($verify, $roll)
    );
    is_same('the first write landed', 55, $client->get(null, $a)->bin('balance'));
    is_same('the second write landed', 45, $client->get(null, $b)->bin('balance'));
    is_same(
        'committing twice with policies is still AlreadyCommitted',
        CommitStatus::AlreadyCommitted,
        $explicit->commitWithPolicies($verify, $roll)
    );

    // Either policy may be null, which is what `commit()` passes for both.
    $oneSided = $client->beginTransaction();
    $client->put(new WritePolicy(txn: $oneSided), $b, [new Bin('balance', 46)]);
    is_same(
        'a null verify policy takes the tuned default',
        CommitStatus::Ok,
        $oneSided->commitWithPolicies(null, $roll)
    );
    is_same('and the write still landed', 46, $client->get(null, $b)->bin('balance'));

    // ---- Abort: nothing lands ----
    $rollback = $client->beginTransaction();
    $client->put(new WritePolicy(txn: $rollback), $a, [new Bin('balance', -1000)]);
    is_same('aborting succeeds', AbortStatus::Ok, $rollback->abort());
    is_same('an aborted write left the record alone', 55, $client->get(null, $a)->bin('balance'));

    // ---- Abort with an explicit roll policy: one policy, not two ----
    $rolledBack = $client->beginTransaction();
    $client->put(new WritePolicy(txn: $rolledBack), $a, [new Bin('balance', -2000)]);
    is_same(
        'aborting with a roll policy succeeds',
        AbortStatus::Ok,
        $rolledBack->abortWithPolicy($roll)
    );
    is_same(
        'and the aborted write left the record alone',
        55,
        $client->get(null, $a)->bin('balance')
    );
    is_same(
        'aborting twice with a policy is still AlreadyAborted',
        AbortStatus::AlreadyAborted,
        $rolledBack->abortWithPolicy($roll)
    );

    // ---- Dropping an unfinished transaction rolls it back ----
    // The safety property this design rests on: a request that throws must not
    // leave record locks behind.
    $dropped = $client->beginTransaction();
    $client->put(new WritePolicy(txn: $dropped), $a, [new Bin('balance', -2000)]);
    unset($dropped);            // no commit, no abort — the destructor aborts
    is_same(
        'dropping an unfinished transaction rolls it back',
        55,
        $client->get(null, $a)->bin('balance')
    );

    // ---- A batch joins a transaction too ----
    $batched = $client->beginTransaction();
    $results = $client->batch(new ReadPolicy(txn: $batched), [
        BatchRead::all($a),
        BatchRead::all($b),
    ]);
    is_true('a batch can read inside a transaction', $results[0]->isOk() && $results[1]->isOk());
    $batched->abort();

    // A durable delete, because this is a strong-consistency namespace and SC
    // **forbids** the ordinary kind: a plain delete is result code 22,
    // `FailForbidden`. That is the rule that makes an SC namespace unusable for
    // the rest of this file, and the reason the two halves can need two
    // namespaces.
    $expunge = new WritePolicy(durableDelete: true);
    $client->delete($expunge, $a);
    $client->delete($expunge, $b);
}

// The open transaction from the top of this section: abort it explicitly rather
// than leaving the destructor to, so the assertion below is about the API and not
// about when PHP frees things.
is_same('the first transaction can still be aborted', AbortStatus::Ok, $txn->abort());

// ----- what transactions refuse ---------------------------------------------

echo "\ntransaction type safety\n";

is_type_error(
    'a policy txn must be a Transaction',
    fn() => new WritePolicy(txn: 'nope')
);
is_type_error(
    'a read policy txn must be a Transaction',
    fn() => new ReadPolicy(txn: 42)
);
is_rejected(
    'a Transaction cannot be constructed by hand',
    'cannot instantiate',
    fn() => new Transaction()
);
is_client_error(
    'a negative transaction timeout is refused',
    'must be between 0',
    fn() => $client->beginTransaction(-1)
);
is_type_error(
    'a transaction timeout is an int',
    fn() => $client->beginTransaction('soon')
);
is_client_error(
    'a negative verify timeout is refused',
    'must be between 0',
    fn() => new TxnVerifyPolicy(totalTimeoutMs: -1)
);
is_client_error(
    'a negative roll timeout is refused',
    'must be between 0',
    fn() => new TxnRollPolicy(totalTimeoutMs: -1)
);

// The two halves of a commit take their own policy types, so passing them the
// wrong way round is a TypeError at the call site rather than a silently wrong
// batch. `abortWithPolicy` takes only the roll — the asymmetry the API exists to
// express — so an extra argument is an error too.
$typed = $client->beginTransaction();
is_type_error(
    'commitWithPolicies takes a verify policy first',
    fn() => $typed->commitWithPolicies(new TxnRollPolicy(), null)
);
is_type_error(
    'commitWithPolicies takes a roll policy second',
    fn() => $typed->commitWithPolicies(null, new TxnVerifyPolicy())
);
is_type_error(
    'abortWithPolicy takes a roll policy',
    fn() => $typed->abortWithPolicy(new TxnVerifyPolicy())
);
is_type_error(
    'abortWithPolicy takes no second policy',
    fn() => $typed->abortWithPolicy(new TxnRollPolicy(), new TxnRollPolicy())
);
is_true('and the transaction survived every rejected call', $typed->isOpen());
$typed->abort();

// A policy carries the id, not the object — so a policy kept in a variable does
// not keep a transaction alive past its destructor.
$held = $client->beginTransaction();
$policy = new WritePolicy(txn: $held);
is_same('a policy carries the transaction id', $held->id(), $policy->txnId());
is_true('a policy with no transaction reports null', (new WritePolicy())->txnId() === null);
$held->abort();

// ===== Users, roles and privileges ==========================================

echo "\nsecurity\n";

// Privileges are built and checked entirely client-side, so these hold whether or
// not the cluster has security enabled — which is most of what there is to get
// wrong about them.
$readWrite = new Privilege(PrivilegeCode::ReadWrite);
is_same('a privilege reports its code', PrivilegeCode::ReadWrite, $readWrite->code());
is_true('an unscoped privilege is not scoped', !$readWrite->isScoped());
is_true('an unscoped privilege has no namespace', $readWrite->namespace() === null);
is_same('a privilege stringifies as its code', 'READ_WRITE', (string) $readWrite);

$namespaced = new Privilege(PrivilegeCode::Read, $namespace);
is_true('a namespaced privilege is scoped', $namespaced->isScoped());
is_same('and reports its namespace', $namespace, $namespaced->namespace());
is_same('and stringifies with it', "READ on $namespace", (string) $namespaced);

$scoped = new Privilege(PrivilegeCode::Read, $namespace, 'users');
is_same('a set-scoped privilege reports its set', 'users', $scoped->setName());
is_same('and stringifies with both', "READ on $namespace.users", (string) $scoped);

// The rule the class exists for: the administrative codes act on the cluster, so
// the server refuses a namespace with a parameter error that names neither the
// privilege nor the reason.
foreach ([
    PrivilegeCode::UserAdmin, PrivilegeCode::SysAdmin, PrivilegeCode::DataAdmin,
    PrivilegeCode::UdfAdmin, PrivilegeCode::SIndexAdmin, PrivilegeCode::MaskingAdmin,
] as $code) {
    is_client_error(
        "a {$code->value} privilege cannot be confined to a namespace",
        'whole cluster',
        fn() => new Privilege($code, $namespace)
    );
    is_true(
        "a {$code->value} privilege is fine unscoped",
        !(new Privilege($code))->isScoped()
    );
}
is_client_error(
    'a set without a namespace is refused',
    'within a namespace',
    fn() => new Privilege(PrivilegeCode::Read, null, 'users')
);

// Whether the cluster can actually run these. Security is a configuration the
// operator chose, so the commands self-skip rather than failing on a cluster that
// is simply not set up for it.
$securityEnabled = true;
try {
    $client->queryRoles(null);
} catch (Aerospike\AerospikeException $e) {
    // 52 is SECURITY_NOT_ENABLED; anything else is a real failure.
    $securityEnabled = $e->getResultCode() !== 52;
    if ($securityEnabled) {
        fail('the cluster can be asked about roles', describe($e));
    }
}

if (!$securityEnabled) {
    skip(
        'users, roles and privileges',
        'the cluster does not have security enabled (result code 52), which every '
        . 'one of these commands requires'
    );
} else {
    ok('the cluster has security enabled');

    // The server's own built-in roles are listed, not only ones created here.
    $roles = $client->queryRoles(null);
    $roleNames = array_map(fn($r) => $r->name(), $roles);
    is_true(
        'the built-in roles are listed',
        in_array('read-write', $roleNames, true),
        implode(', ', $roleNames)
    );

    $testRole = 'php_smoke_role';
    $testUser = 'php_smoke_user';

    // Clean up anything a previous run left, so this does not depend on how that
    // run ended — and wait for the drop to be visible before creating it again,
    // which would otherwise race it and fail with RoleAlreadyExists.
    try { $client->dropUser(null, $testUser); } catch (Aerospike\AerospikeException $e) {}
    try { $client->dropRole(null, $testRole); } catch (Aerospike\AerospikeException $e) {}
    eventually(
        fn() => !in_array(
            $testRole,
            array_map(fn($r) => $r->name(), $client->queryRoles(null)),
            true
        ),
        fn($gone) => $gone
    );

    // ---- roles ----
    $client->createRole(null, $testRole, [
        new Privilege(PrivilegeCode::Read, $namespace),
    ], ['10.0.0.0/8'], 1000, 0);
    // Every read in this block waits for the write before it: see eventually().
    $mine = eventually(
        fn() => $client->queryRoles(null, $testRole),
        fn($roles) => count($roles) === 1 && $roles[0]->readQuota() === 1000
            && $roles[0]->allowlist() === ['10.0.0.0/8']
    );
    is_same('a created role can be read back', 1, count($mine));
    is_same('a role reports its name', $testRole, $mine[0]->name());
    is_same('a role reports its privileges', 1, count($mine[0]->privileges()));
    is_same(
        'a role reports its privilege scope',
        "READ on $namespace",
        (string) $mine[0]->privileges()[0]
    );
    is_same('a role reports its allowlist', ['10.0.0.0/8'], $mine[0]->allowlist());
    is_same('a role reports its read quota', 1000, $mine[0]->readQuota());
    is_true('an unlimited quota is null, not zero', $mine[0]->writeQuota() === null);

    $privileges = fn(int $want) => eventually(
        fn() => count($client->queryRoles(null, $testRole)[0]->privileges()),
        fn($count) => $count === $want
    );
    $client->grantPrivileges(null, $testRole, [new Privilege(PrivilegeCode::Write, $namespace)]);
    is_same('granting a privilege adds it', 2, $privileges(2));
    $client->revokePrivileges(null, $testRole, [new Privilege(PrivilegeCode::Write, $namespace)]);
    is_same('revoking a privilege removes it', 1, $privileges(1));

    // An empty allowlist *clears* the restriction — the one place this API accepts
    // an empty list, because there is no other way to say it.
    $client->setAllowlist(null, $testRole, []);
    is_same(
        'an empty allowlist clears the restriction',
        [],
        eventually(
            fn() => $client->queryRoles(null, $testRole)[0]->allowlist(),
            fn($allowlist) => $allowlist === []
        )
    );

    // Zero lifts a quota, for the same reason.
    $client->setQuotas(null, $testRole, 0, 0);
    $lifted = eventually(
        fn() => $client->queryRoles(null, $testRole)[0],
        fn($role) => $role->readQuota() === null && $role->writeQuota() === null
    );
    is_true(
        'zero quotas are reported as unlimited',
        $lifted->readQuota() === null && $lifted->writeQuota() === null
    );

    // ---- users ----
    $client->createUser(null, $testUser, 'sm0ke-pass', [$testRole]);
    $users = eventually(
        fn() => $client->queryUsers(null, $testUser),
        fn($users) => count($users) === 1 && $users[0]->hasRole($testRole)
    );
    is_same('a created user can be read back', 1, count($users));
    is_same('a user reports its name', $testUser, $users[0]->name());
    is_true('a user holds the role it was given', $users[0]->hasRole($testRole));
    is_true('and not one it was not', !$users[0]->hasRole('sys-admin'));
    is_true('a user reports its connection count', $users[0]->connsInUse() >= 0);

    $holdsRead = fn(bool $want) => eventually(
        fn() => $client->queryUsers(null, $testUser)[0]->hasRole('read'),
        fn($holds) => $holds === $want
    );
    $client->grantRoles(null, $testUser, ['read']);
    is_true('granting a role adds it', $holdsRead(true));
    $client->revokeRoles(null, $testUser, ['read']);
    is_true('revoking a role removes it', !$holdsRead(false));

    $client->changePassword(null, $testUser, 'sm0ke-pass-2');
    ok('a password can be changed');

    // Listing every user includes the one just made.
    $all = eventually(
        fn() => array_map(fn($u) => $u->name(), $client->queryUsers(null)),
        fn($names) => in_array($testUser, $names, true)
    );
    is_true('every user is listed', in_array($testUser, $all, true), implode(', ', $all));

    $client->dropUser(null, $testUser);
    // Naming a user that does not exist is a server error, not an empty list —
    // so the failure *is* the assertion, once the drop has caught up.
    is_true('a dropped user is gone', eventually(
        function () use ($client, $testUser) {
            try {
                $client->queryUsers(null, $testUser);
                return false;
            } catch (Aerospike\AerospikeException $e) {
                return true;
            }
        },
        fn($gone) => $gone
    ));

    $client->dropRole(null, $testRole);
    is_true(
        'a dropped role is gone',
        eventually(
            fn() => !in_array(
                $testRole,
                array_map(fn($r) => $r->name(), $client->queryRoles(null)),
                true
            ),
            fn($gone) => $gone
        )
    );
}

// ----- what the security commands refuse ------------------------------------

echo "\nsecurity type safety\n";

is_type_error(
    'a privilege code is an enum, not a string',
    fn() => new Privilege('READ_WRITE')
);
is_type_error(
    'createRole needs an array of Privilege',
    fn() => $client->createRole(null, 'x', [PrivilegeCode::Read])
);
is_type_error(
    'createUser will not take a WritePolicy',
    fn() => $client->createUser(new WritePolicy(), 'u', 'p')
);
is_type_error(
    'grantRoles needs an array of strings',
    fn() => $client->grantRoles(null, 'u', 'read')
);
is_type_error(
    'setQuotas needs ints',
    fn() => $client->setQuotas(null, 'r', 'lots', 0)
);
is_client_error(
    'a role needs at least one privilege',
    'permits nothing',
    fn() => $client->createRole(null, 'x', [])
);
is_client_error(
    'a negative quota is refused',
    'must be between 0',
    fn() => $client->setQuotas(null, 'x', -1, 0)
);
is_rejected(
    'a User cannot be constructed by hand',
    'cannot instantiate',
    fn() => new Aerospike\User()
);
is_rejected(
    'a Role cannot be constructed by hand',
    'cannot instantiate',
    fn() => new Aerospike\Role()
);

// The daemon refuses a blank name before the cluster is asked, as it does for the
// UDF and index commands.
foreach ([
    'creating a user needs a name' => fn() => $client->createUser(null, '  ', 'p'),
    'dropping a user needs a name' => fn() => $client->dropUser(null, ''),
    'creating a role needs a name' => fn() => $client->createRole(
        null, '', [new Privilege(PrivilegeCode::Read, $namespace)]
    ),
    'dropping a role needs a name' => fn() => $client->dropRole(null, '  '),
] as $what => $call) {
    try {
        $call();
        fail($what, 'the call succeeded');
    } catch (Aerospike\AerospikeException $e) {
        is_true($what, str_contains($e->getMessage(), 'needs a name'), describe($e));
    }
}

// ===== Summary ==============================================================

$summary = "$passed passed, $failed failed";
if ($skipped > 0) {
    $summary .= ", $skipped skipped";
}
echo "\n" . ($failed === 0 ? 'PASS' : 'FAIL') . ": $summary\n";
exit($failed === 0 ? 0 : 1);
