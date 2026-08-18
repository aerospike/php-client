// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Cluster management: UDF modules, secondary indexes, truncation, info and
//! nodes.
//!
//! The contract's side is [`aerospike_php_ipc::admin`]. What lives here is the
//! translation, plus two things worth their own explanation: how a task handle is
//! turned back into a client task, and how the server's `udf-list` text becomes a
//! typed list.
//!
//! # A task handle rebuilds its task, and nothing is kept between calls
//!
//! `aerospike-core`'s task types — `IndexTask`, `RegisterTask`, `UdfRemoveTask`,
//! `DropIndexTask`, `ExecuteTask` — hold a cluster handle and the identifiers
//! needed to ask the server about the work. They keep no server-side state, so
//! [`task_of`] reconstructs one from a [`WireTaskHandle`] on every status request.
//!
//! That is what makes a handle a *description* rather than a registration: no
//! table here, nothing to expire, and a handle stays valid across a daemon
//! restart and across the worker that created it exiting. Compare
//! [`crate::query`], where the cursor genuinely *is* progress that has to be kept
//! — the difference is that a task's progress lives on the server, and a scan's
//! does not.

use std::sync::Arc;

use aerospike_core::task::{
    DropIndexTask, ExecuteTask, IndexTask, RegisterTask, Status, Task, UdfRemoveTask,
};
use aerospike_core::{
    AdminPolicy, Client, CollectionIndexType, IndexType, Node, UDFLang, Value,
};
use aerospike_php_ipc::admin::{
    WireIndexType, WireNode, WireTaskHandle, WireTaskStatus, WireUdfLang, WireUdfModule,
};
use aerospike_php_ipc::query::WireCollectionIndex;

/// Rebuild the client task a handle describes.
///
/// A `Box<dyn Task>` rather than a match at every call site: the four kinds
/// differ only in what they ask the server, and the caller only ever wants
/// `query_status`.
///
/// The cluster comes from `client.cluster`, which is exactly what the client's own
/// task-returning methods pass — so a rebuilt task is indistinguishable from the
/// one the original command returned.
#[must_use]
pub fn task_of(client: &Client, handle: &WireTaskHandle) -> Box<dyn Task + Send + Sync> {
    let cluster = client.cluster.clone();
    match handle {
        WireTaskHandle::Index {
            namespace,
            index_name,
            dropping,
        } => {
            // Creating and dropping ask the server the same question and read the
            // answer oppositely, which is why the handle records which it is
            // rather than leaving the caller to interpret "the index exists".
            if *dropping {
                Box::new(DropIndexTask::new(
                    cluster,
                    namespace.clone(),
                    index_name.clone(),
                ))
            } else {
                Box::new(IndexTask::new(
                    cluster,
                    namespace.clone(),
                    index_name.clone(),
                ))
            }
        }
        WireTaskHandle::UdfRegister { package } => {
            Box::new(RegisterTask::new(cluster, package.clone()))
        }
        WireTaskHandle::UdfRemove { package } => {
            Box::new(UdfRemoveTask::new(cluster, package.clone()))
        }
        WireTaskHandle::Execute { task_id, scan } => {
            Box::new(ExecuteTask::new(cluster, *task_id, *scan))
        }
    }
}

/// The contract's task status.
#[must_use]
pub const fn to_status(status: Status) -> WireTaskStatus {
    match status {
        Status::NotFound => WireTaskStatus::NotFound,
        Status::InProgress => WireTaskStatus::InProgress,
        Status::Complete => WireTaskStatus::Complete,
    }
}

/// The client's index type.
#[must_use]
pub const fn index_type(index_type: WireIndexType) -> IndexType {
    match index_type {
        WireIndexType::Numeric => IndexType::Numeric,
        WireIndexType::String => IndexType::String,
        WireIndexType::Geo2DSphere => IndexType::Geo2DSphere,
        WireIndexType::Blob => IndexType::Blob,
    }
}

/// The client's collection index type.
///
/// The same mapping [`crate::query`] applies to a filter, because an index and a
/// filter over it have to agree about which part of a collection they mean — so
/// there is one function and both call it.
#[must_use]
pub const fn collection_index(collection: WireCollectionIndex) -> CollectionIndexType {
    match collection {
        WireCollectionIndex::Default => CollectionIndexType::Default,
        WireCollectionIndex::List => CollectionIndexType::List,
        WireCollectionIndex::MapKeys => CollectionIndexType::MapKeys,
        WireCollectionIndex::MapValues => CollectionIndexType::MapValues,
    }
}

/// The client's UDF language.
#[must_use]
pub const fn udf_lang(language: WireUdfLang) -> UDFLang {
    match language {
        WireUdfLang::Lua => UDFLang::Lua,
    }
}

/// An admin policy with `timeout_ms`, or the daemon's default.
///
/// `AdminPolicy` carries only a socket timeout, so this is the whole of policy
/// resolution for the info-based commands — which is why they take a bare
/// `Option<u32>` on the wire rather than a `WirePolicy` whose other fifteen
/// fields would be silently ignored.
#[must_use]
pub fn admin_policy(timeout_ms: Option<u32>, default: std::time::Duration) -> AdminPolicy {
    AdminPolicy {
        timeout: timeout_ms.unwrap_or_else(|| u32::try_from(default.as_millis()).unwrap_or(3_000)),
    }
}

/// One node as the contract carries it.
#[must_use]
pub fn to_node(node: &Arc<Node>) -> WireNode {
    let version = node.version();
    WireNode {
        name: node.name().to_owned(),
        address: node.address().to_owned(),
        version: format!(
            "{}.{}.{}.{}",
            version.major, version.minor, version.patch, version.build
        ),
        active: node.is_active(),
    }
}

/// Positional UDF arguments as the client's values.
///
/// An empty list becomes `None` rather than an empty slice, because the client
/// distinguishes them: `Some(&[])` packs an empty argument list where `None`
/// packs no argument field at all, and the server is not obliged to treat those
/// the same.
///
/// # Errors
/// [`crate::convert::ResultOnlyValue`] for a value only the server produces.
pub fn to_udf_args(
    args: &[aerospike_php_ipc::WireValue],
) -> Result<Option<Vec<Value>>, crate::convert::ResultOnlyValue> {
    if args.is_empty() {
        return Ok(None);
    }
    let values: Result<Vec<Value>, _> = args.iter().map(crate::convert::to_value).collect();
    Ok(Some(values?))
}

/// The modules in a server's `udf-list` response.
///
/// The server answers with semicolon-separated records of comma-separated
/// `key=value` fields:
///
/// ```text
/// filename=example.lua,hash=6c9c1e2b...,type=LUA;filename=other.lua,hash=...,type=LUA;
/// ```
///
/// Parsed rather than passed through as text because a caller wanting a list of
/// modules should not have to know that format — and because the *client* does
/// not expose a `list_udf`, so this is the only place it can be done once.
///
/// Unknown fields are ignored and a record with no `filename` is skipped: this is
/// a newer server's output being read by an older client, which should degrade to
/// a shorter list rather than to an error.
#[must_use]
pub fn parse_udf_list(response: &str) -> Vec<WireUdfModule> {
    response
        .split(';')
        .filter_map(|record| {
            let mut name = None;
            let mut hash = String::new();
            let mut language = String::new();
            for field in record.split(',') {
                let Some((key, value)) = field.split_once('=') else {
                    continue;
                };
                match key.trim() {
                    "filename" => name = Some(value.trim().to_owned()),
                    "hash" => hash = value.trim().to_owned(),
                    "type" => language = value.trim().to_owned(),
                    _ => {}
                }
            }
            name.filter(|name| !name.is_empty())
                .map(|name| WireUdfModule {
                    name,
                    hash,
                    language,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::WireValue;

    #[test]
    fn a_udf_list_is_parsed_into_modules() {
        let modules = parse_udf_list(
            "filename=example.lua,hash=6c9c1e2b,type=LUA;filename=other.lua,hash=aabb,type=LUA;",
        );
        assert_eq!(
            modules,
            vec![
                WireUdfModule {
                    name: "example.lua".into(),
                    hash: "6c9c1e2b".into(),
                    language: "LUA".into(),
                },
                WireUdfModule {
                    name: "other.lua".into(),
                    hash: "aabb".into(),
                    language: "LUA".into(),
                },
            ]
        );
    }

    #[test]
    fn an_empty_or_odd_udf_list_is_a_short_list_not_an_error() {
        assert!(parse_udf_list("").is_empty());
        assert!(parse_udf_list(";;").is_empty());
        // A record with no filename names no module, so there is nothing to
        // report — better than a module called "".
        assert!(parse_udf_list("hash=aabb,type=LUA").is_empty());
        assert!(parse_udf_list("filename=,hash=aabb").is_empty());

        // A newer server's extra fields must not break the ones this build
        // knows: an older client reading a longer record should still see the
        // module.
        assert_eq!(
            parse_udf_list("filename=x.lua,hash=aa,type=LUA,something=new"),
            vec![WireUdfModule {
                name: "x.lua".into(),
                hash: "aa".into(),
                language: "LUA".into(),
            }]
        );

        // A module whose hash or type the server omitted is still a module.
        assert_eq!(
            parse_udf_list("filename=x.lua"),
            vec![WireUdfModule {
                name: "x.lua".into(),
                hash: String::new(),
                language: String::new(),
            }]
        );
    }

    #[test]
    fn every_index_type_maps_to_the_clients_own() {
        // Named individually rather than by a loop, so that a variant added
        // upstream is a non-exhaustive-match compile error here.
        assert!(matches!(
            index_type(WireIndexType::Numeric),
            IndexType::Numeric
        ));
        assert!(matches!(
            index_type(WireIndexType::String),
            IndexType::String
        ));
        assert!(matches!(
            index_type(WireIndexType::Geo2DSphere),
            IndexType::Geo2DSphere
        ));
        assert!(matches!(index_type(WireIndexType::Blob), IndexType::Blob));
    }

    #[test]
    fn every_collection_index_maps_to_the_clients_own() {
        assert!(matches!(
            collection_index(WireCollectionIndex::Default),
            CollectionIndexType::Default
        ));
        assert!(matches!(
            collection_index(WireCollectionIndex::List),
            CollectionIndexType::List
        ));
        assert!(matches!(
            collection_index(WireCollectionIndex::MapKeys),
            CollectionIndexType::MapKeys
        ));
        assert!(matches!(
            collection_index(WireCollectionIndex::MapValues),
            CollectionIndexType::MapValues
        ));
    }

    #[test]
    fn every_task_status_maps_to_the_contracts_own() {
        assert_eq!(to_status(Status::NotFound), WireTaskStatus::NotFound);
        assert_eq!(to_status(Status::InProgress), WireTaskStatus::InProgress);
        assert_eq!(to_status(Status::Complete), WireTaskStatus::Complete);
    }

    #[test]
    fn an_admin_policy_takes_the_requests_timeout_or_the_daemons() {
        assert_eq!(
            admin_policy(Some(500), std::time::Duration::from_secs(9)).timeout,
            500
        );
        assert_eq!(
            admin_policy(None, std::time::Duration::from_secs(9)).timeout,
            9_000
        );
        // A daemon default too large for the field is clamped to something
        // usable rather than wrapping to a tiny timeout.
        assert_eq!(
            admin_policy(None, std::time::Duration::from_secs(u64::MAX / 1_000)).timeout,
            3_000
        );
    }

    /// `Some(&[])` and `None` are not the same request to the server, so an empty
    /// PHP argument list must become `None`.
    #[test]
    fn no_udf_arguments_is_none_and_not_an_empty_list() {
        assert_eq!(to_udf_args(&[]).unwrap(), None);
        assert_eq!(
            to_udf_args(&[WireValue::Int(1)]).unwrap(),
            Some(vec![Value::Int(1)])
        );

        // A result-only value cannot be an argument, and says so.
        assert!(to_udf_args(&[WireValue::MultiResult(vec![])]).is_err());
    }
}
