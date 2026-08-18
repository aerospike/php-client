// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! `php.ini` settings and how they resolve into a per-call policy.
//!
//! The spin/sleep knobs map one-for-one onto [`Backoff`], whose own
//! `Default` supplies the ini defaults — so the documented default in
//! `php.ini` cannot drift away from the contract's.

use std::collections::HashMap;
use std::time::Duration;

use aerospike_php_ipc::{Backoff, DEFAULT_INSTANCE};
use ext_php_rs::flags::IniEntryPermission;
use ext_php_rs::zend::{ExecutorGlobals, IniEntryDef};

/// Cluster instance to use when the constructor names none.
pub const INI_INSTANCE: &str = "aerospike.instance";
/// How long a call waits for the daemon's reply, in milliseconds.
pub const INI_TIMEOUT_MS: &str = "aerospike.timeout_ms";
/// [`Backoff::spin_iters`].
pub const INI_SPIN_ITERS: &str = "aerospike.spin_iters";
/// [`Backoff::yield_iters`].
pub const INI_YIELD_ITERS: &str = "aerospike.yield_iters";
/// [`Backoff::initial_sleep`], in microseconds.
pub const INI_INITIAL_SLEEP_US: &str = "aerospike.initial_sleep_us";
/// [`Backoff::max_sleep`], in microseconds.
pub const INI_MAX_SLEEP_US: &str = "aerospike.max_sleep_us";

/// Default for [`INI_TIMEOUT_MS`].
pub const DEFAULT_TIMEOUT_MS: u32 = 1_000;

/// Upper bound accepted for `aerospike.timeout_ms`. A nonsensically large
/// value would turn a stuck daemon into a wedged worker, so it is clamped
/// rather than honoured.
const MAX_TIMEOUT_MS: u32 = 10 * 60 * 1_000;

/// The ini entries this extension registers, for `MINIT`.
///
/// Everything is `IniEntryPermission::All` so a script can `ini_set()`
/// before constructing a client; see [`resolve`] for exactly when the
/// values are read.
#[must_use]
pub fn ini_entries() -> Vec<IniEntryDef> {
    let backoff = Backoff::default();
    vec![
        entry(INI_INSTANCE, DEFAULT_INSTANCE.to_owned()),
        entry(INI_TIMEOUT_MS, DEFAULT_TIMEOUT_MS.to_string()),
        entry(INI_SPIN_ITERS, backoff.spin_iters.to_string()),
        entry(INI_YIELD_ITERS, backoff.yield_iters.to_string()),
        entry(INI_INITIAL_SLEEP_US, backoff.initial_sleep.as_micros().to_string()),
        entry(INI_MAX_SLEEP_US, backoff.max_sleep.as_micros().to_string()),
    ]
}

fn entry(name: &str, default: String) -> IniEntryDef {
    IniEntryDef::new(name.to_owned(), default, &IniEntryPermission::All)
}

/// Everything a call needs beyond the request itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Reply deadline, as a duration.
    pub timeout: Duration,
    /// The same deadline as the contract carries it, so the daemon can drop
    /// work nobody is waiting for.
    pub timeout_ms: u32,
    /// How a worker waits while the reply is in flight.
    pub backoff: Backoff,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            timeout: Duration::from_millis(u64::from(DEFAULT_TIMEOUT_MS)),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            backoff: Backoff::default(),
        }
    }
}

/// Instance name plus policy, as resolved for one client object.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Which cluster instance the daemon should use.
    pub instance: String,
    /// Timeout and wait policy.
    pub settings: Settings,
}

/// Read the ini table and fold in a constructor-supplied instance name.
///
/// Called once per `new Aerospike\Client(...)`, so an `ini_set()` earlier in
/// the same request is honoured — but changing a value *after* a client
/// exists does not affect it. Reading the whole table is why this is not
/// done per call.
#[must_use]
pub fn resolve(instance_arg: Option<&str>) -> Resolved {
    let ini = ExecutorGlobals::get().ini_values();
    let backoff_default = Backoff::default();

    let instance = instance_arg
        .and_then(non_empty)
        .map(str::to_owned)
        .or_else(|| string_of(&ini, INI_INSTANCE))
        .unwrap_or_else(|| DEFAULT_INSTANCE.to_owned());

    let timeout_ms = u32_of(&ini, INI_TIMEOUT_MS, DEFAULT_TIMEOUT_MS).clamp(1, MAX_TIMEOUT_MS);

    Resolved {
        instance,
        settings: Settings {
            timeout: Duration::from_millis(u64::from(timeout_ms)),
            timeout_ms,
            backoff: Backoff {
                spin_iters: u32_of(&ini, INI_SPIN_ITERS, backoff_default.spin_iters),
                yield_iters: u32_of(&ini, INI_YIELD_ITERS, backoff_default.yield_iters),
                initial_sleep: micros_of(&ini, INI_INITIAL_SLEEP_US, backoff_default.initial_sleep),
                max_sleep: micros_of(&ini, INI_MAX_SLEEP_US, backoff_default.max_sleep),
            },
        },
    }
}

type IniTable = HashMap<String, Option<String>>;

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

fn string_of(ini: &IniTable, name: &str) -> Option<String> {
    ini.get(name)
        .and_then(Option::as_deref)
        .and_then(non_empty)
        .map(str::to_owned)
}

/// An unparseable ini value falls back to the default rather than failing
/// the call: a typo in `php.ini` should not take an application down, and
/// the values here are performance knobs, not correctness ones.
fn u32_of(ini: &IniTable, name: &str, fallback: u32) -> u32 {
    string_of(ini, name)
        .and_then(|raw| raw.parse::<u32>().ok())
        .unwrap_or(fallback)
}

fn micros_of(ini: &IniTable, name: &str, fallback: Duration) -> Duration {
    string_of(ini, name)
        .and_then(|raw| raw.parse::<u64>().ok())
        .map_or(fallback, Duration::from_micros)
}
