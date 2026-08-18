// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Optional arguments that are actually type-checked.
//!
//! # The hole this closes
//!
//! PHP does not verify argument types for *internal* functions. The declared
//! types are real — reflection and generated stubs both show them, so an IDE
//! and a static analyser see `?Aerospike\WritePolicy` — but at runtime nothing
//! in the engine enforces them; an extension is expected to check for itself.
//!
//! `ext_php_rs` does check, and raises for a non-nullable parameter. For a
//! **nullable** one it cannot: its conversion has only two outcomes, and
//! "absent" and "present but the wrong type" collapse into the same `None`. The
//! generated code then substitutes the default. So under the obvious spelling
//! of an optional typed argument:
//!
//! ```php
//! new Aerospike\WritePolicy(replica: 'MASTER');            // string, not Replica
//! $client->put(new Aerospike\ReadPolicy(), $key, $bins);   // a read policy
//! ```
//!
//! …both are accepted, and both silently become "no override" and "no policy".
//! A policy that quietly is not the one you wrote is precisely the failure this
//! API exists to prevent, and it is worse than the associative array it
//! replaced, which at least refused an unknown key.
//!
//! [`Given`] closes it. Its conversion never fails, so a wrong type survives as
//! [`Given::Wrong`] instead of being erased, and [`Given::or_none`] turns that
//! into a `TypeError` naming the argument, what it expects, and what it got.
//! The declared type is untouched: `Option<Given<&WritePolicy>>` reports itself
//! to PHP as `?Aerospike\WritePolicy`, exactly as `Option<&WritePolicy>` does.

use ext_php_rs::convert::FromZval;
use ext_php_rs::exception::PhpException;
use ext_php_rs::flags::DataType;
use ext_php_rs::prelude::PhpResult;
use ext_php_rs::types::Zval;
use ext_php_rs::zend::ce;

/// What PHP passed for an optional, typed argument.
///
/// Wrap an optional parameter as `Option<Given<T>>` and resolve it with
/// [`Given::or_none`]:
///
/// ```ignore
/// pub fn put(&self, policy: Option<Given<&WritePolicy>>, /* .. */) -> PhpResult<()> {
///     let policy = Given::or_none(policy, "policy")?;
///     // ..
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Given<T> {
    /// `null` was passed, which for every optional argument here means the same
    /// thing as omitting it.
    Empty,
    /// Something of the wrong type was passed, named for the message.
    Wrong(String),
    /// A value of the expected type.
    Value(T),
}

impl<'a, T: FromZval<'a>> Given<T> {
    /// Resolve an optional argument, raising a `TypeError` for a wrong type.
    ///
    /// `None` (omitted) and [`Given::Empty`] (`null`) both give `Ok(None)`:
    /// every optional argument in this extension means "leave it alone", and
    /// omitting it and passing `null` are two spellings of that.
    ///
    /// # Errors
    /// A PHP `TypeError` naming the argument, the type it wants and the type it
    /// got — worded like the engine's own, so it reads as if PHP had raised it.
    pub fn or_none(given: Option<Given<T>>, argument: &str) -> PhpResult<Option<T>> {
        Self::checked_or_none(given, argument).map_err(type_error)
    }

    /// [`Given::or_none`] with the complaint as a plain string.
    ///
    /// Split out so the rules and their wording are unit-testable: building a
    /// `TypeError` needs PHP's own class entry, which only exists inside a PHP
    /// process, so a test that constructed one would have to be written in PHP.
    fn checked_or_none(given: Option<Given<T>>, argument: &str) -> Result<Option<T>, String> {
        match given {
            None | Some(Given::Empty) => Ok(None),
            Some(Given::Value(value)) => Ok(Some(value)),
            Some(Given::Wrong(actual)) => Err(mismatch(
                argument,
                &php_type_name(&T::TYPE),
                &actual,
                Nullable::Yes,
            )),
        }
    }

    /// Resolve a *required* typed argument.
    ///
    /// Used where `ext_php_rs` would otherwise raise a plain `Exception` reading
    /// "Invalid value given for argument `key`" — true, but it names neither the
    /// type that was wanted nor the one that arrived, and it is not the
    /// `TypeError` a PHP caller catches.
    ///
    /// # Errors
    /// A PHP `TypeError`, whether the argument was `null` or of the wrong type.
    pub fn required(self, argument: &str) -> PhpResult<T> {
        self.checked_required(argument).map_err(type_error)
    }

    /// [`Given::required`] with the complaint as a plain string, for the reason
    /// [`Given::checked_or_none`] gives.
    fn checked_required(self, argument: &str) -> Result<T, String> {
        let expected = php_type_name(&T::TYPE);
        match self {
            Given::Value(value) => Ok(value),
            Given::Empty => Err(mismatch(argument, &expected, "null", Nullable::No)),
            Given::Wrong(actual) => Err(mismatch(argument, &expected, &actual, Nullable::No)),
        }
    }
}

/// Whether the argument being complained about accepts `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nullable {
    Yes,
    No,
}

/// The wording of a type complaint.
///
/// Deliberately the same shape as the engine's own — `$x must be of type ?int,
/// string given` — because a caller should not be able to tell from the message
/// that this argument was checked by an extension rather than by PHP.
fn mismatch(argument: &str, expected: &str, actual: &str, nullable: Nullable) -> String {
    let marker = if nullable == Nullable::Yes { "?" } else { "" };
    format!("${argument} must be of type {marker}{expected}, {actual} given")
}

/// A PHP `TypeError`, which is what a wrong argument type deserves to be.
///
/// Not an `Aerospike\AerospikeException`: a wrong type is a mistake in the
/// calling code, not a database failure, and an application's `catch
/// (AerospikeException)` around a query should not swallow it.
#[must_use]
pub fn type_error(message: String) -> PhpException {
    PhpException::new(message, 0, ce::type_error())
}

/// Name the type of a value PHP passed, for a message about it.
#[must_use]
pub fn type_of(zval: &Zval) -> String {
    describe(zval)
}

impl<'a, T: FromZval<'a>> FromZval<'a> for Given<T> {
    /// The declared type is the wrapped type's, which is what keeps the PHP
    /// signature honest.
    const TYPE: DataType = T::TYPE;

    /// Never `None`. That is the whole point: returning `None` for a wrong type
    /// is what lets it be mistaken for an absent argument.
    fn from_zval(zval: &'a Zval) -> Option<Given<T>> {
        if zval.is_null() {
            return Some(Given::Empty);
        }
        Some(match T::from_zval(zval) {
            Some(value) => Given::Value(value),
            None => Given::Wrong(describe(zval)),
        })
    }
}

/// The PHP name for a declared type, for the "must be of type X" half.
fn php_type_name(ty: &DataType) -> String {
    match ty {
        DataType::Long => "int".to_owned(),
        DataType::Double => "float".to_owned(),
        DataType::String => "string".to_owned(),
        DataType::Bool | DataType::True | DataType::False => "bool".to_owned(),
        DataType::Array => "array".to_owned(),
        DataType::Object(Some(class)) => (*class).to_owned(),
        DataType::Object(None) => "object".to_owned(),
        DataType::Callable => "callable".to_owned(),
        DataType::Iterable => "iterable".to_owned(),
        DataType::Mixed => "mixed".to_owned(),
        other => other.to_string(),
    }
}

/// The PHP name for what was actually passed, for the "X given" half.
///
/// A wrong *object* is named by its class, because "object given" would not
/// help anyone who passed a `ReadPolicy` where a `WritePolicy` belongs — which
/// is the mistake most worth catching here.
fn describe(zval: &Zval) -> String {
    if let Some(object) = zval.object() {
        return object
            .get_class_name()
            .unwrap_or_else(|_| "object".to_owned());
    }
    match zval.get_type() {
        DataType::Null | DataType::Undef => "null",
        DataType::True | DataType::False | DataType::Bool => "bool",
        DataType::Long => "int",
        DataType::Double => "float",
        DataType::String => "string",
        DataType::Array => "array",
        DataType::Resource => "resource",
        DataType::Callable => "callable",
        _ => "a value of an unsupported type",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names in the message are PHP's, not Rust's or ext-php-rs': a message
    /// saying "must be of type Long" would be about the wrong language.
    #[test]
    fn declared_types_are_named_the_way_php_names_them() {
        assert_eq!(php_type_name(&DataType::Long), "int");
        assert_eq!(php_type_name(&DataType::Double), "float");
        assert_eq!(php_type_name(&DataType::Bool), "bool");
        assert_eq!(php_type_name(&DataType::String), "string");
        assert_eq!(php_type_name(&DataType::Array), "array");
        assert_eq!(
            php_type_name(&DataType::Object(Some("Aerospike\\WritePolicy"))),
            "Aerospike\\WritePolicy"
        );
        assert_eq!(php_type_name(&DataType::Object(None)), "object");
    }

    /// `None` and `Empty` are the same answer, and must stay so: an optional
    /// argument that behaved differently when passed `null` explicitly would be
    /// a trap of its own.
    #[test]
    fn omitted_and_null_both_mean_no_value() {
        assert_eq!(Given::<i64>::checked_or_none(None, "x"), Ok(None));
        assert_eq!(
            Given::<i64>::checked_or_none(Some(Given::Empty), "x"),
            Ok(None)
        );
        assert_eq!(
            Given::<i64>::checked_or_none(Some(Given::Value(7)), "x"),
            Ok(Some(7))
        );
    }

    /// A wrong type must survive as `Wrong` rather than collapsing into "no
    /// value" — that collapse is the entire bug this type exists to prevent.
    #[test]
    fn a_wrong_type_is_an_error_and_not_silence() {
        assert_eq!(
            Given::<i64>::checked_or_none(Some(Given::Wrong("string".to_owned())), "generation"),
            Err("$generation must be of type ?int, string given".to_owned())
        );
        assert_eq!(
            Given::<i64>::Empty.checked_required("key"),
            Err("$key must be of type int, null given".to_owned())
        );
        assert_eq!(Given::<i64>::Value(1).checked_required("key"), Ok(1));
    }

    /// The message has to read like one the engine wrote, or a caller will
    /// wonder which layer is complaining.
    #[test]
    fn a_type_complaint_reads_like_phps_own() {
        assert_eq!(
            mismatch("policy", "Aerospike\\WritePolicy", "Aerospike\\ReadPolicy", Nullable::Yes),
            "$policy must be of type ?Aerospike\\WritePolicy, Aerospike\\ReadPolicy given"
        );
        assert_eq!(
            mismatch("key", "Aerospike\\Key", "string", Nullable::No),
            "$key must be of type Aerospike\\Key, string given"
        );
    }
}
