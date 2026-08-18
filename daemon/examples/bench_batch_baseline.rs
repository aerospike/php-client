// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The no-IPC baseline for the PHP **batch** benchmark.
//!
//! Talks to the cluster with `aerospike-core` directly, so the difference
//! between this and `ext/tests/bench_batch.php` is the cost of the daemon hop
//! for a batch: one shared-memory frame carrying every row, the daemon's decode
//! and encode of all of them, and whichever wakeup strategy was compiled in.
//!
//! The point of measuring several batch sizes is that the IPC cost is *per
//! call*, not per row, while the encode/decode cost is per row. A one-row batch
//! is nearly all overhead; a thousand-row batch amortises it away. Where the two
//! curves cross is the useful number.
//!
//! ```bash
//! cargo run -p aerospike-php-daemon --release --example bench_batch_baseline
//! ```
//!
//! `AEROSPIKE_HOSTS` (default `127.0.0.1:3000`), `BENCH_ITERATIONS` (default
//! 2000), `BENCH_WARMUP` (default 200) and `BENCH_RECORDS` (default 1000) are
//! read from the environment so every configuration under test uses the same
//! numbers.

use std::time::{Duration, Instant};

use aerospike_core::{
    BatchOperation, BatchPolicy, BatchReadPolicy, BatchWritePolicy, Bin, Bins, Client,
    ClientPolicy, Key, Value, WritePolicy,
};
use aerospike_core::operations::scalar;

/// The batch sizes to measure. One row is nearly all overhead; a thousand
/// amortises it.
const SIZES: [usize; 4] = [1, 10, 100, 1000];

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Mean, median and tail of a latency sample, plus the per-row cost.
///
/// Two throughput figures, because a batch has two: calls per second is what a
/// PHP request sees, and rows per second is what the cluster does.
fn report(label: &str, rows: usize, mut samples: Vec<Duration>) {
    samples.sort_unstable();
    let n = samples.len();
    let micros = |d: Duration| d.as_secs_f64() * 1e6;
    let total: f64 = samples.iter().copied().map(micros).sum();
    let mean = total / n as f64;
    let pick = |q: f64| micros(samples[((n as f64 * q) as usize).min(n - 1)]);
    println!(
        "{label:<18} rows={rows:<5} n={n:<6} mean={mean:>9.2}  p50={:>9.2}  p99={:>9.2}  \
         per-row={:>7.2}  calls/s={:>8.0}  rows/s={:>9.0}",
        pick(0.50),
        pick(0.99),
        mean / rows as f64,
        1e6 / mean,
        1e6 / mean * rows as f64,
    );
}

/// The record every configuration reads: a string, an integer, a small list and
/// a small map, so a row exercises nested decoding rather than being a
/// trivially small frame.
fn bins() -> Vec<Bin> {
    vec![
        Bin::new("name".into(), Value::from("Alice")),
        Bin::new("age".into(), Value::from(30)),
        Bin::new(
            "scores".into(),
            Value::from(vec![Value::from(95), Value::from(87), Value::from(92)]),
        ),
        Bin::new(
            "prefs".into(),
            Value::from(std::collections::HashMap::from([
                (Value::from("theme"), Value::from("dark")),
                (Value::from("lang"), Value::from("en")),
            ])),
        ),
    ]
}

#[tokio::main]
async fn main() {
    let hosts = std::env::var("AEROSPIKE_HOSTS").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let iterations = env_usize("BENCH_ITERATIONS", 2_000);
    let warmup = env_usize("BENCH_WARMUP", 200);
    let records = env_usize("BENCH_RECORDS", 1_000);

    let client = Client::new(&ClientPolicy::default(), &hosts.as_str())
        .await
        .expect("connect");

    println!("baseline: aerospike-core direct, no IPC");
    println!(
        "hosts={hosts} records={records} iterations={iterations} warmup={warmup} \
         sizes={SIZES:?}\n"
    );

    // Seed the set the PHP benchmark reads too, so both measure the same work.
    let wpolicy = WritePolicy::default();
    let record = bins();
    for i in 0..records {
        let key = key_at(i);
        client.put(&wpolicy, &key, &record).await.expect("seed");
    }

    let policy = BatchPolicy::default();
    let read = BatchReadPolicy::default();
    let write = BatchWritePolicy::default();

    for size in SIZES {
        // A fresh row list per call, as PHP has to build one per request.
        let rows_at = |call: usize| -> Vec<BatchOperation> {
            (0..size)
                .map(|row| {
                    BatchOperation::read(&read, key_at(call * size + row + records), Bins::All)
                })
                .collect()
        };

        for call in 0..warmup {
            client.batch(&policy, &rows_at(call)).await.expect("warmup");
        }

        let calls = calls_for(size, iterations);
        let mut samples = Vec::with_capacity(calls);
        for call in 0..calls {
            let rows = rows_at(call);
            let started = Instant::now();
            client.batch(&policy, &rows).await.expect("batch read");
            samples.push(started.elapsed());
        }
        report("core batch read", size, samples);
    }

    // The same shape, writing: a batch write row is an `operate` the batch
    // carries, so this measures the row encoding as well as the transport.
    for size in SIZES {
        let rows_at = |call: usize| -> Vec<BatchOperation> {
            (0..size)
                .map(|row| {
                    BatchOperation::write(
                        &write,
                        key_at(call * size + row + records),
                        vec![scalar::put(&Bin::new("hits".into(), Value::from(1)))],
                    )
                })
                .collect()
        };

        for call in 0..warmup {
            client.batch(&policy, &rows_at(call)).await.expect("warmup");
        }

        let calls = calls_for(size, iterations);
        let mut samples = Vec::with_capacity(calls);
        for call in 0..calls {
            let rows = rows_at(call);
            let started = Instant::now();
            client.batch(&policy, &rows).await.expect("batch write");
            samples.push(started.elapsed());
        }
        report("core batch write", size, samples);
    }
}

/// How many calls to time at this batch size.
///
/// `iterations / size` alone leaves two samples at a thousand rows, which cannot
/// support a p99 — so there is a floor of a hundred calls. That costs about a
/// second at the largest size and buys a percentile worth printing.
fn calls_for(size: usize, iterations: usize) -> usize {
    (iterations / size).max(100)
}

/// Keys cycle through the seeded set, so every row reads a record that is there
/// — a batch of misses would measure a different thing.
fn key_at(i: usize) -> Key {
    let records = env_usize("BENCH_RECORDS", 1_000);
    Key::new(
        "test",
        "php_batch",
        Value::from(format!("k{}", i % records)),
    )
    .expect("key")
}
