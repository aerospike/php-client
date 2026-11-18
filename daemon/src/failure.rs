// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Turning a client error into the contract's [`StatusCode`] plus detail.
//!
//! | client error | reply |
//! |---|---|
//! | `Server` with `KeyNotFoundError` | `RECORD_NOT_FOUND` |
//! | any other `Server` | `SERVER` + result code + in-doubt |
//! | `InvalidNamespace` | `INVALID_REQUEST` + result code 20, never in doubt |
//! | `UdfBadResponse` | `SERVER` + result code 100 + in-doubt |
//! | `Timeout` | `TIMEOUT` |
//! | `Connection`, `ConnectionPoolEmpty`, `NoMoreConnections` | `CONNECTION` |
//! | anything else | `INTERNAL` |
//!
//! Two of those are **client-side kinds that nevertheless carry a real Aerospike
//! result code**, which is why they are named individually rather than falling
//! through to `INTERNAL`: `server_result_code()` does not see them, but the code
//! is one a caller can branch on and losing it would leave nothing but a message
//! to parse.
//!
//! Classification walks the **cause chain**, because the retry loop decorates
//! the error it finally returns: the timeout or connection failure that
//! actually happened is often one or two links down, and matching only the
//! outermost kind would report `INTERNAL` for an ordinary network blip.

use aerospike_core::{Error, ErrorKind, ResultCode};
use aerospike_php_ipc::StatusCode;

/// A failed operation, in the terms the reply header speaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// Outcome classification.
    pub status: StatusCode,
    /// Detail for [`ErrorBody::message`](aerospike_php_ipc::ErrorBody::message).
    pub message: String,
    /// Server result code and in-doubt marker, when the server spoke.
    pub server: Option<(i32, bool)>,
}

impl Failure {
    /// A failure the daemon itself diagnosed, with no server involvement.
    #[must_use]
    pub fn new(status: StatusCode, message: impl Into<String>) -> Failure {
        Failure {
            status,
            message: message.into(),
            server: None,
        }
    }
}

/// Classify a client error.
#[must_use]
pub fn classify(err: &Error) -> Failure {
    let message = err.to_string();

    // A server result code is authoritative wherever it sits in the chain: it
    // means the command reached a node and came back refused.
    if let Some(rc) = err.server_result_code() {
        if rc == ResultCode::KeyNotFoundError {
            return Failure {
                status: StatusCode::RECORD_NOT_FOUND,
                message,
                server: None,
            };
        }
        return Failure {
            status: StatusCode::SERVER,
            message,
            server: Some((err.result_code(), err.in_doubt())),
        };
    }

    // A permanent, caller-caused failure outranks whatever wraps it. The retry
    // loop gives up with `Timeout` on the outside, so classifying by the
    // outermost link would report "timed out" for a namespace that does not
    // exist and can never exist — retryable-looking, and it throws away the
    // one code the caller could act on. Observed shape:
    //
    //     Timeout(rc 9) -> InvalidNamespace(rc 20, "not found in partition map")
    //
    // `server_result_code()` does not see this because `InvalidNamespace` is a
    // client-side kind, not `ErrorKind::Server` — the request never reached a
    // node. The code is still a real Aerospike result code, so it is worth
    // forwarding: PHP can branch on 20 instead of parsing a message.
    for link in chain(err) {
        let (status, in_doubt) = match link.kind() {
            ErrorKind::InvalidNamespace => (
                StatusCode::INVALID_REQUEST,
                // Not in doubt, and deliberately so: no node was ever chosen,
                // so the write provably did not happen.
                false,
            ),
            // A UDF that failed is the *server* refusing, not the daemon: the
            // function ran on a node and errored, and result code 100 is what
            // the caller branches on. In doubt if the error says so — a
            // function that failed after its `aerospike:update` may well have
            // written.
            ErrorKind::UdfBadResponse => (StatusCode::SERVER, link.in_doubt()),
            _ => continue,
        };
        return Failure {
            status,
            // The cause we classified by, not the retry wrapper around it:
            // leading with "Timeout after 2 tries" would contradict the status
            // the caller just received.
            message: link.to_string(),
            server: Some((link.result_code(), in_doubt)),
        };
    }

    for link in chain(err) {
        let status = match link.kind() {
            ErrorKind::Timeout => StatusCode::TIMEOUT,
            ErrorKind::Connection
            | ErrorKind::ConnectionPoolEmpty
            | ErrorKind::NoMoreConnections => StatusCode::CONNECTION,
            _ => continue,
        };
        return Failure {
            status,
            message,
            server: None,
        };
    }

    Failure {
        status: StatusCode::INTERNAL,
        message,
        server: None,
    }
}

/// The error and every client error beneath it, outermost first.
fn chain(err: &Error) -> impl Iterator<Item = &Error> {
    let mut next = Some(err);
    core::iter::from_fn(move || {
        let current = next?;
        next = std::error::Error::source(current)
            .and_then(|source| source.downcast_ref::<Error>());
        Some(current)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // aerospike-core 3.0 keeps the constructors for server, timeout, connection
    // and UDF failures crate-private. Those mappings still live in `classify`;
    // what a unit test can build is the two public constructors.

    #[test]
    fn a_client_side_error_is_internal_and_keeps_its_message() {
        let err = Error::client_error("something else");
        let failure = classify(&err);
        assert_eq!(failure.status, StatusCode::INTERNAL);
        assert_eq!(failure.server, None);
        assert!(failure.message.contains("something else"));
    }

    #[test]
    fn an_invalid_argument_stays_internal_under_a_wrapper() {
        // `chain_error` is the public half of the retry decoration. The cause
        // is still a client error, so the status does not change, and the
        // message still names what was wrong.
        let wrapped = Error::invalid_argument("bad bin").chain_error("write failed");
        let failure = classify(&wrapped);
        assert_eq!(failure.status, StatusCode::INTERNAL);
        assert!(failure.message.contains("bad bin"), "{}", failure.message);
    }

    #[test]
    fn the_message_is_never_empty() {
        for err in [Error::client_error("x"), Error::invalid_argument("y")] {
            assert!(!classify(&err).message.trim().is_empty());
        }
    }
}
