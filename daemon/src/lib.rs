// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! One long-lived process holding real Aerospike clients, serving many PHP
//! workers over shared memory.
//!
//! # Why a daemon
//!
//! PHP's per-request model cannot own a smart client. Every worker would run
//! its own tend loop and its own connection pool per node, so a host with 200
//! workers would present itself to a 10-node cluster as 200 tenders and up to
//! 2000 idle connections — and each of those clients would still be cold at
//! the start of every request. The daemon holds one client per cluster, tends
//! once, pools once, and hands workers the answers over the IPC contract in
//! [`aerospike_php_ipc`].
//!
//! # Layout
//!
//! - [`config`] — the TOML file: `[daemon]`, `[defaults]`, `[cluster.<name>]`.
//! - [`instance`] — one client per configured cluster, built at startup.
//! - [`convert`] — the contract's value model ⇄ the client's.
//! - [`policy`] — a request's `WirePolicy` → the client's read/write policies.
//! - [`failure`] — client errors → contract status codes.
//! - [`ops`] — the contract's operations → the client's, for `operate`.
//! - [`batch`] — the contract's batch rows → the client's, and answers back.
//! - [`query`] — scans and queries, one page per round trip, and the cursors
//!   that resume them.
//! - [`admin`] — UDF modules, secondary indexes, truncation, info and nodes,
//!   plus the task handles that report on the long-running ones.
//! - [`txn`] — multi-record transactions, and why an abandoned one is *aborted*
//!   rather than dropped.
//! - [`security`] — users, roles and privileges: the one family that holds no
//!   state at all.
//! - [`dispatch`] — decode a request, run it, produce a reply. No transport.
//! - [`server`] — the iceoryx2 loop that carries those replies.
//!
//! # One version, both halves
//!
//! This daemon serves only extensions of **exactly its own version**, and
//! [`VERSION`] — which comes from the contract, not from this crate — is it. The
//! shared-memory service name embeds it, so an extension of any other version
//! cannot even find this daemon, and every frame header carries a fingerprint of
//! it in case a name resolves that should not have. Both halves assert at compile
//! time that their own package version equals the contract's, so the two cannot
//! be bumped apart.
//!
//! The practical consequence for anything below: the wire format may change
//! freely between releases. Nothing here has to stay compatible with a peer from
//! another build, because there can never be one.
//!
//! # Every verb honours a per-call policy
//!
//! Every single-record verb takes a [`Target`](aerospike_php_ipc::Target), which
//! carries a [`WirePolicy`](aerospike_php_ipc::WirePolicy) — so a PHP caller can
//! set a TTL on a `PUT` or a filter on a `GET`, and [`policy`] resolves all of
//! them through one path. A field that does not apply to the verb being run is
//! **rejected by name** rather than ignored: the daemon is the only side that
//! knows which command it is serving.
//!
//! # Serving in-process
//!
//! [`server::Server`] binds and runs separately, so the whole daemon can be
//! driven from a test with a real iceoryx2 client over real shared memory:
//!
//! ```no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//! use aerospike_php_daemon::dispatch::Dispatcher;
//! use aerospike_php_daemon::instance::Instances;
//! use aerospike_php_daemon::server::{Server, Shutdown, Tuning};
//!
//! let runtime = tokio::runtime::Runtime::new().unwrap();
//! let instances = Arc::new(Instances::from_clients(Vec::new()));
//! let dispatcher = Arc::new(Dispatcher::new(instances, Duration::from_secs(1)));
//! let shutdown = Shutdown::new();
//!
//! let stop = shutdown.clone();
//! let handle = runtime.handle().clone();
//! let loop_thread = std::thread::spawn(move || {
//!     // Bind on the serving thread: the port is not `Send`.
//!     let mut server = Server::bind("my-instance", dispatcher, handle, Tuning::default()).unwrap();
//!     server.run_until(&stop).unwrap();
//! });
//!
//! shutdown.signal();
//! loop_thread.join().unwrap();
//! ```

#![warn(missing_docs)]

pub mod admin;
pub mod batch;
pub mod config;
pub mod convert;
pub mod dispatch;
pub mod exp;
pub mod failure;
pub mod instance;
pub mod ops;
pub mod policy;
pub mod query;
pub mod security;
pub mod server;
pub mod txn;

/// The version this daemon serves, as reported by `PING`.
///
/// Taken from the contract rather than from this crate, because the contract
/// is what has to match: an extension of another version cannot even find this
/// daemon's service, so there is one version in play and this is it. The
/// assertion below keeps the two from being separately bumped.
pub const VERSION: &str = aerospike_php_ipc::VERSION;

const _: () = assert!(
    aerospike_php_ipc::is_version(env!("CARGO_PKG_VERSION")),
    "the daemon's package version must equal aerospike-php-ipc's: the extension and the daemon \
     are matched by version, so a daemon whose own version differs from the contract it speaks \
     would report a version nothing can pair with"
);
