// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! `Aerospike\AerospikeException` and the Rust error it is built from.

use aerospike_php_ipc::{CodecError, ErrorBody, ReplyHeader, StatusCode, VERSION, decode_body};
use ext_php_rs::class::RegisteredClass;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::exception::PhpException;
use ext_php_rs::prelude::*;

use crate::enums::{Case, Status};

/// Label used when a failure never reached the daemon at all, so no
/// [`StatusCode`] applies — a missing daemon, a malformed frame, a value PHP
/// could not express.
pub const CLIENT_STATUS_LABEL: &str = "CLIENT";

/// Every failure this extension raises.
///
/// `getStatus()` returns an `Aerospike\Status` — a real PHP enum, so it can be
/// `match`ed exhaustively — which classifies *whose* problem the failure is.
/// `getResultCode()` carries the server's own result code when the daemon
/// attached one, which is the number Aerospike's documentation lists.
///
/// `getCode()`, which `Exception` declares, is the server result code when
/// there is one and the numeric status otherwise. Prefer the two accessors
/// above: they say which of the two you are looking at.
#[php_class]
#[php(name = "Aerospike\\AerospikeException")]
#[php(extends(ce = ext_php_rs::zend::ce::exception, stub = "\\Exception"))]
#[derive(Debug, Default)]
pub struct AerospikeException {
    /// Shadows the `message` that `Exception` declares.
    ///
    /// `Exception::getMessage()` is `final`, and its declared slot is
    /// `protected`, so writing that slot from Rust fails Zend's access check.
    /// Registering `message` as a property of *this* class instead means
    /// ext-php-rs' property handler answers the read before it ever reaches
    /// the parent's slot — which is the pattern ext-php-rs documents for
    /// stateful exceptions, and keeps `getMessage()` working.
    #[php(prop, name = "message")]
    message: String,
    /// Shadows `Exception`'s `code`, for the same reason as `message`.
    #[php(prop, name = "code")]
    code: i64,
    /// How the failure is classified.
    ///
    /// A property as well as an accessor, so `var_dump($e)` shows it: an
    /// exception whose classification is only reachable through a method call
    /// is one nobody sees while debugging.
    ///
    /// Wrapped in [`Case`] because that is what converts an enum case to PHP
    /// without corrupting its reference count; see [`Case`] for the crash that
    /// caused.
    #[php(prop, name = "status")]
    status: Case<Status>,
    /// The server's own result code, when the daemon attached one.
    #[php(prop, name = "resultCode")]
    result_code: Option<i64>,
    /// Whether a failed write may nevertheless have been applied. A `true`
    /// here means a non-idempotent operation must not simply be retried.
    #[php(prop, name = "inDoubt")]
    in_doubt: bool,
}

#[php_impl]
impl AerospikeException {
    /// Construct one by hand.
    ///
    /// The extension always throws fully-populated instances; this exists so
    /// that application code can rethrow, and tests can construct, an
    /// instance of this class. An instance built here reports
    /// [`Status::Client`], since no daemon status applies to it.
    ///
    /// Unlike `Exception::__construct` there is no `$previous` argument:
    /// accepting one and silently dropping it would be worse than not
    /// offering it.
    #[php(defaults(message = None, code = None))]
    pub fn __construct(message: Option<String>, code: Option<i64>) -> AerospikeException {
        AerospikeException {
            message: message.unwrap_or_default(),
            code: code.unwrap_or(0),
            status: Case(Status::Client),
            result_code: None,
            in_doubt: false,
        }
    }

    /// How the failure is classified: an `Aerospike\Status`.
    pub fn get_status(&self) -> Case<Status> {
        self.status
    }

    /// The server's result code, or `null` when the failure did not come
    /// from the server.
    pub fn get_result_code(&self) -> Option<i64> {
        self.result_code
    }

    /// Whether a failed write may still have been applied.
    pub fn is_in_doubt(&self) -> bool {
        self.in_doubt
    }
}

/// A failure on its way to PHP.
#[derive(Debug, Clone)]
pub struct AeroError {
    message: String,
    /// `None` when the failure is local to this process.
    status: Option<StatusCode>,
    result_code: Option<i32>,
    in_doubt: bool,
}

/// Result of anything that can fail on the way to or from the daemon.
pub type AeroResult<T> = Result<T, AeroError>;

impl AeroError {
    /// A failure that never reached the daemon.
    pub fn client(message: impl Into<String>) -> AeroError {
        AeroError {
            message: message.into(),
            status: None,
            result_code: None,
            in_doubt: false,
        }
    }

    /// The worker gave up waiting.
    ///
    /// Reported as [`StatusCode::TIMEOUT`] and marked in-doubt: the daemon
    /// may well have completed the operation, and this side has no way to
    /// find out, which is precisely the situation `IN_DOUBT` exists to
    /// describe.
    pub fn timeout(operation: &str, timeout_ms: u32) -> AeroError {
        AeroError {
            message: format!(
                "{operation} did not complete within {timeout_ms}ms; the daemon may still be \
                 working on it, so a write must not simply be retried"
            ),
            status: Some(StatusCode::TIMEOUT),
            result_code: None,
            in_doubt: true,
        }
    }

    /// A framing failure.
    ///
    /// [`CodecError::VersionMismatch`] gets its own wording: the two halves are
    /// matched by version, so this is the "they were installed from different
    /// releases" case and the fix is the whole diagnosis.
    pub fn codec(error: CodecError) -> AeroError {
        let message = match error {
            CodecError::VersionMismatch { .. } => format!(
                "this extension is version {VERSION}, and the daemon that answered was built from \
                 a different one. The two must be the same version: install them together and \
                 restart the daemon. Its version is in its startup log, and in \
                 Aerospike\\Client::ping()."
            ),
            other => format!("the daemon sent a frame this extension cannot read: {other}"),
        };
        AeroError {
            message,
            status: None,
            result_code: None,
            in_doubt: false,
        }
    }

    /// A non-OK reply.
    ///
    /// The human-readable detail lives in the payload as an [`ErrorBody`];
    /// a payload that does not decode must still produce a usable error, so
    /// the status label stands in for it.
    pub fn from_reply(header: &ReplyHeader, payload: &[u8], operation: &str) -> AeroError {
        let status = header.status();
        let detail = decode_body::<ErrorBody>(payload)
            .map(|body| body.message)
            .unwrap_or_default();
        let message = if detail.is_empty() {
            format!("{operation} failed: {}", status.label())
        } else {
            format!("{operation} failed: {} ({detail})", status.label())
        };
        AeroError {
            message,
            status: Some(status),
            result_code: header.result_code(),
            in_doubt: header.in_doubt(),
        }
    }

    /// Prefix the message, keeping the classification intact.
    #[must_use]
    pub fn context(mut self, prefix: &str) -> AeroError {
        self.message = format!("{prefix}: {}", self.message);
        self
    }

    fn php_code(&self) -> i32 {
        self.result_code
            .unwrap_or_else(|| self.status.map_or(0, |status| i32::from(status.0)))
    }

    /// Build the PHP exception object.
    ///
    /// The state has to be carried by a real object — `zend_throw_exception`
    /// would give us the right class but leave every property at its default
    /// — so the object is constructed here with its Rust state already
    /// populated and thrown as-is.
    fn into_exception(self) -> PhpException {
        let code = self.php_code();
        let entry = AerospikeException::get_metadata().ce();
        let state = AerospikeException {
            message: self.message.clone(),
            code: i64::from(code),
            status: Case(Status::of(self.status)),
            result_code: self.result_code.map(i64::from),
            in_doubt: self.in_doubt,
        };

        match state.into_zval(true) {
            Ok(zval) => PhpException::new(self.message, code, entry).with_object(zval),
            // If the object could not be built we still must not lose the
            // error; throw the plain class and accept that the extra
            // properties read as their defaults.
            Err(_) => PhpException::new(self.message, code, entry),
        }
    }
}

impl std::fmt::Display for AeroError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AeroError {}

/// Lets every fallible helper in this crate be used with `?` from a
/// `PhpResult` method.
impl From<AeroError> for PhpException {
    fn from(error: AeroError) -> PhpException {
        error.into_exception()
    }
}

/// ext-php-rs' own failures (a hashtable insert, a string with a NUL byte)
/// are programming or allocation errors rather than Aerospike ones, but they
/// still have to reach PHP as something.
pub fn php_internal(error: ext_php_rs::error::Error, what: &str) -> AeroError {
    AeroError::client(format!("could not build the PHP value for {what}: {error}"))
}
