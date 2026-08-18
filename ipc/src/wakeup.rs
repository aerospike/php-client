// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! How a waiting side learns that its counterpart has written something.
//!
//! Two strategies ship, selected by the `shm-poll` feature. Both
//! expose the same three calls — [`Waiter::attach`],
//! [`Waiter::wait_hint`] and [`ReplyNotifier::notify`] — so the daemon
//! and the extension are written once and compile either way:
//!
//! ```text
//! loop {
//!     if let Some(reply) = pending.receive()? { break reply }
//!     if Instant::now() >= deadline { return Err(Timeout) }
//!     waiter.wait_hint(deadline - Instant::now())?;
//! }
//! ```
//!
//! # Default: event ([`WakeupMode::Event`])
//!
//! Each worker owns a small iceoryx2 event service and blocks on its
//! listener; the daemon notifies that service after writing the reply. A
//! waiting worker costs essentially no CPU, at the price of a syscall per
//! wakeup.
//!
//! Note that this is *not* shared-memory-only: payloads always travel
//! through shared memory, but iceoryx2's `ipc::Service` implements events
//! over a unix datagram socket (`iceoryx2_cal::event::recommended::Ipc`).
//! The shared-memory alternative it ships
//! (`sem_bitset_posix_shared_memory`) needs process-shared POSIX
//! semaphores, which Darwin does not provide — very likely why the socket
//! backend is iceoryx2's default too.
//!
//! # With `shm-poll` ([`WakeupMode::ShmPoll`])
//!
//! Nothing but shared memory is touched. [`wait_hint`](Waiter::wait_hint)
//! spins, then yields, then sleeps in short capped steps
//! ([`Backoff`]) while the caller re-reads its response
//! port, and [`ReplyNotifier::notify`] is a no-op because the reader will
//! see the reply on its next poll.
//!
//! **Measured, this buys about 10µs of latency for roughly 17× the CPU in
//! the waiting process** (see `bench/README.md`): ~192µs of CPU per
//! operation against ~11µs, because a polling worker pins a core for the
//! whole round trip. That is free at one worker and ruinous at two
//! hundred, which is why it is the opt-in rather than the default. Choose
//! it when worker concurrency is low, latency is critical, or the
//! transport must provably stay in shared memory.
//!
//! Event services are named per worker
//! ([`event_service_name`](crate::event_service_name)) so that a reply
//! wakes exactly one process. A single shared service would wake every
//! waiting worker on every reply.

use core::time::Duration;

use iceoryx2::node::Node;
use iceoryx2::service::Service;

use crate::{Backoff, BackoffState, ClientId};

/// Which wakeup strategy this build was compiled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeupMode {
    /// Poll shared memory with a tiered backoff. No other IPC mechanism
    /// is involved.
    ShmPoll,
    /// Block on an iceoryx2 event port, which under `ipc::Service` is a
    /// unix datagram socket.
    Event,
}

impl core::fmt::Display for WakeupMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WakeupMode::ShmPoll => f.write_str("shm-poll"),
            WakeupMode::Event => f.write_str("event"),
        }
    }
}

/// The strategy compiled into this build.
#[must_use]
pub const fn mode() -> WakeupMode {
    if !cfg!(feature = "shm-poll") {
        WakeupMode::Event
    } else {
        WakeupMode::ShmPoll
    }
}

/// Longest single block in event mode. Bounded so the caller rechecks its
/// own deadline (and notices a dead daemon) even if a notification is
/// missed entirely.
#[cfg(not(feature = "shm-poll"))]
const MAX_EVENT_BLOCK: Duration = Duration::from_millis(50);

/// Failure to set up or use a wakeup channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeupError {
    /// What was being attempted.
    pub context: &'static str,
    /// Detail from the underlying transport.
    pub detail: String,
}

impl WakeupError {
    #[cfg(not(feature = "shm-poll"))]
    fn new(context: &'static str, detail: impl core::fmt::Debug) -> WakeupError {
        WakeupError {
            context,
            detail: format!("{detail:?}"),
        }
    }
}

impl core::fmt::Display for WakeupError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.context, self.detail)
    }
}

impl core::error::Error for WakeupError {}

// ===== Waiter (the side that waits for a reply) =============================

/// Client-side half: how a PHP worker waits for its reply.
pub struct Waiter<S: Service> {
    backoff: BackoffState,
    #[cfg(not(feature = "shm-poll"))]
    listener: iceoryx2::port::listener::Listener<S>,
    #[cfg(feature = "shm-poll")]
    _service: core::marker::PhantomData<S>,
}

impl<S: Service> Waiter<S> {
    /// Prepare to wait.
    ///
    /// In [`WakeupMode::ShmPoll`] this touches nothing outside shared
    /// memory: `node`, `instance` and `client_id` are accepted only so
    /// both builds share one signature.
    ///
    /// # Errors
    /// [`WakeupError`] if the event service cannot be opened (event mode
    /// only).
    #[cfg(feature = "shm-poll")]
    pub fn attach(
        _node: &Node<S>,
        _instance: &str,
        _client_id: ClientId,
        backoff: Backoff,
    ) -> Result<Waiter<S>, WakeupError> {
        Ok(Waiter {
            backoff: backoff.start(),
            _service: core::marker::PhantomData,
        })
    }

    /// Prepare to wait by opening this worker's event service.
    ///
    /// # Errors
    /// [`WakeupError`] if the service or listener cannot be created.
    #[cfg(not(feature = "shm-poll"))]
    pub fn attach(
        node: &Node<S>,
        instance: &str,
        client_id: ClientId,
        backoff: Backoff,
    ) -> Result<Waiter<S>, WakeupError> {
        let name = crate::event_service_name(instance, client_id);
        let service = node
            .service_builder(
                &name
                    .as_str()
                    .try_into()
                    .map_err(|e| WakeupError::new("event service name", e))?,
            )
            .event()
            .open_or_create()
            .map_err(|e| WakeupError::new("open event service", e))?;
        let listener = service
            .listener_builder()
            .create()
            .map_err(|e| WakeupError::new("create listener", e))?;
        Ok(Waiter {
            backoff: backoff.start(),
            listener,
        })
    }

    /// Pause before the caller polls its response port again.
    ///
    /// `remaining` is how long the caller is still willing to wait; it
    /// bounds any block so the deadline stays authoritative.
    ///
    /// # Errors
    /// [`WakeupError`] if the listener fails (event mode only).
    #[cfg(feature = "shm-poll")]
    #[allow(clippy::unnecessary_wraps)]
    pub fn wait_hint(&mut self, _remaining: Duration) -> Result<(), WakeupError> {
        self.backoff.snooze();
        Ok(())
    }

    /// Block until notified, `remaining` elapses, or an internal 50ms cap
    /// passes — whichever comes first.
    ///
    /// A missed or spurious notification is harmless: the caller re-polls
    /// its response port after every hint, so the bounded block degrades
    /// to slow polling rather than a hang.
    ///
    /// # Errors
    /// [`WakeupError`] if the listener fails.
    #[cfg(not(feature = "shm-poll"))]
    pub fn wait_hint(&mut self, remaining: Duration) -> Result<(), WakeupError> {
        let block = remaining.min(MAX_EVENT_BLOCK);
        if block.is_zero() {
            return Ok(());
        }
        self.listener
            .timed_wait_one(block)
            .map_err(|e| WakeupError::new("wait on listener", e))?;
        self.backoff.note_wait();
        // Drain any coalesced notifications so a backlog cannot make the
        // next wait return instantly in a tight loop.
        while self
            .listener
            .try_wait_one()
            .map_err(|e| WakeupError::new("drain listener", e))?
            .is_some()
        {}
        Ok(())
    }

    /// Reset the escalation so the next operation starts by spinning
    /// again. Call this between requests; without it a long-slept worker
    /// would keep the slow cadence for every later call.
    pub fn rearm(&mut self, backoff: Backoff) {
        self.backoff = backoff.start();
    }

    /// How many hints the current wait has taken — useful to tell a
    /// spin-served reply from a slept-through one in diagnostics.
    #[must_use]
    pub const fn hints(&self) -> u32 {
        self.backoff.steps()
    }
}

// ===== Notifier (the side that produces a reply) ============================

/// Daemon-side half: how the daemon wakes the worker it just answered.
///
/// In [`WakeupMode::ShmPoll`] this is inert. Construct one per client id
/// regardless and the daemon needs no feature-specific code; caching them
/// is worthwhile in event mode, where each carries a real port.
pub struct ReplyNotifier<S: Service> {
    #[cfg(not(feature = "shm-poll"))]
    notifier: iceoryx2::port::notifier::Notifier<S>,
    #[cfg(feature = "shm-poll")]
    _service: core::marker::PhantomData<S>,
}

impl<S: Service> ReplyNotifier<S> {
    /// Open the notify side for `client_id`.
    ///
    /// # Errors
    /// [`WakeupError`] if the event service cannot be opened (event mode
    /// only).
    #[cfg(feature = "shm-poll")]
    #[allow(clippy::unnecessary_wraps)]
    pub fn open(
        _node: &Node<S>,
        _instance: &str,
        _client_id: ClientId,
    ) -> Result<ReplyNotifier<S>, WakeupError> {
        Ok(ReplyNotifier {
            _service: core::marker::PhantomData,
        })
    }

    /// Open the notify side for `client_id`.
    ///
    /// # Errors
    /// [`WakeupError`] if the service or notifier cannot be created.
    #[cfg(not(feature = "shm-poll"))]
    pub fn open(
        node: &Node<S>,
        instance: &str,
        client_id: ClientId,
    ) -> Result<ReplyNotifier<S>, WakeupError> {
        let name = crate::event_service_name(instance, client_id);
        let service = node
            .service_builder(
                &name
                    .as_str()
                    .try_into()
                    .map_err(|e| WakeupError::new("event service name", e))?,
            )
            .event()
            .open_or_create()
            .map_err(|e| WakeupError::new("open event service", e))?;
        let notifier = service
            .notifier_builder()
            .create()
            .map_err(|e| WakeupError::new("create notifier", e))?;
        Ok(ReplyNotifier { notifier })
    }

    /// Signal that a reply is available.
    ///
    /// A no-op in [`WakeupMode::ShmPoll`]: the reader sees the reply on
    /// its next poll of shared memory.
    ///
    /// # Errors
    /// [`WakeupError`] if notification fails (event mode only). A failure
    /// here is not fatal to the operation — the reply is already in
    /// shared memory — so callers should log and continue rather than
    /// abandon the request.
    #[cfg(feature = "shm-poll")]
    #[allow(clippy::unnecessary_wraps)]
    pub fn notify(&self) -> Result<(), WakeupError> {
        Ok(())
    }

    /// Signal that a reply is available.
    ///
    /// # Errors
    /// [`WakeupError`] if notification fails. Not fatal: the reply is
    /// already in shared memory and a bounded block will find it.
    #[cfg(not(feature = "shm-poll"))]
    pub fn notify(&self) -> Result<(), WakeupError> {
        self.notifier
            .notify()
            .map(|_| ())
            .map_err(|e| WakeupError::new("notify", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iceoryx2::prelude::*;

    #[test]
    fn mode_matches_the_compiled_feature() {
        if !cfg!(feature = "shm-poll") {
            assert_eq!(mode(), WakeupMode::Event);
            assert_eq!(mode().to_string(), "event");
        } else {
            assert_eq!(mode(), WakeupMode::ShmPoll);
            assert_eq!(mode().to_string(), "shm-poll");
        }
    }

    #[test]
    fn waiter_and_notifier_pair_up_in_either_mode() {
        let node = NodeBuilder::new().create::<ipc::Service>().unwrap();
        let client_id = ClientId::new(std::process::id(), 7);

        let mut waiter =
            Waiter::attach(&node, "wakeup-test", client_id, Backoff::default()).unwrap();
        let notifier = ReplyNotifier::open(&node, "wakeup-test", client_id).unwrap();

        // Notifying before anyone waits must not error in either mode.
        notifier.notify().unwrap();

        // A hint must return promptly and never hang, whether it spun or
        // consumed the notification above.
        let started = std::time::Instant::now();
        waiter.wait_hint(Duration::from_millis(5)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn rearm_restarts_the_escalation() {
        let node = NodeBuilder::new().create::<ipc::Service>().unwrap();
        let client_id = ClientId::new(std::process::id(), 8);
        let mut waiter =
            Waiter::attach(&node, "wakeup-rearm", client_id, Backoff::default()).unwrap();

        for _ in 0..3 {
            waiter.wait_hint(Duration::from_millis(1)).unwrap();
        }
        assert!(waiter.hints() >= 3);

        waiter.rearm(Backoff::default());
        assert_eq!(waiter.hints(), 0);
    }
}
