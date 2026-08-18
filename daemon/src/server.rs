// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The iceoryx2 request/response loop.
//!
//! # Shape
//!
//! One service per instance-serving daemon, one dedicated OS thread driving it:
//!
//! ```text
//!  loop thread                         tokio runtime
//!  ───────────                         ─────────────
//!  receive()  ──decode──▶ spawn ──────▶ client.get(..).await
//!      ▲                                     │
//!      │              Completion{ticket}     │
//!  write reply ◀───── channel ◀──────────────┘
//! ```
//!
//! The reply is written on the loop thread rather than by the completing task,
//! because an `ActiveRequest` is not `Send`: it holds raw pointers into the
//! service's shared memory and, under `ipc::Service`, a non-thread-safe handle
//! to the server's shared state. So the request parks in `pending` under a
//! ticket while the operation runs, and the task sends back only the finished
//! [`Outcome`] — plain data. The alternative (`ipc_threadsafe::Service`) buys
//! nothing here: the reply write is a memcpy, and doing it on the loop thread
//! keeps one writer per port.
//!
//! `receive()` does not block, so the loop paces itself with the contract's
//! [`Backoff`] — spin, then yield, then short capped sleeps — and resets that
//! escalation whenever it made progress, so a busy daemon stays in the spin
//! tier and an idle one costs almost nothing.
//!
//! Both ports **must** set `initial_max_slice_len` and a `PowerOfTwo`
//! allocation strategy. With the default (`Static`, one element) any payload
//! larger than a single byte is refused with `ExceedsMaxLoanSize`.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use iceoryx2::active_request::ActiveRequest;
use iceoryx2::node::{Node, NodeBuilder};
use iceoryx2::port::server::Server as ServerPort;
use iceoryx2::prelude::{AllocationStrategy, ServiceName, SignalHandlingMode};

use aerospike_php_ipc::wakeup::{self, ReplyNotifier, WakeupMode};
use aerospike_php_ipc::{
    opcode, rpc_service_name, Backoff, ClientId, ReplyHeader, RequestHeader, StatusCode,
    INITIAL_MAX_SLICE_LEN, MAX_PAYLOAD_LEN,
};

use crate::dispatch::{Dispatcher, Outcome};

/// The iceoryx2 service flavour both ends use: shared memory, single-threaded
/// port handles.
type Ipc = iceoryx2::service::ipc::Service;

/// The server port, with the contract's typed headers and `[u8]` payloads.
type Port = ServerPort<Ipc, [u8], RequestHeader, [u8], ReplyHeader>;

/// One received request, still holding its response slot.
type Incoming = ActiveRequest<Ipc, [u8], RequestHeader, [u8], ReplyHeader>;

/// How many requests one loop iteration takes before it goes back to
/// delivering completions. Bounds the latency a burst of arrivals can add to
/// an operation that has already finished.
///
/// The default; `[daemon] accept-batch` overrides it.
pub const DEFAULT_ACCEPT_BATCH: usize = 64;

/// How long a graceful shutdown waits for in-flight work before giving up on
/// it. A request nobody answers leaves its worker to time out, which is worse
/// than a bounded wait but better than never exiting.
///
/// The default; `[daemon] drain-timeout` overrides it.
pub const DEFAULT_DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

/// How many per-worker notifiers to keep open. Only meaningful in event mode,
/// where each is a real port; the cache is bypassed entirely otherwise.
///
/// The default; `[daemon] notifier-cache` overrides it.
pub const DEFAULT_NOTIFIER_CACHE: usize = 256;

/// How many attached workers the service is created for, as client ports and as
/// iceoryx2 nodes.
///
/// iceoryx2 defaults to 8 clients and 20 nodes per service, which is nowhere
/// near a PHP-FPM pool: the ninth worker to attach would simply fail, and one
/// PHP process is both a client *and* a node. The value is set here rather than
/// left to the default because only the side that **creates** the service
/// decides it — a worker opening it can require a minimum but cannot raise the
/// ceiling.
///
/// The default; `[daemon] max-workers` overrides it, and a host running a
/// PHP-FPM pool larger than this **must** raise it.
pub const DEFAULT_MAX_WORKERS: usize = 64;

/// The serving loop's tunables, as the configuration file sets them.
///
/// One struct rather than four more `bind` parameters, and it implements
/// [`Default`] so a test — or any caller that does not care — passes
/// `Tuning::default()` and gets the constants above.
///
/// [`max_workers`](Self::max_workers) is the one with a hard consequence: it is
/// fixed when the iceoryx2 service is **created**, so raising it later means
/// restarting the daemon with no worker attached.
#[derive(Debug, Clone)]
pub struct Tuning {
    /// How many PHP workers may attach at once.
    pub max_workers: usize,
    /// Requests accepted per turn of the loop.
    pub accept_batch: usize,
    /// How long a shutdown waits for in-flight work.
    pub drain_timeout: Duration,
    /// How many per-worker event notifiers to keep open.
    pub notifier_cache: usize,
    /// How the loop waits when nothing has arrived.
    pub backoff: Backoff,
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            max_workers: DEFAULT_MAX_WORKERS,
            accept_batch: DEFAULT_ACCEPT_BATCH,
            drain_timeout: DEFAULT_DRAIN_TIMEOUT,
            notifier_cache: DEFAULT_NOTIFIER_CACHE,
            backoff: Backoff::default(),
        }
    }
}

/// A transport failure that prevents the daemon from serving at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerError(String);

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ServerError {}

fn bind_error(what: &str, detail: impl fmt::Debug) -> ServerError {
    ServerError(format!("{what}: {detail:?}"))
}

/// A cooperative stop flag, shared between the signal handler and the loop.
#[derive(Debug, Clone, Default)]
pub struct Shutdown {
    flag: Arc<AtomicBool>,
}

impl Shutdown {
    /// A flag that has not been raised.
    #[must_use]
    pub fn new() -> Shutdown {
        Shutdown::default()
    }

    /// Ask the loop to stop accepting and drain.
    pub fn signal(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Whether a stop was requested.
    #[must_use]
    pub fn is_signalled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

/// A finished operation on its way back to the loop thread.
struct Completion {
    ticket: u64,
    seq: u64,
    client_id: ClientId,
    outcome: Outcome,
}

/// The request/response server for one instance.
pub struct Server {
    instance: String,
    dispatcher: Arc<Dispatcher>,
    runtime: tokio::runtime::Handle,
    node: Node<Ipc>,
    port: Port,
    notifiers: NotifierCache,
    backoff: Backoff,
    accept_batch: usize,
    drain_timeout: Duration,
    completions_tx: Sender<Completion>,
    completions_rx: Receiver<Completion>,
    pending: HashMap<u64, Incoming>,
    next_ticket: u64,
}

impl Server {
    /// Open the service and create the server port.
    ///
    /// Must be called on the thread that will run [`run_until`](Self::run_until):
    /// the port is not `Send`. Binding separately from running is deliberate —
    /// it lets a caller (the daemon's `main`, or a test) find out that the
    /// service is up before it starts depending on answers.
    ///
    /// # Errors
    /// [`ServerError`] if the node, service or port cannot be created — most
    /// often because another daemon already serves this instance name.
    pub fn bind(
        instance: &str,
        dispatcher: Arc<Dispatcher>,
        runtime: tokio::runtime::Handle,
        tuning: Tuning,
    ) -> Result<Server, ServerError> {
        let node = NodeBuilder::new()
            // The daemon handles SIGINT/SIGTERM itself, through tokio, so that
            // shutdown drains in-flight work. Leaving iceoryx2's own handler
            // installed would put two handlers on the same signals.
            .signal_handling_mode(SignalHandlingMode::Disabled)
            .create::<Ipc>()
            .map_err(|e| bind_error("create iceoryx2 node", e))?;

        let name = rpc_service_name(instance);
        let service_name: ServiceName = name
            .as_str()
            .try_into()
            .map_err(|e| bind_error("invalid service name", e))?;

        let service = node
            .service_builder(&service_name)
            .request_response::<[u8], [u8]>()
            .request_user_header::<RequestHeader>()
            .response_user_header::<ReplyHeader>()
            // One worker is one client and one node; see DEFAULT_MAX_WORKERS.
            .max_clients(tuning.max_workers)
            .max_nodes(tuning.max_workers + 1)
            .open_or_create()
            .map_err(|e| bind_error(&format!("open service {name}"), e))?;

        let port = service
            .server_builder()
            .initial_max_slice_len(INITIAL_MAX_SLICE_LEN)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .map_err(|e| bind_error("create server port", e))?;

        let (completions_tx, completions_rx) = mpsc::channel();
        Ok(Server {
            instance: instance.to_string(),
            dispatcher,
            runtime,
            node,
            port,
            notifiers: NotifierCache::new(instance, tuning.notifier_cache),
            backoff: tuning.backoff,
            accept_batch: tuning.accept_batch.max(1),
            drain_timeout: tuning.drain_timeout,
            completions_tx,
            completions_rx,
            pending: HashMap::new(),
            next_ticket: 0,
        })
    }

    /// The service name this server answers on.
    #[must_use]
    pub fn service_name(&self) -> String {
        rpc_service_name(&self.instance)
    }

    /// Serve until `shutdown` is raised and in-flight work has drained.
    ///
    /// # Errors
    /// [`ServerError`] only for a failure that makes the port unusable;
    /// per-request problems become error replies, and a failed reply write is
    /// logged.
    pub fn run_until(&mut self, shutdown: &Shutdown) -> Result<(), ServerError> {
        // The version is logged in its own right, not just as part of the
        // service name: an extension of any other version cannot talk to this
        // daemon at all, so "which version is running" is the first thing an
        // operator needs when a worker reports that it cannot attach.
        log::info!(
            "serving '{}' on {} (version: {}, wakeup: {}, instances: {})",
            self.instance,
            self.service_name(),
            crate::VERSION,
            wakeup::mode(),
            self.dispatcher.instance_names().join(", ")
        );

        let mut waiting = self.backoff.start();
        let mut draining: Option<Instant> = None;

        loop {
            let mut progressed = self.deliver_completions();

            if shutdown.is_signalled() {
                let since = *draining.get_or_insert_with(|| {
                    log::info!(
                        "shutdown requested: no longer accepting requests, \
                         draining {} in flight",
                        self.pending.len()
                    );
                    Instant::now()
                });
                if self.pending.is_empty() {
                    break;
                }
                if since.elapsed() >= self.drain_timeout {
                    log::warn!(
                        "giving up on {} in-flight request(s) after {}s",
                        self.pending.len(),
                        self.drain_timeout.as_secs()
                    );
                    break;
                }
            } else {
                progressed |= self.accept();
            }

            if progressed {
                waiting = self.backoff.start();
            } else {
                waiting.snooze();
            }
        }

        log::info!("stopped serving '{}'", self.instance);
        Ok(())
    }

    /// Take up to `accept_batch` arrivals. Returns whether any arrived.
    fn accept(&mut self) -> bool {
        let mut received = false;
        for _ in 0..self.accept_batch {
            match self.port.receive() {
                Ok(Some(request)) => {
                    received = true;
                    self.begin(request);
                }
                Ok(None) => break,
                Err(e) => {
                    // Nothing to answer — we never got a request — so log and
                    // keep serving rather than tearing the daemon down.
                    log::warn!("receive on {} failed: {e:?}", self.service_name());
                    break;
                }
            }
        }
        received
    }

    /// Validate one request and either answer it immediately or hand it to the
    /// runtime.
    fn begin(&mut self, request: Incoming) {
        let header: RequestHeader = *request.user_header();
        let seq = header.seq;
        let client_id = ClientId(header.client_id);
        let payload_len = request.payload().len();

        if let Err(e) = header.validate(payload_len) {
            log::debug!("rejecting frame from client {}: {e}", header.client_id);
            self.answer(&request, seq, client_id, &Outcome::from_codec_error(&e));
            return;
        }
        if payload_len > MAX_PAYLOAD_LEN {
            self.answer(
                &request,
                seq,
                client_id,
                &Outcome::from_codec_error(&aerospike_php_ipc::CodecError::TooLarge {
                    len: payload_len,
                }),
            );
            return;
        }

        // Liveness is answered here rather than on the runtime: a PING must not
        // queue behind database work, and it needs nothing the loop thread
        // does not already have.
        if header.opcode == opcode::PING {
            let outcome = self.dispatcher.ping();
            self.answer(&request, seq, client_id, &outcome);
            return;
        }

        let ticket = self.next_ticket;
        self.next_ticket = self.next_ticket.wrapping_add(1);
        let body = request.payload().to_vec();
        let opcode = header.opcode;
        let deadline = self.dispatcher.deadline(header.timeout_ms);
        let dispatcher = self.dispatcher.clone();
        let completions = self.completions_tx.clone();

        self.pending.insert(ticket, request);
        self.runtime.spawn(async move {
            // The worker declared how long it will wait, so work nobody is
            // listening for is abandoned rather than left to hold a connection.
            let outcome = match tokio::time::timeout(deadline, dispatcher.execute(opcode, &body))
                .await
            {
                Ok(outcome) => outcome,
                Err(_) => Outcome::error(
                    StatusCode::TIMEOUT,
                    format!(
                        "the daemon abandoned this operation after {}ms",
                        deadline.as_millis()
                    ),
                ),
            };
            // A send failure means the loop is gone, which means the process is
            // going down: there is nobody left to tell.
            let _ = completions.send(Completion {
                ticket,
                seq,
                client_id,
                outcome,
            });
        });
    }

    /// Write every reply the runtime has finished. Returns whether any were.
    fn deliver_completions(&mut self) -> bool {
        let mut delivered = false;
        loop {
            let completion = match self.completions_rx.try_recv() {
                Ok(completion) => completion,
                // Disconnection cannot happen: this struct holds a sender.
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            delivered = true;
            match self.pending.remove(&completion.ticket) {
                Some(request) => self.answer(
                    &request,
                    completion.seq,
                    completion.client_id,
                    &completion.outcome,
                ),
                None => log::error!(
                    "completion for ticket {} has no pending request",
                    completion.ticket
                ),
            }
        }
        delivered
    }

    /// Write `outcome` into the request's response slot and wake the worker.
    fn answer(
        &mut self,
        request: &Incoming,
        seq: u64,
        client_id: ClientId,
        outcome: &Outcome,
    ) {
        if let Err(e) = write_reply(request, seq, outcome) {
            if outcome.status() == StatusCode::FRAME_TOO_LARGE {
                log::error!("seq {seq}: could not send the failure reply either: {e}");
            } else {
                // Most likely the payload outgrew what the segment can loan.
                // Say so, rather than leaving the worker to time out.
                let fallback = Outcome::error(
                    StatusCode::FRAME_TOO_LARGE,
                    format!(
                        "a {}-byte reply could not be placed in shared memory: {e}",
                        outcome.body().len()
                    ),
                );
                match write_reply(request, seq, &fallback) {
                    Ok(()) => log::warn!("seq {seq}: reply replaced by FRAME_TOO_LARGE: {e}"),
                    Err(e) => log::error!("seq {seq}: reply could not be sent: {e}"),
                }
            }
        }
        self.wake(client_id);
    }

    /// Tell the worker its reply is ready.
    fn wake(&mut self, client_id: ClientId) {
        // In the default build the reply is already visible in shared memory
        // and `notify` is a no-op, so skip the cache entirely: polling workers
        // must not pay for a hash lookup per request.
        if wakeup::mode() == WakeupMode::ShmPoll {
            return;
        }
        if let Err(e) = self.notifiers.notify(&self.node, client_id) {
            // Not fatal: the reply is in shared memory, and a worker's blocking
            // wait is bounded, so it degrades to slow polling.
            log::warn!("could not notify client {}: {e}", client_id.0);
        }
    }
}

/// Write one reply into the loaned response slot.
fn write_reply(request: &Incoming, seq: u64, outcome: &Outcome) -> Result<(), String> {
    let body = outcome.body();
    let response = request
        .loan_slice_uninit(body.len())
        .map_err(|e| format!("{e:?}"))?;
    let mut response = response.write_from_slice(body);
    *response.user_header_mut() = outcome.header(seq);
    response.send().map_err(|e| format!("{e:?}"))
}

/// Bounded, least-recently-used cache of per-worker notifiers.
///
/// Opening an event service is not free, and a host can run hundreds of
/// workers, so notifiers are reused; the bound keeps a long-lived daemon from
/// accumulating a port for every worker that ever existed.
struct NotifierCache {
    instance: String,
    capacity: usize,
    clock: u64,
    entries: HashMap<ClientId, CachedNotifier>,
}

struct CachedNotifier {
    notifier: ReplyNotifier<Ipc>,
    last_used: u64,
}

impl NotifierCache {
    fn new(instance: &str, capacity: usize) -> NotifierCache {
        NotifierCache {
            instance: instance.to_string(),
            capacity: capacity.max(1),
            clock: 0,
            entries: HashMap::new(),
        }
    }

    fn notify(
        &mut self,
        node: &Node<Ipc>,
        client_id: ClientId,
    ) -> Result<(), wakeup::WakeupError> {
        self.clock += 1;
        let clock = self.clock;

        if let Some(entry) = self.entries.get_mut(&client_id) {
            entry.last_used = clock;
            return entry.notifier.notify();
        }

        if self.entries.len() >= self.capacity {
            self.evict_least_recently_used();
        }
        let notifier = ReplyNotifier::open(node, &self.instance, client_id)?;
        let result = notifier.notify();
        self.entries.insert(
            client_id,
            CachedNotifier {
                notifier,
                last_used: clock,
            },
        );
        result
    }

    fn evict_least_recently_used(&mut self) {
        if let Some(oldest) = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(id, _)| *id)
        {
            self.entries.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_is_shared_between_clones() {
        let shutdown = Shutdown::new();
        let copy = shutdown.clone();
        assert!(!shutdown.is_signalled());
        copy.signal();
        assert!(shutdown.is_signalled());
    }

    #[test]
    fn the_notifier_cache_stays_bounded_and_evicts_the_oldest() {
        let node = NodeBuilder::new()
            .signal_handling_mode(SignalHandlingMode::Disabled)
            .create::<Ipc>()
            .unwrap();
        let mut cache = NotifierCache::new("cache-test", 2);

        let a = ClientId::new(std::process::id(), 1);
        let b = ClientId::new(std::process::id(), 2);
        let c = ClientId::new(std::process::id(), 3);

        cache.notify(&node, a).unwrap();
        cache.notify(&node, b).unwrap();
        // Touch `a` so `b` becomes the least recently used.
        cache.notify(&node, a).unwrap();
        cache.notify(&node, c).unwrap();

        assert_eq!(cache.entries.len(), 2);
        assert!(cache.entries.contains_key(&a));
        assert!(cache.entries.contains_key(&c));
        assert!(!cache.entries.contains_key(&b));
    }
}
