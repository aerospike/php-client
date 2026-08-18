<?php
/**
 * The concurrency premise, tested with real processes.
 *
 *   php -d extension=$(pwd)/target/release/libaerospike_php.dylib tests/concurrency.php
 *
 * `smoke.php` exercises the API in one process. Nothing in it forks, and the
 * whole design of `src/transport.rs` is about what happens when something does:
 * PHP-FPM runs `MINIT` in a master process and forks its worker pool afterwards,
 * and iceoryx2 ports are not fork-safe. The extension attaches lazily and
 * rechecks its pid on every call, and `discard_if_forked` *leaks* inherited
 * state rather than dropping it, because running iceoryx2's destructors in a
 * child would deregister ports the parent is still using.
 *
 * That is a claim about process behaviour, and only a process can check it. This
 * script covers three things a single-process suite structurally cannot:
 *
 * 1. **Fork safety.** A parent that has already attached, forking children that
 *    use the client — including the case FPM actually produces, and the nastier
 *    ones (a child that forks again, a child that touches nothing and exits).
 * 2. **The `max-workers` ceiling.** iceoryx2 fixes `max_clients` when the
 *    service is created, so a pool larger than the daemon's `max-workers`
 *    leaves the extra workers unable to attach *at all*. What matters is that
 *    the failure says so.
 * 3. **Contention.** Many real processes against one daemon at once, which is
 *    the only way to exercise the notifier cache and the ticket map under load.
 *
 * Requires the `pcntl` and `posix` extensions, which is why this is separate
 * from `smoke.php` rather than a section of it: those are CLI-only, and the
 * suite everyone runs should not need them.
 *
 * Environment:
 *   AEROSPIKE_HOSTS      unused here; the daemon owns the cluster
 *   AEROSPIKE_INSTANCE   daemon instance to talk to (default "default")
 *   AEROSPIKE_NAMESPACE  namespace for the test records (default "test")
 *   AEROSPIKE_SET        set for the test records (default "concurrency")
 *   AEROSPIKE_MAX_WORKERS  the daemon's configured `max-workers`. Section 2
 *                        self-skips without it, because attaching one process
 *                        past a ceiling nobody stated is not a test.
 *   CONCURRENCY_WORKERS  processes for section 3 (default 16)
 *   CONCURRENCY_ROUNDS   round trips each (default 200)
 */

use Aerospike\Bin;
use Aerospike\Client;
use Aerospike\Key;

$passed = 0;
$failed = 0;
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

function skip(string $what, string $why): void
{
    global $skipped;
    $skipped++;
    echo "  skip  $what\n        $why\n";
}

function is_true(string $what, bool $condition, string $why = ''): void
{
    $condition ? ok($what) : fail($what, $why !== '' ? $why : 'condition was false');
}

function is_same(string $what, $expected, $actual): void
{
    if ($expected === $actual) {
        ok($what);
        return;
    }
    fail($what, sprintf('expected %s, got %s', var_export($expected, true), var_export($actual, true)));
}

if (!extension_loaded('aerospike-php')) {
    fwrite(STDERR, "the aerospike extension is not loaded\n");
    exit(1);
}
foreach (['pcntl', 'posix'] as $needed) {
    if (!extension_loaded($needed)) {
        fwrite(STDERR, "this script needs the $needed extension (CLI only)\n");
        exit(1);
    }
}

$instance  = getenv('AEROSPIKE_INSTANCE') ?: 'default';
$namespace = getenv('AEROSPIKE_NAMESPACE') ?: 'test';
$set       = getenv('AEROSPIKE_SET') ?: 'concurrency';

/*
 * The ceiling, and what it costs the rest of this script.
 *
 * `max-workers` counts **every attached process, this one included**: iceoryx2
 * releases a worker's slot when its ports are dropped, and this script holds
 * ports for as long as it runs. So a section that forks N children needs
 * `max-workers >= N + 1`, and against the small ceiling `concurrency.sh` uses
 * the other sections have to fork fewer children — otherwise they fail on the
 * ceiling rather than on what they are testing, which reads as a bug in the
 * thing under test.
 *
 * Zero means "not stated": the ceiling section skips, and the others use their
 * full counts because the default of 64 is well above them.
 */
$ceiling = (int) (getenv('AEROSPIKE_MAX_WORKERS') ?: 0);
$roomForChildren = static function (int $wanted) use ($ceiling): int {
    return $ceiling > 0 ? max(1, min($wanted, $ceiling - 1)) : $wanted;
};

/**
 * Run $body in a forked child and return its exit status.
 *
 * stdout is flushed before the fork so the child does not inherit a buffer and
 * reprint the parent's output on exit — which would look like the parent ran
 * twice. A child reports failure through its exit status and stderr, because an
 * assertion counter in a child is a counter the parent never sees.
 */
function child(callable $body): int
{
    @ob_flush();
    flush();
    $pid = pcntl_fork();
    if ($pid === -1) {
        fwrite(STDERR, "fork failed\n");
        exit(1);
    }
    if ($pid === 0) {
        try {
            $body();
        } catch (Throwable $e) {
            fwrite(STDERR, '  child ' . posix_getpid() . ': ' . $e->getMessage() . "\n");
            exit(1);
        }
        // `exit`, not `return`: a child must not run the rest of this script.
        exit(0);
    }
    pcntl_waitpid($pid, $status);
    return pcntl_wifexited($status) ? pcntl_wexitstatus($status) : 255;
}

/** Fork $n children running $body($i), and collect every exit status. */
function children(int $n, callable $body): array
{
    @ob_flush();
    flush();
    $pids = [];
    for ($i = 0; $i < $n; $i++) {
        $pid = pcntl_fork();
        if ($pid === -1) {
            fwrite(STDERR, "fork failed at child $i\n");
            exit(1);
        }
        if ($pid === 0) {
            try {
                $body($i);
            } catch (Throwable $e) {
                fwrite(STDERR, "  child $i (" . posix_getpid() . '): ' . $e->getMessage() . "\n");
                exit(1);
            }
            exit(0);
        }
        $pids[$i] = $pid;
    }
    $statuses = [];
    foreach ($pids as $i => $pid) {
        pcntl_waitpid($pid, $status);
        $statuses[$i] = pcntl_wifexited($status) ? pcntl_wexitstatus($status) : 255;
    }
    return $statuses;
}

echo "concurrency: fork safety, the max-workers ceiling, and contention\n";
echo "instance=$instance namespace=$namespace set=$set php=" . PHP_VERSION . "\n";

// ===== 1. A hard-killed worker's slot, at the ceiling =======================

echo "\na dead worker's slot\n";

/*
 * The question that decides whether a production pool silently degrades: with
 * every `max-workers` slot **taken**, does killing one worker outright free its
 * slot for the replacement?
 *
 * It matters because FPM replaces a dead worker at once. If a slot leaked on each
 * hard kill — the OOM killer, `kill -9`, FPM terminating one that overran — a pool
 * sized at `max-workers` would lose a worker per death until a daemon restart, and
 * `max-workers` would have to be padded purely to absorb them. Measured here:
 * iceoryx2 reclaims a dead process's registration when the next worker creates its
 * node, so it does not leak and no padding is needed.
 *
 * # This section runs first, and that is load-bearing
 *
 * The shape has to be the one PHP-FPM produces: the master process never touches
 * the client — `MINIT` deliberately does not attach — so the pool and every
 * replacement are forked from a process holding no ports. This script becomes such
 * a process only *before* it attaches, which is why this is section 1 and the
 * fork-safety checks that need an attached parent come after.
 *
 * The distinction is real, not pedantic: when the forking process is itself
 * attached, a hard-killed sibling's slot is **not** reclaimed for the replacement.
 * That is a caveat for a CLI script that uses the client and then forks workers —
 * such a script should fork first and let each child attach — and it is not the
 * FPM shape, which is what production runs.
 *
 * Needs the ceiling stated, so there is a bound on how many workers to start, and
 * at least 2 so there is a worker to kill.
 */
if ($ceiling < 2) {
    skip(
        'a hard-killed worker frees its slot at the ceiling',
        $ceiling <= 0
            ? 'set AEROSPIKE_MAX_WORKERS; filling the pool needs a bound on its size'
            : "max-workers is $ceiling, which leaves no room for a pool with a victim in it"
    );
} else {
    /*
     * The exercise runs inside a child that never attaches, standing in for the
     * FPM master, and reports through its exit status.
     *
     * It fills the pool by **starting workers until one is refused** rather than by
     * computing how many should fit. "Full" is the precondition that matters, and
     * reaching it by observation rather than arithmetic means the check does not
     * have to model every slot the host might already be using.
     */
    $barrier = sys_get_temp_dir() . '/aerospike-slot-' . posix_getpid();
    @mkdir($barrier, 0700, true);

    $outcome = child(function () use ($instance, $ceiling, $barrier) {
        // One at a time, waiting for each to report, so "the next one was
        // refused" is a fact about a full pool and not about a race.
        $pool = [];
        for ($i = 0; $i < $ceiling; $i++) {
            $pid = pcntl_fork();
            if ($pid === 0) {
                try {
                    (new Client($instance))->ping();
                } catch (Throwable $e) {
                    file_put_contents("$barrier/refused-$i", '1');
                    exit(2);
                }
                file_put_contents("$barrier/up-$i", '1');
                while (!file_exists("$barrier/release")) {
                    usleep(20_000);
                }
                exit(0);
            }

            $deadline = microtime(true) + 20.0;
            while (
                !file_exists("$barrier/up-$i")
                && !file_exists("$barrier/refused-$i")
                && microtime(true) < $deadline
            ) {
                usleep(10_000);
            }
            if (file_exists("$barrier/refused-$i")) {
                // Full. This one is not part of the pool; reap it and stop.
                pcntl_waitpid($pid, $ignored);
                break;
            }
            if (!file_exists("$barrier/up-$i")) {
                touch("$barrier/release");
                throw new RuntimeException("worker $i neither attached nor was refused within 20s");
            }
            $pool[] = $pid;
        }

        if ($pool === []) {
            touch("$barrier/release");
            throw new RuntimeException('no worker could attach at all, so nothing was being tested');
        }

        // Kill one outright, as the OOM killer does. No destructor runs.
        $victim = array_shift($pool);
        posix_kill($victim, SIGKILL);
        pcntl_waitpid($victim, $status);
        if (!pcntl_wifsignaled($status)) {
            touch("$barrier/release");
            throw new RuntimeException('the victim exited cleanly, so its slot was released normally');
        }

        // The replacement, forked from this never-attached process exactly as FPM
        // forks one.
        $replacement = pcntl_fork();
        if ($replacement === 0) {
            try {
                (new Client($instance))->ping();
                exit(0);
            } catch (Throwable $e) {
                file_put_contents("$barrier/replacement-failed", $e->getMessage());
                exit(2);
            }
        }
        pcntl_waitpid($replacement, $status);
        $code = pcntl_wexitstatus($status);

        touch("$barrier/release");
        foreach ($pool as $pid) {
            pcntl_waitpid($pid, $ignored);
        }
        if ($code !== 0) {
            throw new RuntimeException('the replacement worker could not attach');
        }
    });

    $why = @file_get_contents("$barrier/replacement-failed");
    is_same(
        "with the pool full, a hard-killed worker's replacement attaches",
        0,
        $outcome
    );
    if ($outcome !== 0 && $why) {
        echo "        the replacement said: $why\n";
    }

    foreach (glob("$barrier/*") ?: [] as $file) {
        @unlink($file);
    }
    @rmdir($barrier);
}

// ===== 2. Fork safety =======================================================

echo "\nfork safety\n";

/*
 * The parent attaches first, deliberately. This is the shape PHP-FPM produces
 * when anything touches the client before the fork: the master process owns a
 * node, a request port and an event listener, and every worker inherits the
 * file descriptors and the mapped segments without owning any of them.
 *
 * `MINIT` does not attach, so FPM should not reach this state — but "should not"
 * is the part worth testing, since a single stray call in a preloaded file would
 * put it there and the failure would be silent data corruption rather than an
 * error.
 */
$client = new Client($instance);
$parentKey = new Key($namespace, $set, 'fork-parent');
$client->put(null, $parentKey, [new Bin('owner', posix_getpid())]);
is_same(
    'the parent can use the client before forking',
    posix_getpid(),
    $client->get(null, $parentKey)->bin('owner')
);

// A child that uses the client must re-attach under its own pid. If it reused
// the parent's ports, two processes would be reading one reply stream and the
// answers would cross — so the assertion is that each child reads back *its own*
// value, not merely that nothing threw.
$forked = $roomForChildren(8);
$statuses = children($forked, function (int $i) use ($instance, $namespace, $set) {
    $client = new Client($instance);
    $key = new Key($namespace, $set, "fork-child-$i");
    $mine = posix_getpid();
    for ($round = 0; $round < 20; $round++) {
        $client->put(null, $key, [new Bin('owner', $mine), new Bin('round', $round)]);
        $record = $client->get(null, $key);
        if ($record->bin('owner') !== $mine || $record->bin('round') !== $round) {
            throw new RuntimeException(sprintf(
                'read back owner=%s round=%s, expected %d and %d — replies crossed between processes',
                var_export($record->bin('owner'), true),
                var_export($record->bin('round'), true),
                $mine,
                $round
            ));
        }
    }
});
is_true(
    "all $forked children forked from an attached parent worked",
    array_sum($statuses) === 0,
    'exit statuses: ' . json_encode($statuses)
);

// The parent is the process that owns the real ports, so the interesting
// question is not whether the children worked but whether they broke it. A child
// that ran iceoryx2's destructors on its inherited copy would have deregistered
// this port and decremented these segments' reference counts.
is_same(
    'the parent still works after its children exited',
    posix_getpid(),
    $client->get(null, $parentKey)->bin('owner')
);

// A child that inherits attached state and never uses the client still runs
// MSHUTDOWN on the way out, which calls `transport::teardown`. That path has to
// notice the pid changed too — otherwise the quietest possible child is the one
// that breaks the parent.
is_same(
    'a child that touches nothing exits cleanly',
    0,
    child(function () {
        // Deliberately empty. The point is what PHP does on the way out.
    })
);
is_same(
    'and the parent still works after it',
    posix_getpid(),
    $client->get(null, $parentKey)->bin('owner')
);

// Nested: the grandchild inherits state from a process that had itself inherited
// state. A pid check handles this by construction where a "have I forked?" flag
// would not.
//
// Three processes attach at once here — this one, the child and the grandchild —
// so it needs `max-workers >= 3`. Below that it would fail on the ceiling and
// look like a fork bug.
if ($ceiling > 0 && $ceiling < 3) {
    skip('a grandchild of an attached parent works', "max-workers is $ceiling, and this needs 3 slots");
} else {
is_same(
    'a grandchild of an attached parent works',
    0,
    child(function () use ($instance, $namespace, $set) {
        $client = new Client($instance);
        $client->put(null, new Key($namespace, $set, 'fork-child-nested'), [new Bin('n', 1)]);
        $inner = child(function () use ($instance, $namespace, $set) {
            $client = new Client($instance);
            $key = new Key($namespace, $set, 'fork-grandchild');
            $client->put(null, $key, [new Bin('owner', posix_getpid())]);
            if ($client->get(null, $key)->bin('owner') !== posix_getpid()) {
                throw new RuntimeException('the grandchild read back another process\'s value');
            }
        });
        if ($inner !== 0) {
            throw new RuntimeException("the grandchild exited $inner");
        }
    })
);
is_same(
    'and the parent still works after that',
    posix_getpid(),
    $client->get(null, $parentKey)->bin('owner')
);
}

// A transaction is the one handle whose abandonment costs record locks, so a
// child that opens one and exits without finishing it must still release it: the
// `Drop` that aborts has to run in the child, under the child's own ports.
is_same(
    'a child that abandons a transaction exits cleanly',
    0,
    child(function () use ($instance) {
        $client = new Client($instance);
        $txn = $client->beginTransaction();
        if (!$txn->isOpen()) {
            throw new RuntimeException('a new transaction should be open');
        }
        // No commit, no abort: the destructor must handle it during shutdown.
    })
);

// ===== 3. The max-workers ceiling ===========================================

echo "\nthe max-workers ceiling\n";

/*
 * `max-workers` is iceoryx2's `max_clients`, and iceoryx2 fixes it when the
 * *service* is created — by the daemon, at startup. So it is not a soft limit
 * the daemon could stretch: worker number `max-workers + 1` cannot create a
 * request port at all, and raising the ceiling needs the daemon restarted with
 * no worker attached.
 *
 * That makes it the one setting that silently caps a production FPM pool, and
 * the failure it produces is what an operator has to work from. Attaching one
 * process past a ceiling nobody stated proves nothing, so this needs the number.
 */
if ($ceiling <= 0) {
    skip(
        'the max-workers ceiling',
        'set AEROSPIKE_MAX_WORKERS to the daemon\'s configured value. Use a daemon with a '
        . 'small one (tests/concurrency.sh starts one with max-workers = 4) — proving the '
        . 'boundary against the default of 64 means holding 65 processes attached at once.'
    );
} elseif ($ceiling < 2) {
    skip(
        'the max-workers ceiling',
        "max-workers is $ceiling, and this process already holds a slot — there is no room for "
        . 'a child that could succeed, so the section could not tell a working ceiling from a '
        . 'broken daemon. Use 2 or more.'
    );
} else {
    /*
     * Every child has to hold its attachment while the others attach, or the
     * ceiling is never actually reached: iceoryx2 releases a client slot when
     * the port is dropped, so children that attach and exit in turn would each
     * see a free slot. They synchronise through the filesystem — a barrier
     * directory each child writes into and then waits on — because a pipe from
     * the parent would need the parent to know which child attached first, and
     * the order is exactly what is not determined.
     */
    $barrier = sys_get_temp_dir() . '/aerospike-ceiling-' . posix_getpid();
    @mkdir($barrier, 0700, true);

    /*
     * **This process counts.** It attached at the top of the script and still
     * holds its ports, so it occupies one of the `max-workers` slots. Forking
     * `$ceiling` children therefore makes `$ceiling + 1` attach attempts in
     * total: `$ceiling - 1` children fit alongside this one, and exactly one is
     * refused. Forking `$ceiling + 1` would produce two refusals — which is what
     * the ceiling doing its job looks like, but not a boundary of one.
     */
    $attempts = $ceiling;
    $expectAttached = $ceiling - 1;

    $statuses = children($attempts, function (int $i) use ($instance, $barrier, $attempts) {
        $client = new Client($instance);
        try {
            // Any call attaches; ping is the cheapest and needs no namespace.
            $client->ping();
        } catch (Aerospike\AerospikeException $e) {
            file_put_contents("$barrier/refused-$i", $e->getMessage());
            exit(2);       // 2, not 1: refused is a distinct outcome from broken
        }
        file_put_contents("$barrier/attached-$i", (string) posix_getpid());

        // Hold the attachment until everyone has had their turn, so the slots
        // stay occupied while the last child tries.
        $deadline = microtime(true) + 20.0;
        while (microtime(true) < $deadline) {
            $seen = count(glob("$barrier/attached-*")) + count(glob("$barrier/refused-*"));
            if ($seen >= $attempts) {
                return;
            }
            usleep(20_000);
        }
        throw new RuntimeException('timed out waiting for the other children to report');
    });

    $attached = count(array_filter($statuses, fn($s) => $s === 0));
    $refused  = count(array_filter($statuses, fn($s) => $s === 2));
    $broken   = count(array_filter($statuses, fn($s) => $s !== 0 && $s !== 2));

    is_same(
        "with max-workers = $ceiling, $expectAttached of $attempts children attached alongside this process",
        $expectAttached,
        $attached
    );
    is_same('exactly one was refused', 1, $refused);
    is_same('and none failed some other way', 0, $broken);

    // The whole point of the section: the refusal has to name the setting. An
    // operator seeing an iceoryx2 enum variant has nothing to act on.
    $messages = array_map('file_get_contents', glob("$barrier/refused-*") ?: []);
    $message = $messages[0] ?? '';
    is_true(
        'the refusal names max-workers',
        str_contains($message, 'max-workers'),
        'got: ' . $message
    );
    is_true(
        'and says the daemon has to be restarted to raise it',
        str_contains($message, 'restart'),
        'got: ' . $message
    );

    foreach (glob("$barrier/*") ?: [] as $file) {
        @unlink($file);
    }
    @rmdir($barrier);
}

// ===== 4. Contention ========================================================

echo "\ncontention\n";

/*
 * Many real processes against one daemon. This is the only way to reach the
 * paths that exist *because* of concurrency: the daemon's notifier cache (keyed
 * by client id, bounded, so a worker beyond the bound pays a lookup per reply),
 * the ticket map that pairs a reply with the request that is waiting for it, and
 * the accept batch that bounds how many requests one serving iteration takes.
 *
 * The assertion is correctness, not speed: every process must see its own
 * replies. The throughput line is reported rather than asserted, because a
 * number that depends on the machine is not a pass/fail condition.
 */
$workers = $roomForChildren(max(2, (int) (getenv('CONCURRENCY_WORKERS') ?: 16)));
$rounds  = max(1, (int) (getenv('CONCURRENCY_ROUNDS') ?: 200));

$started = microtime(true);
$statuses = children($workers, function (int $i) use ($instance, $namespace, $set, $rounds) {
    $client = new Client($instance);
    $key = new Key($namespace, $set, "contend-$i");
    $mine = posix_getpid();
    for ($round = 0; $round < $rounds; $round++) {
        // A distinct value per round per process, so a reply delivered to the
        // wrong waiter is caught rather than being indistinguishable.
        $client->put(null, $key, [new Bin('owner', $mine), new Bin('round', $round)]);
        $record = $client->get(null, $key);
        if ($record->bin('owner') !== $mine || $record->bin('round') !== $round) {
            throw new RuntimeException(sprintf(
                'round %d read back owner=%s round=%s — a reply reached the wrong waiter',
                $round,
                var_export($record->bin('owner'), true),
                var_export($record->bin('round'), true)
            ));
        }
    }
});
$elapsed = microtime(true) - $started;

is_true(
    "$workers processes × $rounds round trips each all saw their own replies",
    array_sum($statuses) === 0,
    'exit statuses: ' . json_encode($statuses)
);
printf(
    "        %d processes, %d round trips each: %.2fs wall, %.0f ops/s aggregate\n",
    $workers,
    $rounds * 2,
    $elapsed,
    ($workers * $rounds * 2) / $elapsed
);

// A worker that dies mid-request must not take the daemon with it, and must not
// keep its slot: FPM replaces a dead worker immediately, so a slot that leaked on
// a hard kill would shrink the pool one death at a time.
$killed = pcntl_fork();
if ($killed === 0) {
    $client = new Client($instance);
    $client->ping();
    posix_kill(posix_getpid(), SIGKILL);
    exit(0);
}
pcntl_waitpid($killed, $status);
is_true(
    'a worker killed while attached is a signal death, not a clean exit',
    pcntl_wifsignaled($status),
    'the child exited normally, so this did not test what it claims'
);
is_true(
    'the daemon still serves this process after a worker was killed',
    $client->ping()->version() !== '',
    'the parent could not reach the daemon after a worker was killed'
);

// ===== Cleanup ==============================================================

$client->delete(null, $parentKey);
$client->delete(null, new Key($namespace, $set, 'fork-child-nested'));
$client->delete(null, new Key($namespace, $set, 'fork-grandchild'));
for ($i = 0; $i < $forked; $i++) {
    $client->delete(null, new Key($namespace, $set, "fork-child-$i"));
}
for ($i = 0; $i < $workers; $i++) {
    $client->delete(null, new Key($namespace, $set, "contend-$i"));
}

echo "\n";
if ($failed > 0) {
    echo "FAIL: $passed passed, $failed failed, $skipped skipped\n";
    exit(1);
}
echo "PASS: $passed passed, 0 failed, $skipped skipped\n";
