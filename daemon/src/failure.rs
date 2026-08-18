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

    #[test]
    fn a_missing_record_is_not_a_server_error() {
        let err = Error::server_error(ResultCode::KeyNotFoundError, "node-1", None);
        let failure = classify(&err);
        assert_eq!(failure.status, StatusCode::RECORD_NOT_FOUND);
        assert_eq!(failure.server, None);
        assert!(!failure.message.is_empty());
    }

    #[test]
    fn other_server_codes_carry_the_code_and_in_doubt_marker() {
        let err = Error::server_error(ResultCode::KeyExistsError, "node-1", None);
        let failure = classify(&err);
        assert_eq!(failure.status, StatusCode::SERVER);
        assert_eq!(failure.server, Some((5, false)));

        // A write that may have been applied must say so, or PHP could retry a
        // non-idempotent operation.
        let doubtful = Error::server_error(ResultCode::Timeout, "node-1", None)
            .set_in_doubt(true, 1);
        let failure = classify(&doubtful);
        assert_eq!(failure.status, StatusCode::SERVER);
        assert_eq!(failure.server, Some((9, true)));
    }

    #[test]
    fn client_side_failures_map_to_their_own_statuses() {
        assert_eq!(
            classify(&Error::timeout("deadline elapsed")).status,
            StatusCode::TIMEOUT
        );
        assert_eq!(
            classify(&Error::connection("broken pipe")).status,
            StatusCode::CONNECTION
        );
        assert_eq!(
            classify(&Error::pool_empty()).status,
            StatusCode::CONNECTION
        );
        assert_eq!(
            classify(&Error::no_more_connections()).status,
            StatusCode::CONNECTION
        );
        assert_eq!(
            classify(&Error::client_error("something else")).status,
            StatusCode::INTERNAL
        );
        assert_eq!(
            classify(&Error::bad_response("garbage")).status,
            StatusCode::INTERNAL
        );
    }

    #[test]
    fn classification_looks_through_retry_decoration() {
        // What the retry loop returns is a wrapper; the real cause is below.
        let wrapped = Error::connection("connection reset").chain_error("write failed");
        assert_eq!(classify(&wrapped).status, StatusCode::CONNECTION);

        let deep = Error::timeout("elapsed")
            .chain_error("attempt 3")
            .chain_error("command failed");
        assert_eq!(classify(&deep).status, StatusCode::TIMEOUT);

        // A server code buried in the chain still wins over the wrapper.
        let server = Error::server_error(ResultCode::KeyNotFoundError, "node-1", None)
            .chain_error("read failed");
        assert_eq!(classify(&server).status, StatusCode::RECORD_NOT_FOUND);
    }

    #[test]
    fn a_bad_namespace_is_a_bad_request_not_a_timeout() {
        // The shape a live cluster actually produces for a namespace that does
        // not exist: the retry loop exhausts its attempts and returns Timeout
        // wrapping the real, permanent cause. Reporting TIMEOUT here would
        // invite a pointless retry and hide result code 20 from the caller.
        let err = Error::invalid_namespace("Namespace not found in partition map: nosuchns")
            .chain_error("Timeout after 2 tries");
        let failure = classify(&err);
        assert_eq!(failure.status, StatusCode::INVALID_REQUEST);
        assert_eq!(
            failure.server,
            Some((i32::from(u8::from(ResultCode::InvalidNamespace)), false)),
            "the caller needs the code to branch on, and the write provably \
             never reached a node so it is not in doubt"
        );
        assert!(failure.message.contains("nosuchns"));
    }

    /// A UDF that errored is the server refusing, not the daemon failing: the
    /// function ran on a node, and result code 100 is what a caller branches on.
    /// Reporting `INTERNAL` would say the daemon broke, and would throw the code
    /// away.
    #[test]
    fn a_failed_udf_is_a_server_failure_carrying_its_code() {
        let err = Error::udf_bad_response("1000:this function always refuses");
        let failure = classify(&err);
        assert_eq!(failure.status, StatusCode::SERVER);
        assert_eq!(
            failure.server,
            Some((i32::from(u8::from(ResultCode::UdfBadResponse)), false))
        );
        assert!(failure.message.contains("refuses"), "{}", failure.message);

        // And through the retry decoration, as every other classification is.
        let wrapped = Error::udf_bad_response("boom").chain_error("Timeout after 2 tries");
        assert_eq!(classify(&wrapped).status, StatusCode::SERVER);

        // The in-doubt marker is read from the error rather than assumed, and
        // for this kind the client never sets one — only `Timeout` and
        // `Connection` carry the flag. So a failed UDF always reports
        // `in_doubt = false`, which is worth pinning because it is *not* the
        // same as "provably did not write": a function that errored after its
        // `aerospike:update` did write, and nothing in the client says so.
        let marked = Error::udf_bad_response("boom").set_in_doubt(true, 1);
        assert_eq!(
            classify(&marked).server,
            Some((i32::from(u8::from(ResultCode::UdfBadResponse)), false)),
            "the client does not carry in-doubt on a UDF failure; if it ever \
             does, this classification already forwards it"
        );
    }

    #[test]
    fn the_message_is_never_empty() {
        for err in [
            Error::timeout("t"),
            Error::connection("c"),
            Error::client_error("x"),
            Error::server_error(ResultCode::ParameterError, "n", None),
        ] {
            assert!(!classify(&err).message.trim().is_empty());
        }
    }
}
