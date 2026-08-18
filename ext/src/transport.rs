// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The worker's side of the shared-memory channel.
//!
//! # Why nothing is opened in `MINIT`
//!
//! PHP-FPM runs `MINIT` in its **master** process and only then forks the
//! pool of workers. An iceoryx2 node and its ports are not fork-safe: they
//! name shared-memory segments whose reference counts and port registrations
//! live in that shared memory, are *not* duplicated by `fork()`, and are
//! owned by whichever process created them. A worker that inherited the
//! master's ports would be a second user of a single registration —
//! corrupting the daemon's view of who is attached, and racing another worker
//! for the same response slots.
//!
//! So this module attaches **lazily, on first use**, which under FPM is
//! always inside a worker. As a second line of defence it records the pid it
//! attached under and rechecks it on every call: if the pid has changed, this
//! process is a child that inherited someone else's state and starts over.
//! See [`discard_if_forked`] for why the inherited state is leaked rather
//! than dropped.
//!
//! # One request at a time
//!
//! PHP is synchronous, so a worker has at most one request outstanding. The
//! per-process `seq` therefore is not a multiplexing key; it exists so that a
//! reply which arrives *after* the worker gave up waiting is recognised as
//! stale instead of being handed back as the answer to the next call.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aerospike_php_ipc::wakeup::Waiter;
use aerospike_php_ipc::{
    ClientId, INITIAL_MAX_SLICE_LEN, MAX_PAYLOAD_LEN, ReplyHeader, RequestHeader, VERSION,
    parse_rpc_service_name, rpc_service_name,
};
use iceoryx2::config::Config;
use iceoryx2::node::{Node, NodeBuilder};
use iceoryx2::port::client::Client as RequestPort;
use iceoryx2::prelude::{AllocationStrategy, CallbackProgression, LogLevel, ServiceName, ipc};
use iceoryx2::service::builder::request_response::RequestResponseOpenError;
use iceoryx2::service::port_factory::client::ClientCreateError;
// The trait, for `Service::list`: enumerating services is what lets a failed
// attach tell "no daemon" apart from "a daemon of another version".
use iceoryx2::service::Service as _;

use crate::error::{AeroError, AeroResult};
use crate::settings::Settings;

/// The request port, with every generic parameter of the contract pinned.
///
/// The contract's headers are used directly as the user header types.
/// iceoryx2 requires `RequestHeader: Default` because `loan_slice_uninit`
/// default-initialises the user header before handing the loan back, and the
/// contract derives it — deliberately producing an *invalid* header (magic 0)
/// so that a frame whose header was never filled in fails
/// [`RequestHeader::validate`] instead of travelling as a plausible one.
type Port = RequestPort<ipc::Service, [u8], RequestHeader, [u8], ReplyHeader>;

/// The service the port is created from, with the same parameters pinned.
type RpcService = iceoryx2::service::port_factory::request_response::PortFactory<
    ipc::Service,
    [u8],
    RequestHeader,
    [u8],
    ReplyHeader,
>;

/// Everything this process has attached, keyed by nothing — there is exactly
/// one per worker.
struct Registry {
    /// The pid this state was built under. Everything below belongs to that
    /// process and to no other.
    pid: u32,
    node: Node<ipc::Service>,
    /// Minted after the fork, as the contract requires.
    client_id: ClientId,
    seq: u64,
    /// One per cluster instance the scripts in this worker have touched.
    sessions: HashMap<String, Session>,
}

struct Session {
    port: Port,
    waiter: Waiter<ipc::Service>,
}

thread_local! {
    /// Process-local — thread-local is the same thing under PHP NTS, and is
    /// the right granularity under ZTS, where each request thread needs its
    /// own ports for exactly the reason a forked worker does.
    static REGISTRY: RefCell<Option<Registry>> = const { RefCell::new(None) };
}

/// Send one request and wait for its reply.
///
/// # Errors
/// [`AeroError`] if the daemon cannot be reached, the request cannot be
/// placed in shared memory, the deadline passes, or the reply does not
/// validate.
pub fn call(
    instance: &str,
    settings: &Settings,
    opcode: u16,
    operation: &str,
    body: &[u8],
) -> AeroResult<(ReplyHeader, Vec<u8>)> {
    REGISTRY.with(|cell| {
        let mut slot = cell
            .try_borrow_mut()
            .map_err(|_| AeroError::client("the Aerospike transport is already in use on this thread"))?;
        discard_if_forked(&mut slot);
        if slot.is_none() {
            *slot = Some(Registry::attach()?);
        }
        let registry = slot.as_mut().expect("attached immediately above");
        registry.call(instance, settings, opcode, operation, body)
    })
}

/// Release this process's ports, from `MSHUTDOWN`.
pub fn teardown() {
    // `try_with` rather than `with`: at shutdown the thread-local may already
    // have been destroyed, and a panic unwinding out of MSHUTDOWN across the
    // FFI boundary would be worse than leaking.
    let _ = REGISTRY.try_with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut() {
            // In a forked child — or in the FPM master, which never attached
            // — there is nothing of ours to release.
            discard_if_forked(&mut slot);
            drop(slot.take());
        }
    });
}

/// Throw away state inherited across a `fork()`.
///
/// The inherited value is `forget`-ed instead of dropped on purpose. Running
/// iceoryx2's destructors here would decrement shared-memory reference counts
/// and deregister ports that the **parent** is still using, so a child
/// cleaning up "its" copy would break the parent and the daemon's bookkeeping
/// with it. Leaking a forked child's copy costs some address space that the
/// child's own exit reclaims anyway; the parent still releases the real
/// resources when it shuts down.
fn discard_if_forked(slot: &mut Option<Registry>) {
    let pid = std::process::id();
    if slot.as_ref().is_some_and(|registry| registry.pid != pid) {
        std::mem::forget(slot.take());
    }
}

/// Stop iceoryx2 from writing to stderr.
///
/// By default it logs informationally — "No config file was loaded", node
/// bookkeeping — straight to stderr. In a PHP extension that lands in a web
/// server's error log, or in the middle of a CLI script's output, on every
/// worker that attaches. Anything that actually matters here is turned into an
/// exception, so warnings and below are noise; `Error` and above still print,
/// which keeps a genuinely broken shared-memory setup diagnosable.
fn quiet_iceoryx_logging() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| iceoryx2::prelude::set_log_level(LogLevel::Error));
}

impl Registry {
    fn attach() -> AeroResult<Registry> {
        let pid = std::process::id();
        // Before the first call into iceoryx2, which is what emits the
        // "no config file" warning.
        quiet_iceoryx_logging();
        let node = NodeBuilder::new().create::<ipc::Service>().map_err(|error| {
            AeroError::client(format!("could not create an iceoryx2 node: {error:?}"))
        })?;
        Ok(Registry {
            pid,
            node,
            // Minted here, which is after any fork, because a pid alone would
            // not do: pids are recycled and a stale id could collide with
            // resources the daemon has not yet reaped.
            client_id: ClientId::new(pid, mint_nonce()),
            seq: 0,
            sessions: HashMap::new(),
        })
    }

    fn session(&mut self, instance: &str, settings: &Settings) -> AeroResult<&mut Session> {
        if !self.sessions.contains_key(instance) {
            let session = Session::open(&self.node, instance, self.client_id, settings)?;
            self.sessions.insert(instance.to_owned(), session);
        }
        Ok(self.sessions.get_mut(instance).expect("inserted immediately above"))
    }

    fn call(
        &mut self,
        instance: &str,
        settings: &Settings,
        opcode: u16,
        operation: &str,
        body: &[u8],
    ) -> AeroResult<(ReplyHeader, Vec<u8>)> {
        if body.len() > MAX_PAYLOAD_LEN {
            return Err(AeroError::client(format!(
                "{operation} needs a {}-byte request, past the {MAX_PAYLOAD_LEN}-byte protocol \
                 maximum",
                body.len()
            )));
        }

        // Wrapping is theoretical — a worker would need 2^64 requests — but
        // the stale-reply comparison below assumes it never happens.
        self.seq = self.seq.wrapping_add(1);
        let seq = self.seq;
        let client_id = self.client_id;
        let session = self.session(instance, settings)?;

        // Start every call by spinning again. Without this a worker that once
        // slept its way through one slow operation would keep the slow
        // cadence for every call after it.
        session.waiter.rearm(settings.backoff);

        let request = session.port.loan_slice_uninit(body.len()).map_err(|error| {
            AeroError::client(format!(
                "could not loan a {}-byte request slice for {operation}: {error:?}",
                body.len()
            ))
        })?;
        let mut request = request.write_from_slice(body);
        *request.user_header_mut() = RequestHeader::new(
            opcode,
            seq,
            client_id,
            settings.timeout_ms,
            u32::try_from(body.len()).expect("checked against MAX_PAYLOAD_LEN above"),
        );
        let pending = request.send().map_err(|error| {
            AeroError::client(format!("could not send {operation} to the daemon: {error:?}"))
        })?;

        let deadline = Instant::now() + settings.timeout;
        loop {
            let received = pending.receive().map_err(|error| {
                AeroError::client(format!(
                    "could not read the daemon's reply to {operation}: {error:?}"
                ))
            })?;
            if let Some(reply) = received {
                let header = *reply.user_header();
                // A reply for an earlier request is one this worker already
                // gave up on. Handing it back as the answer to *this* call is
                // the classic post-timeout bug, so drop it and keep waiting.
                if header.seq < seq {
                    continue;
                }
                if header.seq != seq {
                    return Err(AeroError::client(format!(
                        "the daemon answered sequence {} for request {seq}; the channel is out of \
                         step",
                        header.seq
                    )));
                }
                let payload = reply.payload();
                // Never trust a header out of shared memory before it has
                // been checked: this is where a mismatched daemon shows up.
                header.validate(payload.len()).map_err(AeroError::codec)?;
                return Ok((header, payload.to_vec()));
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(AeroError::timeout(operation, settings.timeout_ms));
            }
            session.waiter.wait_hint(deadline - now).map_err(|error| {
                AeroError::client(format!("waiting for the daemon's reply failed: {error}"))
            })?;
        }
    }
}

impl Session {
    fn open(
        node: &Node<ipc::Service>,
        instance: &str,
        client_id: ClientId,
        settings: &Settings,
    ) -> AeroResult<Session> {
        let name = rpc_service_name(instance);
        let service_name = ServiceName::new(&name).map_err(|error| {
            AeroError::client(format!(
                "\"{name}\" is not a usable iceoryx2 service name: {error:?}"
            ))
        })?;

        // `open`, not `open_or_create`. If the daemon is not running we want
        // to say so immediately; creating the service ourselves would leave a
        // request that can only ever time out, and would leave a stray
        // service behind for the daemon to trip over.
        let service = node
            .service_builder(&service_name)
            .request_response::<[u8], [u8]>()
            .request_user_header::<RequestHeader>()
            .response_user_header::<ReplyHeader>()
            .open()
            .map_err(|error| {
                // The daemon's service *is* there and full — a different problem
                // from not finding one, and one `why_not` would misdiagnose badly:
                // it reports "no daemon serves this instance" for a daemon that
                // does, because a full service is invisible to `open()`.
                if error == RequestResponseOpenError::ExceedsMaxNumberOfNodes {
                    return AeroError::client(pool_is_full(instance));
                }
                AeroError::client(format!(
                    "cannot attach to the Aerospike daemon for instance \"{instance}\" on service \
                     \"{name}\": {error:?}. {}",
                    why_not(instance)
                ))
            })?;

        // The other half of the same ceiling: `max_nodes` and `max_clients` are
        // both sized from `max-workers`, so which of the two is reported depends
        // on nothing an operator can see. Same message for both.
        let port = request_port(&service).map_err(|error| {
            if error == ClientCreateError::ExceedsMaxSupportedClients {
                return AeroError::client(pool_is_full(instance));
            }
            AeroError::client(format!(
                "could not create a request port for instance \"{instance}\": {error:?}"
            ))
        })?;

        let waiter =
            Waiter::attach(node, instance, client_id, settings.backoff).map_err(|error| {
                AeroError::client(format!(
                    "could not prepare the reply wakeup for instance \"{instance}\": {error}"
                ))
            })?;

        Ok(Session { port, waiter })
    }
}

/// The daemon is there, and has no room left for another worker.
///
/// Its own message because this is the failure an operator is most likely to
/// meet at scale and least likely to diagnose: everything checks out — the daemon
/// is running, the configuration is right, the other workers are serving traffic
/// — and *these* workers cannot attach at all. Two facts make it actionable, and
/// neither is guessable from an iceoryx2 error variant:
///
/// - the setting is called `max-workers`, and it has to be at least the size of
///   the PHP-FPM pool (or `pm.max_children`);
/// - it is fixed when the daemon **creates** the shared-memory service, so
///   raising it means restarting the daemon, and the restart only takes effect
///   once no worker is still attached to the old service.
///
/// Both limits it can come from — the node count and the client count — are the
/// same operator-visible cause, so they produce the same message rather than two
/// that would have to be recognised separately.
fn pool_is_full(instance: &str) -> String {
    format!(
        "the Aerospike daemon for instance \"{instance}\" has no room for another worker: its \
         `max-workers` limit is already reached. That limit is fixed when the daemon creates its \
         shared-memory service, so it cannot grow while the daemon is running. Raise `max-workers` \
         in the daemon's [daemon] section to at least the size of this PHP worker pool \
         (`pm.max_children` under PHP-FPM), then restart the daemon — the new limit takes effect \
         only once no worker is still attached to the old service. The slots are held by processes \
         that are still running: a worker killed outright does not keep its slot, because iceoryx2 \
         reclaims a dead process's registration when the next worker starts."
    )
}

/// Explain a failed attach, distinguishing "no daemon" from "the wrong one".
///
/// Worth the service enumeration: the extension and the daemon are paired by
/// version, and the service name carries it — so a daemon of another version is
/// invisible to `open()` and looks exactly like no daemon at all. That is the
/// mismatch an operator is most likely to hit, and the least likely to guess,
/// since a running process and a working configuration both check out.
///
/// Falls back to the generic advice if the services cannot be listed: this runs
/// on a path that is already failing, and it must not fail differently.
fn why_not(instance: &str) -> String {
    let mut others: Vec<String> = Vec::new();
    let mut instances: Vec<String> = Vec::new();

    let _ = iceoryx2::prelude::ipc::Service::list(Config::global_config(), |service| {
        if let Some((found, version)) =
            parse_rpc_service_name(service.static_details.name().as_str())
        {
            if found == instance && version != VERSION {
                others.push(version.to_owned());
            } else if version == VERSION && found != instance {
                instances.push(found.to_owned());
            }
        }
        CallbackProgression::Continue
    });

    if !others.is_empty() {
        others.sort_unstable();
        others.dedup();
        return format!(
            "A daemon for instance \"{instance}\" is running, but it is version {} and this \
             extension is {VERSION}. The two must be exactly the same version: install them \
             together and restart the daemon.",
            others.join(", ")
        );
    }
    if !instances.is_empty() {
        instances.sort_unstable();
        return format!(
            "A daemon of this version is running, but it does not serve instance \"{instance}\" \
             — it serves {}. Add a [cluster.{instance}] section to its configuration file, or \
             name one of those instances.",
            instances.join(", ")
        );
    }
    format!(
        "Is aerospike-php-daemon running, is it version {VERSION}, and is it configured with a \
         [cluster.{instance}] section?"
    )
}

/// The one place the request port's payload sizing is configured.
///
/// Both ends must size their payload pool the same way. Without the initial
/// slice length *and* the power-of-two growth strategy, loaning anything
/// larger than a single element fails with `ExceedsMaxLoanSize`. Factored out
/// so the tests below exercise the same configuration production uses rather
/// than a copy of it that could drift.
fn request_port(
    service: &RpcService,
) -> Result<Port, iceoryx2::service::port_factory::client::ClientCreateError> {
    service
        .client_builder()
        .initial_max_slice_len(INITIAL_MAX_SLICE_LEN)
        .allocation_strategy(AllocationStrategy::PowerOfTwo)
        .create()
}

/// A per-process nonce for [`ClientId`].
///
/// Only has to be unlikely to repeat for a *recycled pid*, so the wall clock
/// plus a counter is enough and avoids pulling in a random-number dependency.
fn mint_nonce() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static ATTACHES: AtomicU32 = AtomicU32::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let attach = ATTACHES.fetch_add(1, Ordering::Relaxed);
    nanos ^ attach.wrapping_mul(0x9E37_79B9)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// iceoryx2 writes `Default::default()` into the user header on loan,
    /// before the caller fills it in. That default must fail validation, so a
    /// request that was never populated cannot travel as a plausible frame.
    /// The contract pins this too; asserted here because this crate is what
    /// relies on it.
    #[test]
    fn a_defaulted_request_header_never_validates() {
        assert!(RequestHeader::default().validate(0).is_err());
    }

    /// Stand up a real service — as the daemon does — so the port
    /// configuration can be exercised without a daemon or a database.
    fn test_port(suffix: &str) -> Port {
        quiet_iceoryx_logging();
        let node = NodeBuilder::new().create::<ipc::Service>().unwrap();
        let name = format!("aerospike-ext-test/{}/{suffix}", std::process::id());
        let service = node
            .service_builder(&ServiceName::new(&name).unwrap())
            .request_response::<[u8], [u8]>()
            .request_user_header::<RequestHeader>()
            .response_user_header::<ReplyHeader>()
            .create()
            .unwrap();
        request_port(&service).unwrap()
    }

    /// The sizing on the request port is the thing most likely to be wrong:
    /// omitting either half of it turns every loan past one byte into
    /// `ExceedsMaxLoanSize`. Cover the three sizes that matter — PING's empty
    /// body, exactly the contract's initial slice length, and past it, where
    /// the power-of-two strategy has to grow the segment.
    #[test]
    fn the_request_port_can_loan_every_body_size_the_contract_allows() {
        let port = test_port("loan-sizes");

        // PING carries no body at all, so a zero-length loan must work.
        assert!(
            port.loan_slice_uninit(0).is_ok(),
            "a zero-length loan must work, or PING cannot be sent"
        );

        assert!(
            port.loan_slice_uninit(INITIAL_MAX_SLICE_LEN).is_ok(),
            "a loan of exactly INITIAL_MAX_SLICE_LEN must work without reallocating"
        );

        assert!(
            port.loan_slice_uninit(INITIAL_MAX_SLICE_LEN + 1).is_ok(),
            "a loan past INITIAL_MAX_SLICE_LEN must grow the segment rather than fail; \
             AllocationStrategy::PowerOfTwo is what makes that work"
        );
    }

    /// The header written into a loan must survive being handed to iceoryx2
    /// unchanged — this is the whole premise of a typed, zero-copy header.
    #[test]
    fn a_written_request_header_round_trips_through_the_loan() {
        let port = test_port("header-round-trip");
        let body = b"payload";

        let request = port.loan_slice_uninit(body.len()).unwrap();
        let mut request = request.write_from_slice(body);
        let written = RequestHeader::new(
            aerospike_php_ipc::opcode::PUT,
            42,
            ClientId::new(std::process::id(), 7),
            250,
            u32::try_from(body.len()).unwrap(),
        );
        *request.user_header_mut() = written;

        let read_back = *request.user_header();
        assert_eq!(read_back.opcode, aerospike_php_ipc::opcode::PUT);
        assert_eq!(read_back.seq, 42);
        assert_eq!(read_back.timeout_ms, 250);
        assert!(read_back.validate(body.len()).is_ok());
        assert_eq!(request.payload(), body);
    }
}
