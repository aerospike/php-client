// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The no-IPC baseline for the PHP client benchmark.
//!
//! Talks to the cluster with `aerospike-core` directly, so the difference
//! between this and `ext/tests/bench.php` is the cost of the daemon hop:
//! shared-memory transfer, the daemon's decode/encode, and whichever wakeup
//! strategy was compiled in.
//!
//! Deliberately sequential — one operation at a time, awaiting each — because
//! PHP is synchronous and a concurrent baseline would not be comparable.
//!
//! ```bash
//! cargo run -p aerospike-php-daemon --release --example bench_core_baseline
//! ```
//!
//! `AEROSPIKE_HOSTS` (default `127.0.0.1:3000`), `BENCH_ITERATIONS`
//! (default 20000) and `BENCH_WARMUP` (default 2000) are read from the
//! environment so every configuration under test uses the same numbers.

use std::time::{Duration, Instant};

use aerospike_core::{Bin, Bins, Client, ClientPolicy, Key, ReadPolicy, Value, WritePolicy};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Mean, median and tail of a latency sample, in microseconds.
fn report(label: &str, mut samples: Vec<Duration>) {
    samples.sort_unstable();
    let n = samples.len();
    let micros = |d: Duration| d.as_secs_f64() * 1e6;
    let total: f64 = samples.iter().copied().map(micros).sum();
    let pick = |q: f64| micros(samples[((n as f64 * q) as usize).min(n - 1)]);
    println!(
        "{label:<12} n={n:<7} mean={:>8.2}  p50={:>8.2}  p99={:>8.2}  p999={:>8.2}  ops/s={:>9.0}",
        total / n as f64,
        pick(0.50),
        pick(0.99),
        pick(0.999),
        1e6 / (total / n as f64),
    );
}

/// The record every configuration writes: a string, an integer, a small list
/// and a small map, so the payload exercises nested encoding rather than being
/// a trivially small frame.
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
    let iterations = env_usize("BENCH_ITERATIONS", 20_000);
    let warmup = env_usize("BENCH_WARMUP", 2_000);

    let client = Client::new(&ClientPolicy::default(), &hosts.as_str())
        .await
        .expect("connect");
    let wpolicy = WritePolicy::default();
    let rpolicy = ReadPolicy::default();
    let record = bins();

    println!("baseline: aerospike-core direct, no IPC");
    println!("hosts={hosts} iterations={iterations} warmup={warmup}\n");

    let key_at = |i: usize| {
        Key::new("test", "php_bench", Value::from(format!("k{}", i % 1_000)))
            .expect("key")
    };

    for i in 0..warmup {
        let key = key_at(i);
        client.put(&wpolicy, &key, &record).await.expect("warmup put");
        client.get(&rpolicy, &key, Bins::All).await.expect("warmup get");
    }

    let mut puts = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let key = key_at(i);
        let started = Instant::now();
        client.put(&wpolicy, &key, &record).await.expect("put");
        puts.push(started.elapsed());
    }

    let mut gets = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let key = key_at(i);
        let started = Instant::now();
        let _ = client.get(&rpolicy, &key, Bins::All).await.expect("get");
        gets.push(started.elapsed());
    }

    report("core put", puts);
    report("core get", gets);
}
