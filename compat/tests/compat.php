<?php

/**
 * The compatibility layer, exercised the way 1.x code was written.
 *
 *   php -d extension=../ext/target/release/libaerospike_php.dylib tests/compat.php
 *
 * Every assertion here is deliberately written in the **old** idiom — mutable
 * policies, static enum factories, policy-first CDT operations, `Client::connect()`
 * — because that is the only thing worth testing. A test written in the new idiom
 * would pass without the layer being loaded at all.
 *
 * Needs the daemon running, as the extension's own suite does.
 *
 * Environment:
 *   AEROSPIKE_INSTANCE   daemon instance (default "default")
 *   AEROSPIKE_NAMESPACE  namespace for the test records (default "test")
 *   AEROSPIKE_SET        set for the test records (default "compat")
 */

declare(strict_types=1);

require_once __DIR__ . '/../src/bootstrap.php';

$passed = 0;
$failed = 0;

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

function is_true(string $what, bool $condition, string $why = ''): void
{
    $condition ? ok($what) : fail($what, $why !== '' ? $why : 'condition was false');
}

function is_same(string $what, mixed $expected, mixed $actual): void
{
    if ($expected === $actual) {
        ok($what);
        return;
    }
    fail($what, sprintf('expected %s, got %s', var_export($expected, true), var_export($actual, true)));
}

/** Assert that a call is refused, with a message saying why. */
function refuses(string $what, string $expected, callable $call): void
{
    try {
        $call();
        fail($what, 'it was accepted');
    } catch (\Throwable $e) {
        is_true($what, str_contains($e->getMessage(), $expected), $e->getMessage());
    }
}

$instance = getenv('AEROSPIKE_INSTANCE') ?: 'default';
$namespace = getenv('AEROSPIKE_NAMESPACE') ?: 'test';
$set = getenv('AEROSPIKE_SET') ?: 'compat';

echo "1.x compatibility layer\n";
echo "instance=$instance namespace=$namespace set=$set php=" . PHP_VERSION . "\n";

// ===== connecting, the 1.x way ==============================================

echo "\nconnecting\n";

$client = Aerospike\Compat\Client::connect($instance);
is_true('Client::connect() by instance name', $client instanceof Aerospike\Compat\Client);
is_same('socket() reports what it connected to', $instance, $client->socket());

// The 1.x configuration passed a socket path; the basename is the instance.
is_same(
    'a 1.x socket path maps to an instance name',
    'default',
    Aerospike\Compat\Client::connect('/tmp/asd-default.sock')->socket()
);
is_same(
    'and a plain path does too',
    'analytics',
    Aerospike\Compat\Client::connect('/var/run/analytics.sock')->socket()
);

// ===== mutable policies =====================================================

echo "\nmutable policies\n";

$wp = new Aerospike\Compat\WritePolicy();
$wp->setExpiration(3600);
$wp->setSendKey(true);
$wp->setRecordExistsAction(Aerospike\RecordExistsAction::Update());
$wp->total_timeout = 2000;
is_same('a setter reads back', 3600, $wp->getExpiration());
is_same('a property reads back', 2000, $wp->getTotalTimeout());
is_true('build() produces the immutable policy', $wp->build() instanceof Aerospike\WritePolicy);
is_same('and it carries the setting', 3600, $wp->build()->expiration()->toSeconds());
is_same('and the record-exists action', Aerospike\RecordExistsAction::Update, $wp->build()->recordExistsAction());

refuses(
    'a misspelled property is refused rather than silently ignored',
    'has no property $expiraton',
    function() {
        $typo = new Aerospike\Compat\WritePolicy();
        $typo->expiraton = 3600;
    }
);

// The 1.x sentinels for the three expirations that are not durations.
is_true('expiration -1 is never', (function() {
    $p = new Aerospike\Compat\WritePolicy();
    $p->setExpiration(-1);
    return $p->build()->expiration()->isNever();
})());
is_true('expiration -2 is do-not-update', (function() {
    $p = new Aerospike\Compat\WritePolicy();
    $p->setExpiration(-2);
    return $p->build()->expiration()->isDontUpdate();
})());
is_true('expiration 0 is the namespace default', (function() {
    $p = new Aerospike\Compat\WritePolicy();
    $p->setExpiration(0);
    return $p->build()->expiration()->isNamespaceDefault();
})());
refuses(
    'any other negative expiration is refused by name',
    'never meaningful',
    function() {
        $p = new Aerospike\Compat\WritePolicy();
        $p->setExpiration(-7);
        $p->build();
    }
);

$rp = new Aerospike\Compat\ReadPolicy();
$rp->setReadModeAp(Aerospike\ReadModeAP::one());
$rp->setTotalTimeout(500);
is_same('a read policy builds', Aerospike\ReadModeAp::One, $rp->build()->readModeAp());

// A 1.x filter_expression held an Expression; this client has two parameters.
$rp2 = new Aerospike\Compat\ReadPolicy();
$rp2->setFilterExpression(Aerospike\Expression::gt(
    Aerospike\Expression::intBin('age'),
    Aerospike\Expression::intVal(21)
));
is_true('an Expression filter reaches filterExp', $rp2->build()->filterExp() !== null);
is_true('and not the text filter', $rp2->build()->filter() === null);

$rp3 = new Aerospike\Compat\ReadPolicy();
$rp3->setFilterExpression('$.age:INT > 21');
is_same('text reaches the text filter', '$.age:INT > 21', $rp3->build()->filter());

// ===== drop-in enum factories ===============================================

echo "\nenum factories\n";

is_same('ReadModeAP::one()', Aerospike\ReadModeAp::One, Aerospike\ReadModeAP::one());
is_same('ReadModeSC::Linearize()', Aerospike\ReadModeSc::Linearize, Aerospike\ReadModeSC::Linearize());
is_same('ListOrderType::Ordered()', Aerospike\ListOrder::Ordered, Aerospike\ListOrderType::Ordered());
is_same('MapOrderType::KeyOrdered()', Aerospike\MapOrder::KeyOrdered, Aerospike\MapOrderType::KeyOrdered());
is_same('ListReturnType::count()', Aerospike\ListReturn::Count, Aerospike\ListReturnType::count());
is_same('ListReturnType::Value() is Values', Aerospike\ListReturn::Values, Aerospike\ListReturnType::Value());
is_same('MapReturnType::KeyValue()', Aerospike\MapReturn::KeyValue, Aerospike\MapReturnType::KeyValue());
is_same('IndexCollectionType::Default() is Scalar', Aerospike\CollectionIndex::Scalar, Aerospike\IndexCollectionType::Default());
is_same('IndexCollectionType::List()', Aerospike\CollectionIndex::ListElements, Aerospike\IndexCollectionType::List());
is_same('BitwiseResizeFlags::Default() is AtEnd', Aerospike\BitResize::AtEnd, Aerospike\BitwiseResizeFlags::Default());
is_same('BitwiseOverflowAction::Wrap()', Aerospike\BitOverflow::Wrap, Aerospike\BitwiseOverflowAction::Wrap());

// The ones the extension owns need the Compat namespace.
// The 1.x factories live on the extension's own enums, in `Aerospike\`. That is not
// a stylistic choice: PHP class names are case-insensitive, so a PHP file cannot
// define `Aerospike\ReadModeAP` while the extension owns `Aerospike\ReadModeAp` —
// they are one name. So these are the names old code already writes.
is_same('CommitLevel::CommitAll()', Aerospike\CommitLevel::CommitAll, Aerospike\CommitLevel::CommitAll());
is_same('IndexType::String() is Text', Aerospike\IndexType::Text, Aerospike\IndexType::String());
is_same('ExpType::Int() is Integer', Aerospike\ExpType::Integer, Aerospike\ExpType::Int());
is_same('ExpType::List() is ListType', Aerospike\ExpType::ListType, Aerospike\ExpType::List());
is_same('UdfLanguage::Lua()', Aerospike\UdfLanguage::Lua, Aerospike\UdfLanguage::Lua());
is_same('ReadModeSC::Session()', Aerospike\ReadModeSc::Session, Aerospike\ReadModeSc::Session());
is_same('GenerationPolicy::ExpectGenEqual()', Aerospike\GenerationPolicy::ExpectGenEqual, Aerospike\GenerationPolicy::ExpectGenEqual());
is_same('MapWriteMode::CreateOnly()', Aerospike\MapWriteMode::CreateOnly, Aerospike\MapWriteMode::CreateOnly());
is_same('BitwiseOverflowAction::Fail()', Aerospike\BitOverflow::Fail, Aerospike\BitwiseOverflowAction::Fail());
// And `ConsistencyLevel`, which the server itself replaced with the read modes.
is_same('ConsistencyLevel::ConsistencyOne() is a read mode', Aerospike\ReadModeAp::One, Aerospike\ConsistencyLevel::ConsistencyOne());

// Accepted and inert, so old code keeps parsing.
is_true('Concurrency still resolves', is_int(Aerospike\Concurrency::Parallel()));
is_true('QueryDuration still resolves', is_int(Aerospike\QueryDuration::short()));
is_same('ParticleType numbers are the server\'s', 3, Aerospike\ParticleType::string());
is_same('ConsistencyLevel maps onto the AP read mode', Aerospike\ReadModeAp::All, Aerospike\ConsistencyLevel::ConsistencyAll());

// ===== aliases ==============================================================

echo "\naliases\n";

is_true('GeoJSON is GeoJson', (new Aerospike\GeoJSON('{"type":"Point","coordinates":[0,0]}')) instanceof Aerospike\GeoJson);
is_true('HLL is Hll', class_exists('Aerospike\HLL'));

// ===== single-record verbs, 1.x style =======================================

echo "\nsingle-record verbs\n";

$key = new Aerospike\Key($namespace, $set, 'c1');
$client->delete(null, $key);

/*
 * The policy above carries a TTL, and a namespace whose reaper is off
 * (`nsup-period=0`) refuses any write that sets one — result code 22,
 * `FailForbidden` — unless it was configured with `allow-ttl-without-nsup`. What
 * this section is about is the 1.x *policy object* reaching the server, not the
 * TTL, so on such a namespace it asks for "never expire" instead, which every
 * namespace accepts. (`info()` is forwarded to the modern client untouched.)
 */
$namespaceConfig = $client->info(null, ["namespace/$namespace"])["namespace/$namespace"] ?? '';
$finiteTtls = !preg_match('/(^|;)nsup-period=0(;|$)/', $namespaceConfig)
    || (bool) preg_match('/(^|;)allow-ttl-without-nsup=true(;|$)/', $namespaceConfig);
if (!$finiteTtls) {
    $wp->setExpiration(-1);
}

$client->put($wp, $key, [new Aerospike\Bin('age', 30), new Aerospike\Bin('name', 'Alice')]);
ok('put() with a 1.x policy');

$record = $client->get($rp, $key);
is_same('get() reads it back', 30, $record->getBins()['age']);
is_same('and the 1.x property form works', 'Alice', $record->bins['name']);
is_true('getHeader() has no bins', count($client->getHeader(null, $key)->getBins()) === 0);
is_true('exists()', $client->exists(null, $key));

$client->add(null, $key, [new Aerospike\Bin('age', 1)]);
is_same('add()', 31, $client->get(null, $key)->bin('age'));
$client->append(null, $key, [new Aerospike\Bin('name', ' B.')]);
is_same('append()', 'Alice B.', $client->get(null, $key)->bin('name'));

// A bin selection was a plain array of names in 1.x.
$selected = $client->get(null, $key, ['age']);
is_true('a plain array of bin names selects bins', $selected->has('age') && !$selected->has('name'));

// ===== CDT operations, policy first =========================================

echo "\nCDT operations\n";

$listKey = new Aerospike\Key($namespace, $set, 'c-list');
$client->delete(null, $listKey);
$client->put(null, $listKey, [new Aerospike\Bin('history', ['a', 'b'])]);

$client->operate(null, $listKey, [
    Aerospike\Compat\ListOp::append(null, 'history', ['c'], null),
]);
is_same('ListOp::append() with the 1.x argument order', 3, count($client->get(null, $listKey)->bin('history')));

$result = $client->operate(null, $listKey, [Aerospike\Compat\ListOp::size('history', null)]);
is_same('ListOp::size()', 3, $result->bin('history'));

// A 1.x list policy was an order plus an OR-ed flag bitmask.
$lp = new Aerospike\Compat\ListPolicy(
    Aerospike\ListOrderType::Unordered(),
    Aerospike\ListWriteFlags::AddUnique() | Aerospike\ListWriteFlags::NoFail()
);
$client->operate(null, $listKey, [Aerospike\Compat\ListOp::append($lp, 'history', ['c'], null)]);
is_same(
    'a bitmask list policy decomposes: addUnique kept the duplicate out',
    3,
    count($client->get(null, $listKey)->bin('history'))
);

// The 1.x *RangeCount pairs.
$popped = $client->operate(null, $listKey, [
    Aerospike\Compat\ListOp::getByIndexRangeCount('history', 0, 2, Aerospike\ListReturnType::Value(), null),
]);
is_same('getByIndexRangeCount()', 2, count($popped->bin('history')));

$mapKey = new Aerospike\Key($namespace, $set, 'c-map');
$client->delete(null, $mapKey);
$client->put(null, $mapKey, [new Aerospike\Bin('counts', ['a' => 5])]);
$client->operate(null, $mapKey, [
    Aerospike\Compat\MapOp::increment(null, 'counts', 'a', 3, null),
]);
is_same('MapOp::increment()', 8, $client->get(null, $mapKey)->bin('counts')['a']);
$client->operate(null, $mapKey, [
    Aerospike\Compat\MapOp::decrement(null, 'counts', 'a', 3, null),
]);
is_same('MapOp::decrement(), whose 1.x argument order differs', 5, $client->get(null, $mapKey)->bin('counts')['a']);
refuses(
    'and decrement refuses a non-number rather than sending nonsense',
    'needs a number',
    fn() => Aerospike\Compat\MapOp::decrement(null, 'counts', 'a', 'three', null)
);

// A context path, which 1.x passed as the last argument.
$nested = new Aerospike\Key($namespace, $set, 'c-nested');
$client->delete(null, $nested);
$client->put(null, $nested, [new Aerospike\Bin('outer', ['inner' => [1, 2, 3]])]);
$size = $client->operate(null, $nested, [
    Aerospike\Compat\ListOp::size('outer', [Aerospike\Context::mapKey('inner')]),
]);
is_same('a 1.x trailing context path reaches the nested list', 3, $size->bin('outer'));

refuses(
    'a context on a bitwise operation is refused, not ignored',
    'do not take one',
    fn() => Aerospike\Compat\BitwiseOp::not(null, 'flags', 0, 8, [Aerospike\Context::mapKey('x')])
);

// ===== the 1.x expression builder ==========================================

echo "\nthe 1.x expression builder\n";

$filter = Aerospike\Expression::and([
    Aerospike\Expression::gt(Aerospike\Expression::intBin('age'), Aerospike\Expression::intVal(21)),
    Aerospike\Expression::binExists('name'),
]);
$rp4 = new Aerospike\Compat\ReadPolicy();
$rp4->setFilterExpression($filter);
is_true('a 1.x-built filter reaches the server and matches', $client->get($rp4, $key) !== null);

$rp5 = new Aerospike\Compat\ReadPolicy();
$rp5->setFilterExpression(Aerospike\Expression::lt(
    Aerospike\Expression::intBin('age'),
    Aerospike\Expression::intVal(5)
));
is_true('and one that does not match filters the read out', (function() use ($client, $key, $rp5) {
    try {
        return $client->get($rp5, $key) === null;
    } catch (Aerospike\AerospikeException $e) {
        return $e->getResultCode() === 27;
    }
})());
is_true('Expression::expLet()', Aerospike\Expression::expLet([
    Aerospike\Expression::def('a', Aerospike\Expression::intBin('age')),
    Aerospike\Expression::gt(Aerospike\Expression::var('a'), Aerospike\Expression::intVal(1)),
])->isBuilt());

// ===== Value, and Recordset's semantics =====================================

echo "\nvalues and record sets\n";

is_true('Value::blob() from an array of bytes', Aerospike\Value::blob([1, 2, 3]) instanceof Aerospike\Blob);
is_true('Value::blob() from a string', Aerospike\Value::blob("\x01\x02") instanceof Aerospike\Blob);
is_true('Value::geoJson()', Aerospike\Value::geoJson('{"type":"Point","coordinates":[0,0]}') instanceof Aerospike\GeoJson);
is_same('Value::int() is the identity', 7, Aerospike\Value::int(7));
is_same('Value::list() is the identity', [1, 2], Aerospike\Value::list([1, 2]));
refuses('Value::blob() refuses a non-byte', 'is an int from 0 to 255', fn() => Aerospike\Value::blob([300]));

$rs = $client->scan(null, null, $namespace, $set);
// Not `Aerospike\Recordset`: that name is case-insensitively the extension's own
// `RecordSet`, so it cannot be defined here. The wrapper is in `Compat\`, and the
// point of that is the assertion below it — an old type hint fails loudly.
is_true('scan() returns a 1.x-shaped Recordset', $rs instanceof Aerospike\Compat\Recordset);
is_true(
    'and a 1.x `Aerospike\Recordset` type hint rejects it, rather than silently iterating zero times',
    !($rs instanceof Aerospike\RecordSet)
);
// The native class carries the 1.x pull under a name PHP allows.
$native = $rs->inner();
is_true('RecordSet::nextRecord() is the 1.x pull on the native class', $native->nextRecord() instanceof Aerospike\Record);
is_true('and getActive() is the 1.x liveness check', $native->getActive() === true || $native->getActive() === false);
$pulled = 0;
while (($rec = $rs->next()) !== null) {
    $pulled++;
    if ($pulled > 50) {
        break;
    }
}
is_true('Recordset::next() returns records, as 1.x did', $pulled >= 3, "pulled $pulled");
$rs->close();

// And foreach still works, on a fresh traversal.
$iterated = 0;
foreach ($client->scan(null, null, $namespace, $set) as $rec) {
    $iterated++;
}
is_true('and foreach works on the same class', $iterated >= 3, "iterated $iterated");
is_same('the two agree on how many records there are', $pulled, $iterated);

// ===== batch, UDF metadata, and the leftover 1.x names ======================

echo "\nbatch and the leftover names\n";

// 1.x `batch()` handed back a `BatchRecord` per row, carrying the key. A reply row
// does not contain one, so the layer pairs each result with the command's key.
$batchKeys = [
    new Aerospike\Key($namespace, $set, 'c1'),
    new Aerospike\Key($namespace, $set, 'no-such-record'),
];
$rows = $client->batch(null, [
    Aerospike\BatchRead::all($batchKeys[0]),
    Aerospike\BatchRead::all($batchKeys[1]),
]);
is_same('batch() returns one row per command', 2, count($rows));
is_true('and each row is a 1.x BatchRecord', $rows[0] instanceof Aerospike\BatchRecord);
is_same('getKey() is the key of the command at that index', 'c1', $rows[0]->getKey()->getValue());
is_true('getRecord() is the record', $rows[0]->getRecord() instanceof Aerospike\Record);
is_same('getResultCode() reports success as 0, as 1.x did', Aerospike\ResultCode::OK, $rows[0]->getResultCode());
is_same(
    'and a missing record as KEY_NOT_FOUND_ERROR',
    Aerospike\ResultCode::KEY_NOT_FOUND_ERROR,
    $rows[1]->getResultCode()
);
is_true('a BatchRecord forwards the new methods too', $rows[0]->isOk() === true);
// The native class carries the 1.x getters as well, for code holding a BatchResult.
is_true('BatchResult::getRecord() is the 1.x spelling', $rows[0]->inner()->getRecord() instanceof Aerospike\Record);
is_same('BatchResult::getResultCode() too', 0, $rows[0]->inner()->getResultCode());

// `listUdf()` returned `UdfMeta` in 1.x.
$modules = $client->listUdf(null);
is_true('listUdf() returns UdfMeta objects', $modules === [] || $modules[0] instanceof Aerospike\UdfMeta);
if ($modules !== []) {
    is_true('with a package name', is_string($modules[0]->getPackageName()));
    is_true('and a hash', is_string($modules[0]->getHash()));
} else {
    ok('with a package name (no modules registered, so vacuously)');
    ok('and a hash (no modules registered, so vacuously)');
}

// The flag sets and policy holders 1.x had and this client arranges differently.
is_same('BitwiseWriteFlags is the server\'s numbers', 4, Aerospike\BitwiseWriteFlags::No_Fail());
is_same('BitwisePolicy holds a mask', 1, (new Aerospike\BitwisePolicy(Aerospike\BitwiseWriteFlags::CREATE_ONLY))->getFlags());
is_same('HllPolicy holds a mask', 8, (new Aerospike\HllPolicy(Aerospike\HllWriteFlags::ALLOW_FOLD))->getFlags());
is_same('Json::getValue() gives the array back', ['a' => 1], (new Aerospike\Json(['a' => 1]))->getValue());
is_same('PartitionStatus still answers which partition', 42, (new Aerospike\PartitionStatus(42))->getPartitionId());

// `Aerospike\BitwiseOp` is free — the extension registers `BitOp` — so the 1.x name
// is the real one and old code needs no edit.
is_true('Aerospike\BitwiseOp is the 1.x name, not a Compat one', class_exists('Aerospike\BitwiseOp'));
is_same(
    'and it is the same class as the Compat one',
    'Aerospike\Compat\BitwiseOp',
    (new ReflectionClass('Aerospike\BitwiseOp'))->getName()
);

// ===== cleanup ==============================================================

foreach (['c1', 'c-list', 'c-map', 'c-nested'] as $name) {
    $client->delete(null, new Aerospike\Key($namespace, $set, $name));
}

echo "\n";
if ($failed > 0) {
    echo "FAIL: $passed passed, $failed failed\n";
    exit(1);
}
echo "PASS: $passed passed, 0 failed\n";
