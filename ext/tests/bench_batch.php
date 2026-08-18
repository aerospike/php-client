<?php
/**
 * Batch benchmark for the Aerospike PHP client.
 *
 * Reads and writes the same rows as
 * `daemon/examples/bench_batch_baseline.rs`, at the same batch sizes, so the
 * difference between the two is the cost of the daemon hop for a batch.
 *
 * The point of several sizes is that the IPC cost is *per call* while the
 * encode/decode cost is per row: a one-row batch is nearly all overhead, and a
 * thousand-row batch amortises it away.
 *
 * Reports CPU time as well as wall time, because the two wakeup strategies trade
 * exactly that — polling shared memory spends CPU to shave latency, while the
 * event build sleeps and pays a syscall per wakeup.
 *
 *   php -d extension=$(pwd)/target/release/libaerospike_php.dylib tests/bench_batch.php
 *
 * Honours BENCH_ITERATIONS (default 2000), BENCH_WARMUP (default 200) and
 * BENCH_RECORDS (default 1000).
 */

declare(strict_types=1);

use Aerospike\Bin;
use Aerospike\BatchRead;
use Aerospike\BatchWrite;
use Aerospike\Client;
use Aerospike\Key;
use Aerospike\Op;

/** The batch sizes to measure. */
const SIZES = [1, 10, 100, 1000];

$iterations = (int) (getenv('BENCH_ITERATIONS') ?: 2000);
$warmup     = (int) (getenv('BENCH_WARMUP') ?: 200);
$records    = (int) (getenv('BENCH_RECORDS') ?: 1000);

/**
 * Mean, median and tail of a latency sample, plus the per-row cost.
 *
 * Two throughput figures, because a batch has two: calls per second is what a
 * PHP request sees, and rows per second is what the cluster does.
 */
function report(string $label, int $rows, array $samples): void
{
    sort($samples);
    $n = count($samples);
    $mean = array_sum($samples) / $n;
    $pick = fn(float $q) => $samples[min((int) ($n * $q), $n - 1)];
    printf(
        "%-18s rows=%-5d n=%-6d mean=%9.2f  p50=%9.2f  p99=%9.2f  per-row=%7.2f  calls/s=%8.0f  rows/s=%9.0f\n",
        $label, $rows, $n, $mean, $pick(0.50), $pick(0.99),
        $mean / $rows, 1e6 / $mean, 1e6 / $mean * $rows
    );
}

/**
 * How many calls to time at this batch size.
 *
 * `iterations / size` alone leaves two samples at a thousand rows, which cannot
 * support a p99 — so there is a floor of a hundred calls, matching the Rust
 * baseline exactly.
 */
function calls_for(int $size, int $iterations): int
{
    return max(100, intdiv($iterations, $size));
}

/** User + system CPU microseconds consumed by this process so far. */
function cpu_micros(): float
{
    $u = getrusage();
    return ($u['ru_utime.tv_sec'] + $u['ru_stime.tv_sec']) * 1e6
         + $u['ru_utime.tv_usec'] + $u['ru_stime.tv_usec'];
}

$client = new Client();
printf("php client via daemon (version=%s)\n", $client->ping()->version());
printf(
    "records=%d iterations=%d warmup=%d sizes=[%s]\n\n",
    $records, $iterations, $warmup, implode(', ', SIZES)
);

// Keys are built once, outside every loop: constructing one validates it, and
// the baseline does not pay that per call either.
$keys = [];
for ($i = 0; $i < $records; $i++) {
    $keys[$i] = new Key('test', 'php_batch', "k$i");
}

// Seed, in case this runs without the Rust baseline having done it.
$record = [
    new Bin('name', 'Alice'),
    new Bin('age', 30),
    new Bin('scores', [95, 87, 92]),
    new Bin('prefs', ['theme' => 'dark', 'lang' => 'en']),
];
foreach ($keys as $key) {
    $client->put(null, $key, $record);
}

$wall0 = microtime(true);
$cpu0  = cpu_micros();
$totalRows = 0;

foreach (SIZES as $size) {
    // A fresh row list per call, as a PHP request has to build one.
    $rowsAt = function (int $call) use ($size, $keys, $records): array {
        $rows = [];
        for ($row = 0; $row < $size; $row++) {
            $rows[] = BatchRead::all($keys[($call * $size + $row) % $records]);
        }
        return $rows;
    };

    for ($call = 0; $call < $warmup; $call++) {
        $client->batch(null, $rowsAt($call));
    }

    $calls = calls_for($size, $iterations);
    $samples = [];
    for ($call = 0; $call < $calls; $call++) {
        $rows = $rowsAt($call);
        $t = hrtime(true);
        $client->batch(null, $rows);
        $samples[] = (hrtime(true) - $t) / 1000.0;
    }
    $totalRows += $calls * $size;
    report('php batch read', $size, $samples);
}

foreach (SIZES as $size) {
    $rowsAt = function (int $call) use ($size, $keys, $records): array {
        $rows = [];
        for ($row = 0; $row < $size; $row++) {
            $rows[] = BatchWrite::ops(
                $keys[($call * $size + $row) % $records],
                [Op::put(new Bin('hits', 1))]
            );
        }
        return $rows;
    };

    for ($call = 0; $call < $warmup; $call++) {
        $client->batch(null, $rowsAt($call));
    }

    $calls = calls_for($size, $iterations);
    $samples = [];
    for ($call = 0; $call < $calls; $call++) {
        $rows = $rowsAt($call);
        $t = hrtime(true);
        $client->batch(null, $rows);
        $samples[] = (hrtime(true) - $t) / 1000.0;
    }
    $totalRows += $calls * $size;
    report('php batch write', $size, $samples);
}

$wall = (microtime(true) - $wall0) * 1e6;
$cpu  = cpu_micros() - $cpu0;

printf(
    "\ncpu: %.2f us/row across %d rows (%.1f%% of wall time — high means the build polls)\n",
    $cpu / $totalRows, $totalRows, 100.0 * $cpu / $wall
);
