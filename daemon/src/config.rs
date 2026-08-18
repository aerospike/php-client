// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The daemon's TOML configuration.
//!
//! # Shape
//!
//! ```toml
//! [daemon]           # how the daemon itself runs
//! [defaults]         # cluster keys inherited by every cluster
//! [cluster.default]  # one section per cluster; the name is what PHP asks for
//! [cluster.analytics]
//! ```
//!
//! Cluster keys are declared exactly once, in [`ClusterConfig`], with
//! `deny_unknown_fields` so a typo is a startup error rather than a setting
//! that silently does nothing. `[defaults]` is validated against the same
//! struct and then merged *under* each cluster section, so inheritance costs
//! no extra declarations and a typo in `[defaults]` is caught too.
//!
//! A bare `[cluster]` section carrying cluster keys directly is accepted as
//! `[cluster.default]`. A table inside `[cluster]` is a named cluster and
//! anything else is a key of the default one — with two exceptions, `tls` and
//! `ip-map`, which are themselves table-valued cluster keys and so are named in
//! `TABLE_VALUED_CLUSTER_KEYS`. A cluster genuinely called `tls` has to be
//! written under an explicit `[cluster.<name>]` layout.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aerospike_core::policy::{AuthMode, ClientPolicy};
use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;

/// Where the daemon looks for its configuration when `--config` is absent.
pub const DEFAULT_CONFIG_PATH: &str = "./aerospike-daemon.toml";

/// Cluster keys the daemon parses but deliberately does not apply. Accepting
/// them keeps configurations that carry them valid; they are reported once at
/// startup so nobody believes they took effect.
pub const IGNORED_KEYS: [&str; 2] = [
    "limit-connections-to-queue-size",
    "ignore-other-subnet-aliases",
];

/// Cluster keys whose value is a TOML table.
///
/// The bare `[cluster]` form tells a *named cluster* from a *key of the default
/// cluster* by whether the value is a table — which works only because almost no
/// cluster key is table-valued. These two are, so they have to be named:
/// `[cluster.tls]` means TLS on the default cluster, not a cluster called `tls`.
///
/// Both spellings, since either may appear in a file.
const TABLE_VALUED_CLUSTER_KEYS: [&str; 3] = ["tls", "ip-map", "ip_map"];

/// A configuration that could not be loaded. Always carries a message naming
/// the offending section or key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(String);

impl ConfigError {
    fn new(message: impl Into<String>) -> ConfigError {
        ConfigError(message.into())
    }

    /// The failure detail.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

type Result<T> = std::result::Result<T, ConfigError>;

// ===== Durations ============================================================

/// A duration expressed in milliseconds.
///
/// Accepts either a bare integer (milliseconds, so `500` and `"500ms"` agree)
/// or a duration string: `"30s"`, `"1s500ms"`, `"250ms"`, `"0"`. Milliseconds
/// are the unit every `ClientPolicy` timeout uses, so this is the one place
/// that conversion happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Millis(pub u32);

impl Millis {
    /// The value as a `Duration`.
    #[must_use]
    pub const fn as_duration(self) -> Duration {
        Duration::from_millis(self.0 as u64)
    }
}

impl fmt::Display for Millis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ms", self.0)
    }
}

/// Parse a duration string into whole milliseconds.
///
/// Understands `ns`, `us`, `ms`, `s`, `m` and `h` suffixes, a sequence of them
/// (`"1m30s"`), and a plain number (milliseconds). Sub-millisecond components
/// are rounded down, so `"999us"` is `0` — the client's timeouts have
/// millisecond resolution and pretending otherwise would be a lie.
///
/// # Errors
/// A message naming what could not be parsed.
pub fn parse_duration_ms(text: &str) -> Result<u32> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::new("empty duration"));
    }

    let mut total_nanos: u128 = 0;
    let mut rest = trimmed;
    let mut segments = 0;

    while !rest.is_empty() {
        let digits_end = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        if digits_end == 0 {
            return Err(ConfigError::new(format!(
                "invalid duration '{text}': expected a number before the unit"
            )));
        }
        let (number, tail) = rest.split_at(digits_end);
        let unit_end = tail
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(tail.len());
        let (unit, tail) = tail.split_at(unit_end);

        let value: f64 = number.parse().map_err(|_| {
            ConfigError::new(format!("invalid duration '{text}': '{number}' is not a number"))
        })?;
        let nanos_per_unit: f64 = match unit.trim() {
            // No unit is only allowed as the whole value, so that "1s500"
            // cannot be read as "1s500ms" by accident.
            "" if segments == 0 && tail.is_empty() => 1_000_000.0,
            "ns" => 1.0,
            "us" | "µs" => 1_000.0,
            "ms" => 1_000_000.0,
            "s" => 1_000_000_000.0,
            "m" => 60.0 * 1_000_000_000.0,
            "h" => 3_600.0 * 1_000_000_000.0,
            other => {
                return Err(ConfigError::new(format!(
                    "invalid duration '{text}': unknown unit '{other}' \
                     (use ns, us, ms, s, m or h)"
                )))
            }
        };
        total_nanos += (value * nanos_per_unit) as u128;
        rest = tail;
        segments += 1;
    }

    let millis = total_nanos / 1_000_000;
    u32::try_from(millis)
        .map_err(|_| ConfigError::new(format!("duration '{text}' does not fit in 32 bits of ms")))
}

impl<'de> Deserialize<'de> for Millis {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Millis, D::Error> {
        struct MillisVisitor;

        impl<'v> Visitor<'v> for MillisVisitor {
            type Value = Millis;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a duration string like \"30s\" or a number of milliseconds")
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Millis, E> {
                u32::try_from(v)
                    .map(Millis)
                    .map_err(|_| E::custom(format!("{v} ms does not fit in 32 bits")))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Millis, E> {
                if v < 0 {
                    return Err(E::custom(format!("negative duration {v}")));
                }
                self.visit_u64(v as u64)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Millis, E> {
                parse_duration_ms(v).map(Millis).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(MillisVisitor)
    }
}

// ===== [daemon] =============================================================

/// The `[daemon]` section: how this process runs, independent of any cluster.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonConfig {
    /// IPC service instance name. Part of the iceoryx2 service name, so it is
    /// what pairs a daemon with the workers meant to talk to it; two daemons
    /// on one host must not share it.
    #[serde(default = "default_instance", alias = "instance-name")]
    pub instance: String,

    /// Tokio worker threads. `None` leaves tokio's default (one per core).
    #[serde(default, alias = "worker-threads")]
    pub worker_threads: Option<usize>,

    /// Deadline applied to a request that declares no timeout of its own.
    #[serde(default = "default_default_timeout", alias = "default-timeout")]
    pub default_timeout: Millis,

    /// Log level: `error`, `warn`, `info`, `debug` or `trace`. `RUST_LOG`, if
    /// set, wins.
    #[serde(default = "default_log_level", alias = "log-level")]
    pub log_level: String,

    /// Records one page of a scan or query holds when the caller asks for no
    /// particular size.
    ///
    /// A page is held in this process before it is sent, so this is the memory
    /// one traversal costs while a page is in flight.
    #[serde(default = "default_page_size", alias = "page-size")]
    pub page_size: u32,

    /// How long a scan or query cursor may sit unused before it is reclaimed.
    ///
    /// This is what bounds the cost of a PHP worker that is killed mid-scan: its
    /// cursor is never closed, so it has to expire. Long enough that a slow
    /// consumer is not cut off, short enough that abandoned traversals do not
    /// accumulate.
    #[serde(default = "default_cursor_idle", alias = "cursor-idle-timeout")]
    pub cursor_idle_timeout: Millis,

    /// How many scan or query cursors may be open at once, across every worker.
    ///
    /// A backstop, not a tuning knob: reaching it means workers are abandoning
    /// traversals faster than they expire, and refusing the next one is better
    /// than growing without bound.
    #[serde(default = "default_max_cursors", alias = "max-cursors")]
    pub max_cursors: usize,

    /// How long a multi-record transaction may sit unused before the daemon
    /// **aborts** it.
    ///
    /// Not merely forgets it: an open transaction holds record locks on the
    /// server, so every other writer of those records waits behind an abandoned
    /// one. Shorter than `cursor_idle_timeout` for exactly that reason — an idle
    /// cursor costs memory, an idle transaction costs other writers.
    #[serde(default = "default_txn_idle", alias = "txn-idle-timeout")]
    pub txn_idle_timeout: Millis,

    /// How many multi-record transactions may be open at once, across every
    /// worker.
    #[serde(default = "default_max_txns", alias = "max-transactions")]
    pub max_transactions: usize,

    // --- The IPC serving loop -----------------------------------------------
    /// How many PHP workers may be attached at once.
    ///
    /// **The one to raise on a busy host.** It is the iceoryx2 service's client
    /// limit, fixed when the service is created, so a pool larger than this
    /// leaves the extra workers unable to attach at all. A PHP-FPM pool of 200
    /// needs at least 200 here; the default suits development.
    #[serde(default = "default_max_workers", alias = "max-workers")]
    pub max_workers: usize,

    /// How many requests one turn of the serving loop accepts before it goes
    /// back to writing replies.
    ///
    /// Bounds how long a burst of arrivals can starve the replies for work
    /// already done. Larger favours throughput under load, smaller favours
    /// latency fairness.
    #[serde(default = "default_accept_batch", alias = "accept-batch")]
    pub accept_batch: usize,

    /// How long a shutdown waits for work already in flight before abandoning
    /// it.
    #[serde(default = "default_drain_timeout", alias = "drain-timeout")]
    pub drain_timeout: Millis,

    /// How many per-worker event notifiers to keep open.
    ///
    /// Only used by the default (event) build, where the daemon notifies a
    /// worker that its reply is ready. Opening one costs a syscall, so they are
    /// cached; the cache evicts the oldest past this size. No effect under
    /// `shm-poll`, where nothing is notified.
    #[serde(default = "default_notifier_cache", alias = "notifier-cache")]
    pub notifier_cache: usize,

    /// How the serving loop waits when no request has arrived.
    ///
    /// Its own section because the daemon's wait is **not** the worker's: a
    /// worker waits for one reply and can block, while iceoryx2
    /// request/response ports cannot be attached to a `WaitSet`, so this loop has
    /// no choice but to poll. Measured, that costs ~255µs of CPU per operation
    /// whichever wakeup mode the worker uses — which is exactly why it deserves
    /// tuning independently of the worker's `aerospike.*` ini settings.
    #[serde(default, alias = "receive-backoff")]
    pub receive_backoff: BackoffConfig,
}

/// A tiered wait: spin, then yield, then sleep in doubling steps to a ceiling.
///
/// The defaults are [`aerospike_php_ipc::Backoff`]'s, which were tuned against a
/// measured ~180µs local round trip: spin long enough to cover one, yield for a
/// while after that, and treat sleeping as a last resort with a low ceiling.
///
/// Raise the sleep tiers for a **remote** cluster, where a round trip is
/// milliseconds rather than microseconds and spinning through it wastes a core.
/// Lower `max-sleep` to shorten how long an idle daemon can take to notice a
/// request.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct BackoffConfig {
    /// Iterations to spin with a busy hint before yielding.
    #[serde(default = "default_spin_iters", alias = "spin_iters")]
    pub spin_iters: u32,
    /// Iterations to yield the thread before sleeping.
    #[serde(default = "default_yield_iters", alias = "yield_iters")]
    pub yield_iters: u32,
    /// First sleep once yielding is exhausted, in **microseconds**.
    #[serde(default = "default_initial_sleep_us", alias = "initial_sleep_us")]
    pub initial_sleep_us: u32,
    /// Ceiling the sleep doubles up to, in **microseconds**.
    #[serde(default = "default_max_sleep_us", alias = "max_sleep_us")]
    pub max_sleep_us: u32,
}

impl BackoffConfig {
    /// The contract's backoff.
    ///
    /// Microseconds rather than the [`Millis`] every other duration in this file
    /// uses, and named `*-us` for it: the tiers are *tens* of microseconds, so a
    /// value that could only say "1ms" could not express the default — and the
    /// extension's own `aerospike.initial_sleep_us` ini setting is spelled the
    /// same way, for the same reason.
    #[must_use]
    pub const fn to_backoff(&self) -> aerospike_php_ipc::Backoff {
        aerospike_php_ipc::Backoff {
            spin_iters: self.spin_iters,
            yield_iters: self.yield_iters,
            initial_sleep: Duration::from_micros(self.initial_sleep_us as u64),
            max_sleep: Duration::from_micros(self.max_sleep_us as u64),
        }
    }

    /// Refuse a ladder that cannot escalate.
    ///
    /// # Errors
    /// [`ConfigError`] when the ceiling is below the first step: the sleep doubles
    /// *up to* the ceiling, so a ceiling underneath it would silently clamp every
    /// sleep to the smaller value and make the first tier meaningless.
    pub fn validate(&self) -> Result<()> {
        if self.max_sleep_us < self.initial_sleep_us {
            return Err(ConfigError::new(format!(
                "receive-backoff max-sleep-us ({}) is below initial-sleep-us ({}); the sleep \
                 doubles up to the ceiling, so a lower ceiling would clamp every sleep to it",
                self.max_sleep_us, self.initial_sleep_us
            )));
        }
        Ok(())
    }
}

impl Default for BackoffConfig {
    fn default() -> BackoffConfig {
        BackoffConfig {
            spin_iters: default_spin_iters(),
            yield_iters: default_yield_iters(),
            initial_sleep_us: default_initial_sleep_us(),
            max_sleep_us: default_max_sleep_us(),
        }
    }
}

fn default_instance() -> String {
    aerospike_php_ipc::DEFAULT_INSTANCE.to_string()
}

fn default_default_timeout() -> Millis {
    Millis(1_000)
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_page_size() -> u32 {
    crate::query::DEFAULT_PAGE_SIZE
}

fn default_cursor_idle() -> Millis {
    Millis(u32::try_from(crate::query::DEFAULT_CURSOR_IDLE.as_millis()).unwrap_or(60_000))
}

fn default_max_cursors() -> usize {
    crate::query::DEFAULT_MAX_CURSORS
}

fn default_txn_idle() -> Millis {
    Millis(u32::try_from(crate::txn::DEFAULT_TXN_IDLE.as_millis()).unwrap_or(30_000))
}

fn default_max_txns() -> usize {
    crate::txn::DEFAULT_MAX_TXNS
}

fn default_max_workers() -> usize {
    crate::server::DEFAULT_MAX_WORKERS
}

fn default_accept_batch() -> usize {
    crate::server::DEFAULT_ACCEPT_BATCH
}

fn default_drain_timeout() -> Millis {
    Millis(u32::try_from(crate::server::DEFAULT_DRAIN_TIMEOUT.as_millis()).unwrap_or(10_000))
}

fn default_notifier_cache() -> usize {
    crate::server::DEFAULT_NOTIFIER_CACHE
}

/// The contract's own backoff, so the file's defaults and the code's cannot
/// drift.
fn default_spin_iters() -> u32 {
    aerospike_php_ipc::Backoff::default().spin_iters
}

fn default_yield_iters() -> u32 {
    aerospike_php_ipc::Backoff::default().yield_iters
}

fn default_initial_sleep_us() -> u32 {
    u32::try_from(aerospike_php_ipc::Backoff::default().initial_sleep.as_micros()).unwrap_or(10)
}

fn default_max_sleep_us() -> u32 {
    u32::try_from(aerospike_php_ipc::Backoff::default().max_sleep.as_micros()).unwrap_or(50)
}

impl DaemonConfig {
    /// The serving loop's tunables.
    #[must_use]
    pub fn tuning(&self) -> crate::server::Tuning {
        crate::server::Tuning {
            max_workers: self.max_workers.max(1),
            accept_batch: self.accept_batch.max(1),
            drain_timeout: self.drain_timeout.as_duration(),
            notifier_cache: self.notifier_cache.max(1),
            backoff: self.receive_backoff.to_backoff(),
        }
    }

    /// Refuse settings that would leave the daemon unable to serve.
    ///
    /// # Errors
    /// [`ConfigError`] for a worker limit of zero — a service no worker can
    /// attach to is not a running daemon — and for a backoff ladder that cannot
    /// escalate.
    pub fn validate(&self) -> Result<()> {
        if self.max_workers == 0 {
            return Err(ConfigError::new(
                "max-workers must be at least 1; a service with room for no client is one no \
                 PHP worker can attach to",
            ));
        }
        self.receive_backoff.validate()
    }
}

impl Default for DaemonConfig {
    fn default() -> DaemonConfig {
        DaemonConfig {
            instance: default_instance(),
            worker_threads: None,
            default_timeout: default_default_timeout(),
            log_level: default_log_level(),
            page_size: default_page_size(),
            cursor_idle_timeout: default_cursor_idle(),
            max_cursors: default_max_cursors(),
            txn_idle_timeout: default_txn_idle(),
            max_transactions: default_max_txns(),
            max_workers: default_max_workers(),
            accept_batch: default_accept_batch(),
            drain_timeout: default_drain_timeout(),
            notifier_cache: default_notifier_cache(),
            receive_backoff: BackoffConfig::default(),
        }
    }
}

// ===== [cluster.<name>.tls] =================================================

/// TLS for one cluster.
///
/// Its presence is what enables TLS — there is no `enabled` flag, because a
/// `[cluster.x.tls]` section with nothing in it would then be ambiguous, and
/// because the CA is not optional in any deployment worth calling configured.
///
/// # What this covers, and what it does not
///
/// Two things, which is what a client actually needs:
///
/// - **which certificates to trust** — a CA file, and optionally the platform's
///   own root store for a cluster whose certificate came from a public CA;
/// - **which certificate to present** — a client certificate and key, for mutual
///   TLS and for `PKI` authentication, where the certificate *is* the identity.
///
/// Not covered, deliberately: cipher-suite and protocol-version selection
/// (rustls picks safe defaults and refuses unsafe ones, so there is nothing
/// useful to choose), revocation checking, and any "skip verification" switch.
/// That last one is an omission rather than an oversight: it needs a custom
/// certificate verifier, and a daemon that can be configured not to check
/// certificates is one whose TLS means nothing.
///
/// # The TLS name is part of the seed, not of this section
///
/// A certificate is verified against a *name*, and each seed carries its own:
/// `host = "node1:node1.cluster.example:4333"`. That is where a TLS name goes,
/// because different nodes can legitimately have different ones.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct TlsConfig {
    /// PEM file holding the certificate authority — or chain of them — that
    /// signed the cluster's certificates.
    ///
    /// Required unless [`use_platform_roots`](Self::use_platform_roots) is set:
    /// an empty trust store would reject every node, which is a failure worth
    /// having at startup rather than on the first connection.
    #[serde(alias = "ca_file")]
    pub ca_file: Option<PathBuf>,

    /// Also trust the platform's bundled root certificates.
    ///
    /// For a cluster whose certificate came from a public CA. An Aerospike
    /// cluster normally uses a private one, which needs only
    /// [`ca_file`](Self::ca_file) — so this is off by default, and turning it on
    /// widens what the daemon will trust.
    #[serde(default, alias = "use_platform_roots")]
    pub use_platform_roots: bool,

    /// PEM file holding the certificate this daemon presents to the cluster.
    ///
    /// Needed for mutual TLS, and **required for `auth = 'PKI'`**, where the
    /// server takes the user's identity from this certificate rather than from a
    /// username. Must be given together with [`key_file`](Self::key_file).
    #[serde(alias = "cert_file")]
    pub cert_file: Option<PathBuf>,

    /// PEM file holding the private key for [`cert_file`](Self::cert_file).
    #[serde(alias = "key_file")]
    pub key_file: Option<PathBuf>,
}

impl TlsConfig {
    /// Build the `rustls` configuration the client policy carries.
    ///
    /// # Errors
    /// [`ConfigError`] for a missing or unreadable file, a PEM file with no
    /// certificate in it, a certificate without its key (or the reverse), and an
    /// empty trust store — each named, because "TLS failed" is the least useful
    /// thing this could say.
    pub fn to_rustls(&self) -> Result<rustls::ClientConfig> {
        use rustls::pki_types::pem::PemObject;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let mut roots = rustls::RootCertStore::empty();
        if self.use_platform_roots {
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }
        if let Some(path) = &self.ca_file {
            let certs: std::result::Result<Vec<CertificateDer>, _> =
                CertificateDer::pem_file_iter(path)
                    .map_err(|e| {
                        ConfigError::new(format!("cannot read tls ca-file {}: {e}", path.display()))
                    })?
                    .collect();
            let certs = certs.map_err(|e| {
                ConfigError::new(format!(
                    "tls ca-file {} is not a readable PEM certificate: {e}",
                    path.display()
                ))
            })?;
            if certs.is_empty() {
                return Err(ConfigError::new(format!(
                    "tls ca-file {} holds no certificate; a trust store with nothing in it \
                     would reject every node",
                    path.display()
                )));
            }
            // `add_parsable_certificates` reports how many it took, and a
            // certificate it could not parse is exactly the case worth naming
            // rather than silently trusting a shorter chain.
            let added = roots.add_parsable_certificates(certs.clone());
            if added.0 == 0 {
                return Err(ConfigError::new(format!(
                    "none of the {} certificate(s) in tls ca-file {} could be used as a trust \
                     anchor",
                    certs.len(),
                    path.display()
                )));
            }
        }
        if roots.is_empty() {
            return Err(ConfigError::new(
                "a tls section needs 'ca-file', or 'use-platform-roots = true' for a cluster \
                 whose certificate came from a public CA; with neither there is nothing to \
                 verify the cluster against",
            ));
        }

        let builder = rustls::ClientConfig::builder().with_root_certificates(roots);

        match (&self.cert_file, &self.key_file) {
            (Some(cert_path), Some(key_path)) => {
                let certs: std::result::Result<Vec<CertificateDer>, _> =
                    CertificateDer::pem_file_iter(cert_path)
                        .map_err(|e| {
                            ConfigError::new(format!(
                                "cannot read tls cert-file {}: {e}",
                                cert_path.display()
                            ))
                        })?
                        .collect();
                let certs = certs.map_err(|e| {
                    ConfigError::new(format!(
                        "tls cert-file {} is not a readable PEM certificate: {e}",
                        cert_path.display()
                    ))
                })?;
                if certs.is_empty() {
                    return Err(ConfigError::new(format!(
                        "tls cert-file {} holds no certificate",
                        cert_path.display()
                    )));
                }
                let key = PrivateKeyDer::from_pem_file(key_path).map_err(|e| {
                    ConfigError::new(format!(
                        "cannot read tls key-file {}: {e}",
                        key_path.display()
                    ))
                })?;
                builder.with_client_auth_cert(certs, key).map_err(|e| {
                    ConfigError::new(format!(
                        "the certificate in {} and the key in {} do not form a usable client \
                         identity: {e}",
                        cert_path.display(),
                        key_path.display()
                    ))
                })
            }
            // Half an identity is a configuration mistake, not a fallback to
            // none: the server would refuse the connection and say nothing
            // about which half was missing.
            (Some(_), None) => Err(ConfigError::new(
                "tls cert-file is set without key-file; a client certificate is useless \
                 without its private key",
            )),
            (None, Some(_)) => Err(ConfigError::new(
                "tls key-file is set without cert-file; a private key is useless without the \
                 certificate it belongs to",
            )),
            (None, None) => Ok(builder.with_no_client_auth()),
        }
    }

    /// Whether this section names a client certificate, which `PKI`
    /// authentication requires.
    #[must_use]
    pub const fn has_client_identity(&self) -> bool {
        self.cert_file.is_some()
    }
}

// ===== [cluster.<name>] =====================================================

/// One cluster's configuration: every key PHP can influence, in the file's
/// kebab-case spelling. Each field is optional so that `[defaults]` can be
/// validated against this same struct.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
#[allow(clippy::struct_excessive_bools)] // it is a bag of settings
pub struct ClusterConfig {
    /// Seed host(s), comma-separated. `"host"`, `"host:port"` and
    /// `"host:tls-name:port"` forms are accepted; the default port is 3000.
    pub host: Option<String>,
    /// Seed hosts as an array, appended after `host`.
    pub hosts: Option<Vec<String>>,

    /// Username for cluster authentication.
    pub user: Option<String>,
    /// Password for cluster authentication.
    pub password: Option<String>,
    /// `INTERNAL`, `EXTERNAL`, `EXTERNAL_INSECURE` or `PKI`, case-insensitive.
    pub auth: Option<String>,

    /// Expected cluster name; empty means "do not check".
    #[serde(alias = "cluster_name")]
    pub cluster_name: Option<String>,

    /// TLS for this cluster. Present enables it; absent leaves the connection
    /// in the clear.
    ///
    /// A sub-table rather than five prefixed keys, because TLS is one decision
    /// with parts — and because its presence is what switches it on, which a set
    /// of independent optional keys could not express.
    pub tls: Option<TlsConfig>,

    /// Socket timeout for info/admin commands.
    pub timeout: Option<Millis>,
    /// Timeout for **opening** a connection, as distinct from using one.
    ///
    /// `0` means "use `timeout`", which is the client's own default. Worth
    /// setting separately on a link where the TCP handshake and the TLS one are
    /// slow but established connections are not.
    #[serde(alias = "connect_timeout")]
    pub connect_timeout: Option<Millis>,
    /// How long an idle connection may sit in the pool. `0` disables reaping.
    #[serde(alias = "idle_timeout")]
    pub idle_timeout: Option<Millis>,
    /// Timeout for the login/authentication exchange.
    #[serde(alias = "login_timeout")]
    pub login_timeout: Option<Millis>,
    /// Interval between cluster tends. Minimum 250ms.
    #[serde(alias = "tend_interval")]
    pub tend_interval: Option<Millis>,
    /// How often the client re-reads dynamic server configuration, in
    /// milliseconds. `0` disables it.
    #[serde(alias = "config_interval")]
    pub config_interval: Option<Millis>,

    /// Maximum connections per node (`ClientPolicy::max_conns_per_node`).
    #[serde(alias = "connection_queue_size")]
    pub connection_queue_size: Option<usize>,
    /// Connections held open per node even when idle.
    #[serde(alias = "min_connections_per_node")]
    pub min_connections_per_node: Option<usize>,
    /// Independent connection pools per node.
    ///
    /// More pools means less contention between threads competing for a
    /// connection, at the cost of more idle sockets: the per-node ceiling is
    /// divided between them. `1` is right unless a profile shows pool contention.
    #[serde(alias = "conn_pools_per_node")]
    pub conn_pools_per_node: Option<u8>,

    /// Errors tolerated per node per window before its breaker trips.
    #[serde(alias = "max_error_rate")]
    pub max_error_rate: Option<usize>,
    /// Tend iterations after which a node's error counter resets.
    #[serde(alias = "error_rate_window")]
    pub error_rate_window: Option<usize>,
    /// Concurrent in-flight connection opens per node before callers wait.
    #[serde(alias = "opening_connection_threshold")]
    pub opening_connection_threshold: Option<usize>,

    /// Whether `Client::new` fails when no seed can be reached.
    #[serde(alias = "fail_if_not_connected")]
    pub fail_if_not_connected: Option<bool>,
    /// Use the `services-alternate` node list during tending.
    #[serde(alias = "use_services_alternate")]
    pub use_services_alternate: Option<bool>,
    /// Restrict the cluster view to the seeds; disables peer discovery.
    #[serde(alias = "seed_only_cluster")]
    pub seed_only_cluster: Option<bool>,

    /// Preferred racks, in preference order. Empty means "no rack awareness".
    #[serde(alias = "rack_ids")]
    pub rack_ids: Option<Vec<usize>>,
    /// Rack awareness switch. The client has no such field — it infers rack
    /// awareness from `rack-ids` being set — so `false` clears `rack-ids` and
    /// `true` without `rack-ids` is a configuration error.
    #[serde(alias = "rack_aware")]
    pub rack_aware: Option<bool>,

    /// Rewrite discovered node addresses, `"from" = "to"`.
    ///
    /// For a cluster the daemon reaches through different addresses than the
    /// nodes advertise to each other and where `use-services-alternate` is not
    /// enough — a NAT with per-node mappings, for instance.
    #[serde(alias = "ip_map")]
    pub ip_map: Option<HashMap<String, String>>,

    /// Buffer above which a command's read buffer is released rather than kept
    /// for reuse, in bytes. Bounds the memory one unusually large record can
    /// pin per connection.
    #[serde(alias = "buffer_reclaim_threshold")]
    pub buffer_reclaim_threshold: Option<usize>,
    /// Whether commands take their buffers from the shared tiered pool.
    #[serde(alias = "use_buffer_pool")]
    pub use_buffer_pool: Option<bool>,
    /// Smallest buffer the pool keeps, in bytes. **Must be a power of two.**
    #[serde(alias = "buffer_pool_min_size")]
    pub buffer_pool_min_size: Option<usize>,
    /// Largest buffer the pool keeps, in bytes. **Must be a power of two**, and
    /// at least the minimum.
    #[serde(alias = "buffer_pool_max_size")]
    pub buffer_pool_max_size: Option<usize>,
    /// Total bytes the pool may hold per size tier.
    #[serde(alias = "buffer_pool_tier_bytes")]
    pub buffer_pool_tier_bytes: Option<usize>,

    /// Application name the client reports to the server, for its connection
    /// statistics. Purely descriptive — it changes nothing about behaviour, and
    /// it is what makes this daemon identifiable in a server-side connection
    /// list.
    #[serde(alias = "application_id")]
    pub application_id: Option<String>,
    /// Client identifier the server reports alongside the application name.
    /// Defaults to something the client generates.
    #[serde(alias = "custom_client_id")]
    pub custom_client_id: Option<String>,

    /// Parsed and **not applied**; see [`IGNORED_KEYS`].
    #[serde(alias = "limit_connections_to_queue_size")]
    pub limit_connections_to_queue_size: Option<bool>,
    /// Parsed and **not applied**; see [`IGNORED_KEYS`].
    #[serde(alias = "ignore_other_subnet_aliases")]
    pub ignore_other_subnet_aliases: Option<bool>,
}

impl ClusterConfig {
    /// The seed list as `ToHosts` wants it: one comma-separated string,
    /// duplicates removed, order preserved.
    ///
    /// # Errors
    /// When no seed was configured at all.
    pub fn seeds(&self) -> Result<String> {
        let mut seeds: Vec<String> = Vec::new();
        let mut push = |raw: &str| {
            let seed = raw.trim();
            if !seed.is_empty() && !seeds.iter().any(|s| s == seed) {
                seeds.push(seed.to_string());
            }
        };

        if let Some(host) = &self.host {
            for part in host.split(',') {
                push(part);
            }
        }
        for host in self.hosts.iter().flatten() {
            for part in host.split(',') {
                push(part);
            }
        }

        if seeds.is_empty() {
            return Err(ConfigError::new(
                "no seed host configured: set 'host' and/or 'hosts'",
            ));
        }
        Ok(seeds.join(","))
    }

    /// Build the client policy this section describes.
    ///
    /// # Errors
    /// A message naming the key at fault. Notably: `auth = "EXTERNAL"` and
    /// `auth = "PKI"` are rejected here, because the client requires TLS for
    /// both and this daemon has no TLS configuration to give it — better a
    /// clear message at startup than the client's own error from deep inside
    /// `Client::new`.
    pub fn to_client_policy(&self) -> Result<ClientPolicy> {
        let mut policy = ClientPolicy::default();
        policy.auth_mode = self.auth_mode()?;

        if let Some(name) = &self.cluster_name {
            policy.cluster_name = if name.is_empty() {
                None
            } else {
                Some(name.clone())
            };
        }

        // TLS first: the auth mode above is validated against it, and the
        // conversion can fail on an unreadable file — better to find that out
        // before anything else is resolved.
        if let Some(tls) = &self.tls {
            policy.tls_config = Some(tls.to_rustls()?);
        }

        if let Some(v) = self.timeout {
            policy.timeout = v.0;
        }
        if let Some(v) = self.connect_timeout {
            policy.connect_timeout = v.0;
        }
        if let Some(v) = self.idle_timeout {
            policy.idle_timeout = v.0;
        }
        if let Some(v) = self.login_timeout {
            policy.login_timeout = v.0;
        }
        if let Some(v) = self.tend_interval {
            policy.tend_interval = v.0;
        }
        if let Some(v) = self.config_interval {
            policy.config_interval = v.0;
        }

        if let Some(v) = self.connection_queue_size {
            policy.max_conns_per_node = v;
        }
        if let Some(v) = self.min_connections_per_node {
            policy.min_conns_per_node = v;
        }
        if let Some(v) = self.conn_pools_per_node {
            if v == 0 {
                return Err(ConfigError::new(
                    "conn-pools-per-node must be at least 1; zero pools means no connections",
                ));
            }
            policy.conn_pools_per_node = v;
        }

        if let Some(v) = self.buffer_reclaim_threshold {
            policy.buffer_reclaim_threshold = v;
        }
        if let Some(v) = self.use_buffer_pool {
            policy.use_buffer_pool = v;
        }
        if let Some(v) = self.buffer_pool_min_size {
            policy.buffer_pool_min_size = v;
        }
        if let Some(v) = self.buffer_pool_max_size {
            policy.buffer_pool_max_size = v;
        }
        if let Some(v) = self.buffer_pool_tier_bytes {
            policy.buffer_pool_tier_bytes = v;
        }
        // The client validates these two in `ClientPolicy::validate`, but only
        // once it is handed to `Client::new` — by which point the message is
        // about a policy rather than about a line in a file.
        if policy.use_buffer_pool {
            for (name, size) in [
                ("buffer-pool-min-size", policy.buffer_pool_min_size),
                ("buffer-pool-max-size", policy.buffer_pool_max_size),
            ] {
                if !size.is_power_of_two() {
                    return Err(ConfigError::new(format!(
                        "{name} must be a power of two, but is {size}; the pool indexes its \
                         tiers by bit width"
                    )));
                }
            }
            if policy.buffer_pool_max_size < policy.buffer_pool_min_size {
                return Err(ConfigError::new(format!(
                    "buffer-pool-max-size ({}) is below buffer-pool-min-size ({})",
                    policy.buffer_pool_max_size, policy.buffer_pool_min_size
                )));
            }
        }

        if let Some(map) = &self.ip_map {
            if !map.is_empty() {
                policy.ip_map = Some(map.clone());
            }
        }
        if let Some(v) = &self.application_id {
            policy.application_id = Some(v.clone());
        }
        if let Some(v) = &self.custom_client_id {
            policy.custom_client_id = Some(v.clone());
        }
        if let Some(v) = self.max_error_rate {
            policy.max_error_rate = v;
        }
        if let Some(v) = self.error_rate_window {
            policy.error_rate_window = v;
        }
        if let Some(v) = self.opening_connection_threshold {
            policy.opening_connection_threshold = v;
        }
        if let Some(v) = self.fail_if_not_connected {
            policy.fail_if_not_connected = v;
        }
        if let Some(v) = self.use_services_alternate {
            policy.use_services_alternate = v;
        }
        if let Some(v) = self.seed_only_cluster {
            policy.seed_only_cluster = v;
        }

        policy.rack_ids = self.resolved_rack_ids()?;

        Ok(policy)
    }

    /// `rack-aware` folded into `rack-ids`, since the client only has the
    /// latter.
    fn resolved_rack_ids(&self) -> Result<Option<Vec<usize>>> {
        let ids = match &self.rack_ids {
            Some(ids) if !ids.is_empty() => Some(ids.clone()),
            _ => None,
        };
        match self.rack_aware {
            Some(false) => Ok(None),
            Some(true) if ids.is_none() => Err(ConfigError::new(
                "rack-aware = true requires a non-empty 'rack-ids': the client \
                 derives rack awareness from the rack list, so there is nothing \
                 to enable without it",
            )),
            _ => Ok(ids),
        }
    }

    fn auth_mode(&self) -> Result<AuthMode> {
        let user = self.user.clone().unwrap_or_default();
        let password = self.password.clone().unwrap_or_default();
        let named = self.auth.as_deref().map(str::trim).unwrap_or("");

        let normalized: String = named
            .chars()
            .filter(|c| *c != '_' && *c != '-')
            .flat_map(char::to_lowercase)
            .collect();

        let mode = match normalized.as_str() {
            "" if user.is_empty() => AuthMode::None,
            "" | "internal" => AuthMode::Internal(user.clone(), password),
            "none" => AuthMode::None,
            "external" => AuthMode::External(user.clone(), password),
            "externalinsecure" => AuthMode::ExternalInsecure(user.clone(), password),
            "pki" => AuthMode::PKI,
            other => {
                return Err(ConfigError::new(format!(
                    "unknown auth mode '{other}': use INTERNAL, EXTERNAL, \
                     EXTERNAL_INSECURE or PKI"
                )))
            }
        };

        if matches!(
            mode,
            AuthMode::Internal(..) | AuthMode::External(..) | AuthMode::ExternalInsecure(..)
        ) && user.is_empty()
        {
            return Err(ConfigError::new(format!(
                "auth = '{named}' requires a 'user'"
            )));
        }

        // The client rejects both of these without TLS, and `PKI` needs a client
        // certificate specifically. Checking here means the complaint names the
        // configuration key that is missing, where `Client::new` would report a
        // policy error about a struct the operator never wrote.
        match &mode {
            AuthMode::External(..) if self.tls.is_none() => Err(ConfigError::new(
                "auth = 'EXTERNAL' sends the password to the server, so it requires TLS: add a \
                 [cluster.<name>.tls] section with a 'ca-file'. Use EXTERNAL_INSECURE only if \
                 sending the password in the clear is acceptable (testing)",
            )),
            AuthMode::PKI if self.tls.is_none() => Err(ConfigError::new(
                "auth = 'PKI' identifies the user by a client certificate, so it requires a \
                 [cluster.<name>.tls] section with 'cert-file' and 'key-file'",
            )),
            // TLS is configured but presents no certificate, so there is no
            // identity for the server to read — which it reports as an
            // authentication failure that says nothing about the cause.
            AuthMode::PKI
                if !self
                    .tls
                    .as_ref()
                    .is_some_and(TlsConfig::has_client_identity) =>
            {
                Err(ConfigError::new(
                    "auth = 'PKI' takes the user's identity from the client certificate, but the \
                     tls section names no 'cert-file'; without one the server has nothing to \
                     identify this daemon by",
                ))
            }
            _ => Ok(mode),
        }
    }

    /// Which [`IGNORED_KEYS`] this section actually set.
    fn ignored_keys_present(&self) -> Vec<&'static str> {
        let mut present = Vec::new();
        if self.limit_connections_to_queue_size.is_some() {
            present.push(IGNORED_KEYS[0]);
        }
        if self.ignore_other_subnet_aliases.is_some() {
            present.push(IGNORED_KEYS[1]);
        }
        present
    }
}

// ===== The file as a whole ==================================================

/// Raw file shape. The cluster sections stay `toml::Table` so that `[cluster]`
/// can hold either cluster keys or named sub-sections, and so that
/// `[defaults]` can be merged in before validation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    daemon: DaemonConfig,
    #[serde(default)]
    defaults: toml::Table,
    #[serde(default)]
    cluster: toml::Table,
}

/// A loaded, validated configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// The `[daemon]` section.
    pub daemon: DaemonConfig,
    /// Clusters in serving order: `default` first, then alphabetical.
    pub clusters: Vec<(String, ClusterConfig)>,
    /// Keys that were accepted and are not applied, qualified by section, for
    /// the one informational line the daemon logs at startup.
    pub ignored: Vec<String>,
}

impl Config {
    /// Read and validate a configuration file.
    ///
    /// # Errors
    /// [`ConfigError`] for an unreadable file, a syntax error, an unknown key
    /// or a semantically impossible setting.
    pub fn load(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            ConfigError::new(format!("cannot read {}: {e}", path.display()))
        })?;
        Config::parse(&text).map_err(|e| ConfigError::new(format!("{}: {e}", path.display())))
    }

    /// Validate configuration text.
    ///
    /// # Errors
    /// [`ConfigError`] as for [`load`](Self::load), without the file name.
    pub fn parse(text: &str) -> Result<Config> {
        let raw: RawFile =
            toml::from_str(text).map_err(|e| ConfigError::new(e.message().to_string()))?;

        raw.daemon
            .validate()
            .map_err(|e| ConfigError::new(format!("[daemon]: {e}")))?;

        // `[defaults]` is not attached to a cluster, so validate it on its own
        // to get an error that names the right section.
        deserialize_cluster(&raw.defaults)
            .map_err(|e| ConfigError::new(format!("[defaults]: {e}")))?;

        // Split `[cluster]`: table values are named clusters, everything else is
        // a key of the default cluster.
        //
        // Two cluster keys *are* table-valued — `tls` and `ip-map` — so a table
        // is no longer unambiguously a cluster name, and `[cluster.tls]` in the
        // bare form has to mean TLS on the default cluster rather than a cluster
        // called "tls". They are named here because that is the only way to tell
        // the two apart, and a cluster genuinely called `tls` has to be written
        // as `[cluster.tls]` under an explicit `[cluster.<name>]` layout — which
        // the error below points at.
        let mut sections: BTreeMap<String, toml::Table> = BTreeMap::new();
        let mut bare = toml::Table::new();
        for (key, value) in raw.cluster {
            match value {
                toml::Value::Table(table) if !TABLE_VALUED_CLUSTER_KEYS.contains(&key.as_str()) => {
                    sections.insert(key, table);
                }
                value => {
                    bare.insert(key, value);
                }
            }
        }
        if !bare.is_empty() {
            let entry = sections
                .entry(aerospike_php_ipc::DEFAULT_INSTANCE.to_string())
                .or_default();
            for (key, value) in bare {
                entry.entry(key).or_insert(value);
            }
        }

        if sections.is_empty() {
            return Err(ConfigError::new(
                "no clusters configured: add a [cluster.default] section",
            ));
        }

        let mut clusters: Vec<(String, ClusterConfig)> = Vec::with_capacity(sections.len());
        let mut ignored = Vec::new();
        for (name, section) in sections {
            let merged = merge(&raw.defaults, &section);
            let cluster = deserialize_cluster(&merged)
                .map_err(|e| ConfigError::new(format!("[cluster.{name}]: {e}")))?;
            // Fail fast on anything that could never work, so a typo in a
            // policy value is a startup error and not a mysterious instance
            // that answers CONNECTION forever.
            cluster
                .seeds()
                .map_err(|e| ConfigError::new(format!("[cluster.{name}]: {e}")))?;
            cluster
                .to_client_policy()
                .map_err(|e| ConfigError::new(format!("[cluster.{name}]: {e}")))?;
            for key in cluster.ignored_keys_present() {
                ignored.push(format!("cluster.{name}.{key}"));
            }
            clusters.push((name, cluster));
        }

        // `default` is the instance PHP gets when it names none, so serve it
        // first; the rest stay alphabetical for a stable startup log.
        clusters.sort_by(|(a, _), (b, _)| {
            let rank = |n: &str| u8::from(n != aerospike_php_ipc::DEFAULT_INSTANCE);
            rank(a).cmp(&rank(b)).then_with(|| a.cmp(b))
        });

        Ok(Config {
            daemon: raw.daemon,
            clusters,
            ignored,
        })
    }

    /// The instance names this daemon serves.
    #[must_use]
    pub fn cluster_names(&self) -> Vec<String> {
        self.clusters.iter().map(|(name, _)| name.clone()).collect()
    }
}

fn deserialize_cluster(table: &toml::Table) -> Result<ClusterConfig> {
    ClusterConfig::deserialize(toml::Value::Table(table.clone()))
        .map_err(|e| ConfigError::new(e.message().to_string()))
}

/// Shallow merge: every key of `over` wins over `base`.
fn merge(base: &toml::Table, over: &toml::Table) -> toml::Table {
    let mut merged = base.clone();
    for (key, value) in over {
        merged.insert(key.clone(), value.clone());
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_accept_strings_and_bare_milliseconds() {
        assert_eq!(parse_duration_ms("0").unwrap(), 0);
        assert_eq!(parse_duration_ms("500").unwrap(), 500);
        assert_eq!(parse_duration_ms("500ms").unwrap(), 500);
        assert_eq!(parse_duration_ms("1s").unwrap(), 1_000);
        assert_eq!(parse_duration_ms("30s").unwrap(), 30_000);
        assert_eq!(parse_duration_ms("2m").unwrap(), 120_000);
        assert_eq!(parse_duration_ms("1h").unwrap(), 3_600_000);
        assert_eq!(parse_duration_ms("1m30s").unwrap(), 90_000);
        assert_eq!(parse_duration_ms(" 250ms ").unwrap(), 250);
        // Sub-millisecond resolution is truncated rather than pretended away.
        assert_eq!(parse_duration_ms("999us").unwrap(), 0);
        assert_eq!(parse_duration_ms("1500us").unwrap(), 1);
    }

    #[test]
    fn bad_durations_are_rejected_with_a_message() {
        for bad in ["", "  ", "abc", "10x", "s10", "1s500"] {
            let err = parse_duration_ms(bad).unwrap_err();
            assert!(
                err.message().contains("duration"),
                "{bad}: {}",
                err.message()
            );
        }
    }

    #[test]
    fn bare_cluster_section_is_the_default_instance() {
        let cfg = Config::parse(
            r#"
            [cluster]
            host = "10.0.0.1:3000"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.cluster_names(), vec!["default".to_string()]);
        assert_eq!(cfg.clusters[0].1.seeds().unwrap(), "10.0.0.1:3000");
    }

    #[test]
    fn defaults_are_inherited_and_overridable() {
        let cfg = Config::parse(
            r#"
            [defaults]
            host = "seed:3000"
            timeout = "3s"
            use-services-alternate = true

            [cluster.default]

            [cluster.analytics]
            timeout = "10s"
            "#,
        )
        .unwrap();
        let by_name = |n: &str| {
            cfg.clusters
                .iter()
                .find(|(name, _)| name == n)
                .unwrap()
                .1
                .clone()
        };

        let default = by_name("default");
        assert_eq!(default.seeds().unwrap(), "seed:3000");
        let policy = default.to_client_policy().unwrap();
        assert_eq!(policy.timeout, 3_000);
        assert!(policy.use_services_alternate);

        let analytics = by_name("analytics");
        // Inherited seed, overridden timeout.
        assert_eq!(analytics.seeds().unwrap(), "seed:3000");
        assert_eq!(analytics.to_client_policy().unwrap().timeout, 10_000);
    }

    #[test]
    fn default_cluster_is_served_first() {
        let cfg = Config::parse(
            r#"
            [defaults]
            host = "seed"

            [cluster.zulu]
            [cluster.alpha]
            [cluster.default]
            "#,
        )
        .unwrap();
        assert_eq!(cfg.cluster_names(), ["default", "alpha", "zulu"]);
    }

    #[test]
    fn every_documented_key_loads_including_the_ignored_ones() {
        let cfg = Config::parse(
            r#"
            [daemon]
            instance = "default"
            worker_threads = 4
            default_timeout = "2s"
            log_level = "debug"

            [cluster.default]
            host = "127.0.0.1:3000,127.0.0.1:3010"
            hosts = ["127.0.0.2:3000"]
            user = "admin"
            password = "admin"
            auth = "internal"
            cluster-name = ""
            timeout = "30s"
            idle-timeout = "0"
            login-timeout = "5s"
            tend-interval = "1s"
            connection-queue-size = 300
            min-connections-per-node = 5
            max-error-rate = 100
            error-rate-window = 1
            opening-connection-threshold = 10
            fail-if-not-connected = false
            use-services-alternate = true
            seed-only-cluster = false
            rack-aware = true
            rack-ids = [1, 2]
            limit-connections-to-queue-size = true
            ignore-other-subnet-aliases = true
            "#,
        )
        .unwrap();

        assert_eq!(cfg.daemon.instance, "default");
        assert_eq!(cfg.daemon.worker_threads, Some(4));
        assert_eq!(cfg.daemon.default_timeout, Millis(2_000));
        assert_eq!(cfg.daemon.log_level, "debug");

        let cluster = &cfg.clusters[0].1;
        assert_eq!(
            cluster.seeds().unwrap(),
            "127.0.0.1:3000,127.0.0.1:3010,127.0.0.2:3000"
        );

        let policy = cluster.to_client_policy().unwrap();
        assert_eq!(
            policy.auth_mode,
            AuthMode::Internal("admin".into(), "admin".into())
        );
        assert_eq!(policy.cluster_name, None);
        assert_eq!(policy.timeout, 30_000);
        assert_eq!(policy.idle_timeout, 0);
        assert_eq!(policy.login_timeout, 5_000);
        assert_eq!(policy.tend_interval, 1_000);
        assert_eq!(policy.max_conns_per_node, 300);
        assert_eq!(policy.min_conns_per_node, 5);
        assert_eq!(policy.max_error_rate, 100);
        assert_eq!(policy.error_rate_window, 1);
        assert_eq!(policy.opening_connection_threshold, 10);
        assert!(!policy.fail_if_not_connected);
        assert!(policy.use_services_alternate);
        assert!(!policy.seed_only_cluster);
        assert_eq!(policy.rack_ids, Some(vec![1, 2]));

        // Accepted, reported, not applied.
        assert_eq!(
            cfg.ignored,
            [
                "cluster.default.limit-connections-to-queue-size",
                "cluster.default.ignore-other-subnet-aliases",
            ]
        );
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let err = Config::parse(
            r#"
            [cluster.default]
            host = "seed"
            connection-que-size = 10
            "#,
        )
        .unwrap_err();
        assert!(
            err.message().contains("cluster.default") && err.message().contains("unknown field"),
            "{}",
            err.message()
        );

        let err = Config::parse(
            r#"
            [defaults]
            hostt = "seed"
            "#,
        )
        .unwrap_err();
        assert!(err.message().contains("[defaults]"), "{}", err.message());

        let err = Config::parse("[daemon]\nnope = 1\n").unwrap_err();
        assert!(err.message().contains("unknown field"), "{}", err.message());
    }

    #[test]
    fn snake_case_cluster_keys_are_accepted_too() {
        let cfg = Config::parse(
            r#"
            [cluster.default]
            host = "seed"
            connection_queue_size = 7
            use_services_alternate = true
            "#,
        )
        .unwrap();
        let policy = cfg.clusters[0].1.to_client_policy().unwrap();
        assert_eq!(policy.max_conns_per_node, 7);
        assert!(policy.use_services_alternate);
    }

    #[test]
    fn missing_seed_is_a_startup_error() {
        let err = Config::parse("[cluster.default]\ntimeout = \"1s\"\n").unwrap_err();
        assert!(err.message().contains("no seed host"), "{}", err.message());
    }

    #[test]
    fn no_clusters_at_all_is_an_error() {
        let err = Config::parse("[daemon]\ninstance = \"x\"\n").unwrap_err();
        assert!(
            err.message().contains("no clusters configured"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn rack_awareness_folds_into_rack_ids() {
        let off = Config::parse(
            r#"
            [cluster.default]
            host = "seed"
            rack-aware = false
            rack-ids = [3]
            "#,
        )
        .unwrap();
        assert_eq!(off.clusters[0].1.to_client_policy().unwrap().rack_ids, None);

        let empty = Config::parse(
            r#"
            [cluster.default]
            host = "seed"
            rack-ids = []
            "#,
        )
        .unwrap();
        assert_eq!(
            empty.clusters[0].1.to_client_policy().unwrap().rack_ids,
            None
        );

        let err = Config::parse(
            r#"
            [cluster.default]
            host = "seed"
            rack-aware = true
            "#,
        )
        .unwrap_err();
        assert!(
            err.message().contains("rack-aware = true requires"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn auth_modes_map_and_tls_only_modes_are_refused_clearly() {
        let internal = ClusterConfig {
            host: Some("seed".into()),
            user: Some("u".into()),
            password: Some("p".into()),
            ..ClusterConfig::default()
        };
        assert_eq!(
            internal.to_client_policy().unwrap().auth_mode,
            AuthMode::Internal("u".into(), "p".into())
        );

        let anonymous = ClusterConfig {
            host: Some("seed".into()),
            ..ClusterConfig::default()
        };
        assert_eq!(
            anonymous.to_client_policy().unwrap().auth_mode,
            AuthMode::None
        );

        let insecure = ClusterConfig {
            auth: Some("External_Insecure".into()),
            ..internal.clone()
        };
        assert_eq!(
            insecure.to_client_policy().unwrap().auth_mode,
            AuthMode::ExternalInsecure("u".into(), "p".into())
        );

        // Both of these need TLS, and the complaint names the section to add —
        // which is the whole difference from before it could be configured.
        let external = ClusterConfig {
            auth: Some("EXTERNAL".into()),
            ..internal.clone()
        };
        let err = external.to_client_policy().unwrap_err();
        assert!(err.message().contains("requires TLS"), "{}", err.message());
        assert!(err.message().contains("ca-file"), "{}", err.message());

        let pki = ClusterConfig {
            auth: Some("pki".into()),
            ..anonymous.clone()
        };
        let err = pki.to_client_policy().unwrap_err();
        assert!(err.message().contains("cert-file"), "{}", err.message());

        // PKI with TLS but no client certificate is still refused: the server
        // takes the identity *from* the certificate, so there is nothing to
        // authenticate as. This is the case a plain "requires TLS" check misses.
        let pki_without_cert = ClusterConfig {
            auth: Some("pki".into()),
            tls: Some(TlsConfig {
                use_platform_roots: true,
                ..TlsConfig::default()
            }),
            ..anonymous.clone()
        };
        let err = pki_without_cert.to_client_policy().unwrap_err();
        assert!(
            err.message().contains("names no 'cert-file'"),
            "{}",
            err.message()
        );

        // EXTERNAL with TLS is accepted, which is what phase 9 changed.
        let external_over_tls = ClusterConfig {
            auth: Some("EXTERNAL".into()),
            tls: Some(TlsConfig {
                use_platform_roots: true,
                ..TlsConfig::default()
            }),
            ..internal.clone()
        };
        let policy = external_over_tls
            .to_client_policy()
            .expect("EXTERNAL over TLS must now be accepted");
        assert_eq!(
            policy.auth_mode,
            AuthMode::External("u".into(), "p".into())
        );
        assert!(
            policy.tls_config.is_some(),
            "the policy must carry the TLS configuration the client needs"
        );

        let no_user = ClusterConfig {
            auth: Some("INTERNAL".into()),
            ..anonymous
        };
        let err = no_user.to_client_policy().unwrap_err();
        assert!(err.message().contains("requires a 'user'"), "{}", err.message());

        let bogus = ClusterConfig {
            auth: Some("kerberos".into()),
            ..internal
        };
        let err = bogus.to_client_policy().unwrap_err();
        assert!(err.message().contains("unknown auth mode"), "{}", err.message());
    }

    /// A trust store is not optional: with neither a CA file nor the platform
    /// roots there is nothing to verify the cluster against, and rustls would
    /// happily build a config that rejects every node.
    #[test]
    fn tls_needs_something_to_trust() {
        let error = TlsConfig::default()
            .to_rustls()
            .expect_err("an empty tls section must be refused");
        assert!(error.message().contains("ca-file"), "{error}");
        assert!(error.message().contains("use-platform-roots"), "{error}");

        // The platform roots alone are enough — that is the public-CA case, and
        // it needs no files at all.
        assert!(TlsConfig {
            use_platform_roots: true,
            ..TlsConfig::default()
        }
        .to_rustls()
        .is_ok());
    }

    #[test]
    fn an_unreadable_certificate_file_is_named() {
        let error = TlsConfig {
            ca_file: Some(PathBuf::from("/nonexistent/ca.pem")),
            ..TlsConfig::default()
        }
        .to_rustls()
        .expect_err("a missing file must be refused");
        // The path, because "TLS failed" is the least useful thing this could
        // say.
        assert!(error.message().contains("/nonexistent/ca.pem"), "{error}");
        assert!(error.message().contains("ca-file"), "{error}");
    }

    /// Half a client identity is a mistake, not a fallback to none: the server
    /// would refuse the connection without saying which half was missing.
    #[test]
    fn a_client_certificate_without_its_key_is_refused_and_the_reverse() {
        let cert_only = TlsConfig {
            use_platform_roots: true,
            cert_file: Some(PathBuf::from("client.pem")),
            ..TlsConfig::default()
        }
        .to_rustls()
        .expect_err("a certificate with no key must be refused");
        assert!(cert_only.message().contains("without its private key"), "{cert_only}");

        let key_only = TlsConfig {
            use_platform_roots: true,
            key_file: Some(PathBuf::from("client.key")),
            ..TlsConfig::default()
        }
        .to_rustls()
        .expect_err("a key with no certificate must be refused");
        assert!(
            key_only.message().contains("without the certificate"),
            "{key_only}"
        );

        // And `has_client_identity` is what the PKI check reads, so it has to
        // agree with what was configured.
        assert!(!TlsConfig::default().has_client_identity());
        assert!(TlsConfig {
            cert_file: Some(PathBuf::from("client.pem")),
            ..TlsConfig::default()
        }
        .has_client_identity());
    }

    #[test]
    fn tls_is_read_from_the_file_in_both_spellings() {
        let cfg = Config::parse(
            "[cluster.default]\nhost = \"seed\"\n[cluster.default.tls]\n\
             use-platform-roots = true\n",
        )
        .unwrap();
        let tls = cfg.clusters[0].1.tls.as_ref().expect("tls must be parsed");
        assert!(tls.use_platform_roots);

        let underscored = Config::parse(
            "[cluster.default]\nhost = \"seed\"\n[cluster.default.tls]\n\
             use_platform_roots = true\n",
        )
        .unwrap();
        assert!(underscored.clusters[0].1.tls.as_ref().unwrap().use_platform_roots);

        // Absent means TLS off, which is a different thing from an empty table.
        let plain = Config::parse("[cluster.default]\nhost = \"seed\"\n").unwrap();
        assert!(plain.clusters[0].1.tls.is_none());
    }

    /// The bare `[cluster]` form tells a named cluster from a cluster key by
    /// whether the value is a table — and `tls` and `ip-map` broke that, because
    /// they are table-valued keys. `[cluster.tls]` has to mean TLS on the default
    /// cluster; read as a cluster called "tls" it would silently configure
    /// nothing.
    #[test]
    fn a_table_valued_cluster_key_is_not_mistaken_for_a_cluster_name() {
        let cfg = Config::parse(
            "[cluster]\nhost = \"seed\"\n[cluster.tls]\nuse-platform-roots = true\n",
        )
        .expect("[cluster.tls] must be TLS on the default cluster");
        assert_eq!(cfg.clusters.len(), 1, "there is one cluster, not two");
        assert_eq!(cfg.clusters[0].0, aerospike_php_ipc::DEFAULT_INSTANCE);
        assert!(cfg.clusters[0]
            .1
            .tls
            .as_ref()
            .is_some_and(|tls| tls.use_platform_roots));

        // The same for the other one.
        let mapped = Config::parse(
            "[cluster]\nhost = \"seed\"\n[cluster.ip-map]\n\"10.0.0.1\" = \"127.0.0.1\"\n",
        )
        .expect("[cluster.ip-map] must be an address map on the default cluster");
        assert_eq!(mapped.clusters.len(), 1);
        assert_eq!(
            mapped.clusters[0]
                .1
                .to_client_policy()
                .unwrap()
                .ip_map
                .as_ref()
                .and_then(|m| m.get("10.0.0.1"))
                .map(String::as_str),
            Some("127.0.0.1")
        );

        // A table that is *not* one of those two is still a named cluster, which
        // is the behaviour this exception must not have broken.
        let named = Config::parse(
            "[cluster]\nhost = \"seed\"\n[cluster.analytics]\nhost = \"other\"\n",
        )
        .unwrap();
        assert_eq!(named.cluster_names(), vec!["default", "analytics"]);
    }

    /// The certificate files are read while the configuration is being
    /// validated, not on the first connection — so `--check` catches a path that
    /// is wrong, which is the only time anyone is looking.
    #[test]
    fn a_certificate_path_that_does_not_exist_fails_at_startup() {
        let error = Config::parse(
            "[cluster.default]\nhost = \"seed\"\n[cluster.default.tls]\n\
             ca-file = \"/nonexistent/ca.pem\"\n",
        )
        .expect_err("an unreadable certificate must fail the configuration");
        assert!(error.message().contains("/nonexistent/ca.pem"), "{error}");
        // Named with its section, as every other cluster-key failure is.
        assert!(error.message().contains("[cluster.default]"), "{error}");
    }

    /// Every client-policy key the file gained in phase 9, checked where it
    /// lands — a key that parses but is never applied is the failure mode
    /// `deny_unknown_fields` cannot catch.
    #[test]
    fn the_remaining_client_policy_keys_reach_the_policy() {
        let cfg = ClusterConfig {
            host: Some("seed".into()),
            connect_timeout: Some(Millis(1_500)),
            config_interval: Some(Millis(20_000)),
            conn_pools_per_node: Some(4),
            buffer_reclaim_threshold: Some(70_000),
            use_buffer_pool: Some(true),
            buffer_pool_min_size: Some(16_384),
            buffer_pool_max_size: Some(262_144),
            buffer_pool_tier_bytes: Some(4_194_304),
            application_id: Some("php-daemon".into()),
            custom_client_id: Some("node-7".into()),
            ip_map: Some(HashMap::from([("10.0.0.1".to_string(), "127.0.0.1".to_string())])),
            ..ClusterConfig::default()
        };
        let policy = cfg.to_client_policy().unwrap();

        assert_eq!(policy.connect_timeout, 1_500);
        assert_eq!(policy.config_interval, 20_000);
        assert_eq!(policy.conn_pools_per_node, 4);
        assert_eq!(policy.buffer_reclaim_threshold, 70_000);
        assert!(policy.use_buffer_pool);
        assert_eq!(policy.buffer_pool_min_size, 16_384);
        assert_eq!(policy.buffer_pool_max_size, 262_144);
        assert_eq!(policy.buffer_pool_tier_bytes, 4_194_304);
        assert_eq!(policy.application_id.as_deref(), Some("php-daemon"));
        assert_eq!(policy.custom_client_id.as_deref(), Some("node-7"));
        assert_eq!(
            policy.ip_map.as_ref().and_then(|m| m.get("10.0.0.1")).map(String::as_str),
            Some("127.0.0.1")
        );

        // An empty map is "no rewriting", not an empty rewrite table.
        let empty = ClusterConfig {
            ip_map: Some(HashMap::new()),
            ..cfg
        };
        assert!(empty.to_client_policy().unwrap().ip_map.is_none());
    }

    /// The buffer pool indexes its tiers by bit width, so a size that is not a
    /// power of two is refused — here, naming the key, rather than by
    /// `Client::new` naming a struct field the operator never wrote.
    #[test]
    fn buffer_pool_sizes_must_be_powers_of_two_and_ordered() {
        let base = ClusterConfig {
            host: Some("seed".into()),
            use_buffer_pool: Some(true),
            ..ClusterConfig::default()
        };

        let odd = ClusterConfig {
            buffer_pool_min_size: Some(1_000),
            ..base.clone()
        };
        let error = odd.to_client_policy().unwrap_err();
        assert!(error.message().contains("buffer-pool-min-size"), "{error}");
        assert!(error.message().contains("power of two"), "{error}");

        let inverted = ClusterConfig {
            buffer_pool_min_size: Some(65_536),
            buffer_pool_max_size: Some(1_024),
            ..base.clone()
        };
        let error = inverted.to_client_policy().unwrap_err();
        assert!(error.message().contains("below"), "{error}");

        // Not checked at all when the pool is off, because nothing reads them.
        let disabled = ClusterConfig {
            use_buffer_pool: Some(false),
            buffer_pool_min_size: Some(1_000),
            ..base
        };
        assert!(disabled.to_client_policy().is_ok());
    }

    #[test]
    fn a_zero_connection_pool_count_is_refused() {
        let cfg = ClusterConfig {
            host: Some("seed".into()),
            conn_pools_per_node: Some(0),
            ..ClusterConfig::default()
        };
        let error = cfg.to_client_policy().unwrap_err();
        assert!(error.message().contains("at least 1"), "{error}");
    }

    /// The serving-loop tunables, and the one whose default is a real ceiling: a
    /// PHP-FPM pool larger than `max-workers` leaves the extra workers unable to
    /// attach at all.
    #[test]
    fn the_serving_loop_tunables_are_read_and_bounded() {
        let cfg = Config::parse(
            "[daemon]\nmax-workers = 250\naccept-batch = 8\ndrain-timeout = \"2s\"\n\
             notifier-cache = 512\n[daemon.receive-backoff]\nspin-iters = 10\n\
             yield-iters = 20\ninitial-sleep-us = 30\nmax-sleep-us = 40\n\
             [cluster]\nhost = \"seed\"\n",
        )
        .unwrap();
        let tuning = cfg.daemon.tuning();
        assert_eq!(tuning.max_workers, 250);
        assert_eq!(tuning.accept_batch, 8);
        assert_eq!(tuning.drain_timeout, Duration::from_secs(2));
        assert_eq!(tuning.notifier_cache, 512);
        assert_eq!(tuning.backoff.spin_iters, 10);
        assert_eq!(tuning.backoff.yield_iters, 20);
        assert_eq!(tuning.backoff.initial_sleep, Duration::from_micros(30));
        assert_eq!(tuning.backoff.max_sleep, Duration::from_micros(40));

        // Omitted, the tunables are the module's own constants, and the backoff
        // is the contract's — so the file's defaults cannot drift from the code's.
        let plain = Config::parse("[cluster]\nhost = \"seed\"\n").unwrap();
        let tuning = plain.daemon.tuning();
        assert_eq!(tuning.max_workers, crate::server::DEFAULT_MAX_WORKERS);
        assert_eq!(tuning.accept_batch, crate::server::DEFAULT_ACCEPT_BATCH);
        assert_eq!(tuning.drain_timeout, crate::server::DEFAULT_DRAIN_TIMEOUT);
        assert_eq!(tuning.notifier_cache, crate::server::DEFAULT_NOTIFIER_CACHE);
        assert_eq!(tuning.backoff, aerospike_php_ipc::Backoff::default());
    }

    #[test]
    fn a_daemon_that_could_not_serve_is_refused_at_startup() {
        let no_room = Config::parse("[daemon]\nmax-workers = 0\n[cluster]\nhost = \"s\"\n")
            .expect_err("a service with room for no worker must be refused");
        assert!(no_room.message().contains("max-workers"), "{no_room}");

        // A ceiling below the first step would clamp every sleep to the smaller
        // value, making the first tier meaningless.
        let backwards = Config::parse(
            "[daemon.receive-backoff]\ninitial-sleep-us = 100\nmax-sleep-us = 10\n\
             [cluster]\nhost = \"s\"\n",
        )
        .expect_err("an inverted backoff ladder must be refused");
        assert!(backwards.message().contains("doubles up to"), "{backwards}");
    }

    #[test]
    fn daemon_section_may_be_omitted_entirely() {
        let cfg = Config::parse("[cluster]\nhost = \"seed\"\n").unwrap();
        assert_eq!(cfg.daemon.instance, aerospike_php_ipc::DEFAULT_INSTANCE);
        assert_eq!(cfg.daemon.default_timeout, Millis(1_000));
        assert_eq!(cfg.daemon.log_level, "info");
        assert!(cfg.daemon.worker_threads.is_none());

        // The paging settings default to the query module's own, so a daemon
        // configured before they existed still pages.
        assert_eq!(cfg.daemon.page_size, crate::query::DEFAULT_PAGE_SIZE);
        assert_eq!(cfg.daemon.max_cursors, crate::query::DEFAULT_MAX_CURSORS);
        assert_eq!(
            cfg.daemon.cursor_idle_timeout.as_duration(),
            crate::query::DEFAULT_CURSOR_IDLE
        );
        assert_eq!(
            cfg.daemon.txn_idle_timeout.as_duration(),
            crate::txn::DEFAULT_TXN_IDLE
        );
        assert_eq!(cfg.daemon.max_transactions, crate::txn::DEFAULT_MAX_TXNS);

        // A transaction holds locks where a cursor holds memory, so it must be
        // reaped sooner by default. If these two ever converge it is worth
        // knowing on purpose rather than by accident.
        assert!(
            cfg.daemon.txn_idle_timeout.as_duration()
                < cfg.daemon.cursor_idle_timeout.as_duration(),
            "an idle transaction costs other writers their locks; an idle cursor \
             only costs memory"
        );
    }

    #[test]
    fn the_transaction_settings_are_read_in_both_spellings() {
        let cfg = Config::parse(
            "[daemon]\ntxn_idle_timeout = \"5s\"\nmax_transactions = 4\n\
             [cluster]\nhost = \"seed\"\n",
        )
        .unwrap();
        assert_eq!(cfg.daemon.txn_idle_timeout, Millis(5_000));
        assert_eq!(cfg.daemon.max_transactions, 4);

        let dashed = Config::parse(
            "[daemon]\ntxn-idle-timeout = \"5s\"\nmax-transactions = 4\n\
             [cluster]\nhost = \"seed\"\n",
        )
        .unwrap();
        assert_eq!(dashed.daemon.txn_idle_timeout, Millis(5_000));
        assert_eq!(dashed.daemon.max_transactions, 4);
    }

    /// Unknown keys are a startup error, so a key that is not read is a key that
    /// does not work — this is what pins the three paging ones as read.
    #[test]
    fn the_paging_settings_are_read_in_both_spellings() {
        let cfg = Config::parse(
            "[daemon]\npage_size = 250\ncursor_idle_timeout = \"5s\"\nmax_cursors = 8\n\
             [cluster]\nhost = \"seed\"\n",
        )
        .unwrap();
        assert_eq!(cfg.daemon.page_size, 250);
        assert_eq!(cfg.daemon.cursor_idle_timeout, Millis(5_000));
        assert_eq!(cfg.daemon.max_cursors, 8);

        // The hyphenated spelling every other key accepts.
        let dashed = Config::parse(
            "[daemon]\npage-size = 250\ncursor-idle-timeout = \"5s\"\nmax-cursors = 8\n\
             [cluster]\nhost = \"seed\"\n",
        )
        .unwrap();
        assert_eq!(dashed.daemon.page_size, 250);
        assert_eq!(dashed.daemon.cursor_idle_timeout, Millis(5_000));
        assert_eq!(dashed.daemon.max_cursors, 8);
    }

    #[test]
    fn the_shipped_example_configuration_loads() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("aerospike-daemon.toml.example");
        let cfg = Config::load(&path).expect("aerospike-daemon.toml.example must load");
        assert!(cfg
            .clusters
            .iter()
            .any(|(name, _)| name == aerospike_php_ipc::DEFAULT_INSTANCE));
    }
}
