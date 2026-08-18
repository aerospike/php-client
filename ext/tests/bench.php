<?php
/**
 * PHP-side benchmark for the Aerospike PHP client.
 *
 * Writes and reads the same record shape as
 * `daemon/examples/bench_core_baseline.rs`, with the same iteration counts, so
 * the difference between the two is the cost of the daemon hop.
 *
 * Reports CPU time as well as wall time, because the two wakeup strategies
 * trade exactly that: polling shared memory spends CPU to shave latency, while
 * the event build sleeps and pays a syscall per wakeup.
 *
 *   php -d extension=$(pwd)/target/release/libaerospike_php.dylib tests/bench.php
 *
 * Honours BENCH_ITERATIONS (default 20000) and BENCH_WARMUP (default 2000).
 */

$iterations = (int) (getenv('BENCH_ITERATIONS') ?: 20000);
$warmup     = (int) (getenv('BENCH_WARMUP') ?: 2000);

// Built once, outside the loop, exactly as an application should: a Bin
// converts its value when it is constructed, so rebuilding these every
// iteration would be measuring the conversion instead of the round trip.
$record = [
    new Aerospike\Bin('name', 'Alice'),
    new Aerospike\Bin('age', 30),
    new Aerospike\Bin('scores', [95, 87, 92]),
    new Aerospike\Bin('prefs', ['theme' => 'dark', 'lang' => 'en']),
];

/** Mean, median and tail of a latency sample, in microseconds. */
function report(string $label, array $samples): void {
    sort($samples);
    $n = count($samples);
    $mean = array_sum($samples) / $n;
    $pick = function (float $q) use ($samples, $n) {
        return $samples[min((int) ($n * $q), $n - 1)];
    };
    printf(
        "%-12s n=%-7d mean=%8.2f  p50=%8.2f  p99=%8.2f  p999=%8.2f  ops/s=%9.0f\n",
        $label, $n, $mean, $pick(0.50), $pick(0.99), $pick(0.999), 1e6 / $mean
    );
}

/** User + system CPU microseconds consumed by this process so far. */
function cpu_micros(): float {
    $u = getrusage();
    return ($u['ru_utime.tv_sec'] + $u['ru_stime.tv_sec']) * 1e6
         + $u['ru_utime.tv_usec'] + $u['ru_stime.tv_usec'];
}

$client = new Aerospike\Client();

printf("php client via daemon (version=%s)\n", $client->ping()->version());
printf("iterations=%d warmup=%d\n\n", $iterations, $warmup);

// A thousand keys, built once. Constructing a Key validates it, so building
// them inside the timed loop would charge that to the round trip.
$keys = [];
for ($i = 0; $i < 1000; $i++) {
    $keys[$i] = new Aerospike\Key('test', 'php_bench', "k$i");
}
$key = fn(int $i) => $keys[$i % 1000];

for ($i = 0; $i < $warmup; $i++) {
    $client->put(null, $key($i), $record);
    $client->get(null, $key($i));
}

$wall0 = microtime(true);
$cpu0  = cpu_micros();

$puts = [];
for ($i = 0; $i < $iterations; $i++) {
    $t = hrtime(true);
    $client->put(null, $key($i), $record);
    $puts[] = (hrtime(true) - $t) / 1000.0;
}

$gets = [];
for ($i = 0; $i < $iterations; $i++) {
    $t = hrtime(true);
    $client->get(null, $key($i));
    $gets[] = (hrtime(true) - $t) / 1000.0;
}

$wall = (microtime(true) - $wall0) * 1e6;
$cpu  = cpu_micros() - $cpu0;

report('php put', $puts);
report('php get', $gets);

$ops = $iterations * 2;
printf(
    "\ncpu: %.2f us/op across %d ops (%.1f%% of wall time — high means the build polls)\n",
    $cpu / $ops, $ops, 100.0 * $cpu / $wall
);
