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
//! turned into a status query, and how the server's `udf-list` text becomes a
//! typed list.
//!
//! # A task handle is a description, and nothing is kept between calls
//!
//! `aerospike-core` 3.0 seals its task types: the client creates them, and a
//! binding cannot rebuild one because `Client::cluster` and the task constructors
//! are crate-private. The questions those tasks ask are ordinary info commands,
//! so [`query_task_status`] asks every node the same commands, through the public
//! [`Client::nodes`] and [`Node::info`].
//!
//! That is what makes a handle a *description* rather than a registration: no
//! table here, nothing to expire, and a handle stays valid across a daemon
//! restart and across the worker that created it exiting. Compare
//! [`crate::query`], where the cursor genuinely *is* progress that has to be kept
//! — the difference is that a task's progress lives on the server, and a scan's
//! does not.

use std::sync::Arc;

use aerospike_core::task::Status;
use aerospike_core::{
    AdminPolicy, Client, CollectionIndexType, Error, IndexType, Node, Result as AeroResult,
    UdfLang, Value, Version,
};
use aerospike_php_ipc::admin::{
    WireIndexType, WireNode, WireTaskHandle, WireTaskStatus, WireUdfLang, WireUdfModule,
};
use aerospike_php_ipc::query::WireCollectionIndex;

/// Ask every node how far the work a handle describes has got.
///
/// The same questions `aerospike-core`'s task types ask, asked here because those
/// types can no longer be rebuilt from outside the crate. A handle stays a
/// description: the identifiers travel with the request, and the progress lives
/// on the server.
///
/// # Errors
/// No node is connected, or a node answers with a response that cannot be read.
pub async fn query_task_status(client: &Client, handle: &WireTaskHandle) -> AeroResult<Status> {
    let nodes = client.nodes();
    if nodes.is_empty() {
        return Err(Error::client_error("No connected node"));
    }
    let policy = AdminPolicy { timeout: 3_000 };
    match handle {
        WireTaskHandle::Index {
            namespace,
            index_name,
            dropping,
        } => {
            // Creating and dropping ask different info commands and read the
            // answer oppositely, which is why the handle records which it is.
            if *dropping {
                drop_index_status(&nodes, &policy, namespace, index_name).await
            } else {
                index_status(&nodes, &policy, namespace, index_name).await
            }
        }
        WireTaskHandle::UdfRegister { package } => {
            udf_register_status(&nodes, &policy, package).await
        }
        WireTaskHandle::UdfRemove { package } => udf_remove_status(&nodes, &policy, package).await,
        WireTaskHandle::Execute { task_id, scan } => {
            execute_status(&nodes, &policy, *task_id, *scan).await
        }
    }
}

/// One info command on one node. `None` means the node did not answer that command.
async fn ask(node: &Node, policy: &AdminPolicy, command: &str) -> AeroResult<Option<String>> {
    let response = node.info(policy, &[command]).await?;
    Ok(response.get(command).cloned())
}

async fn index_status(
    nodes: &[Arc<Node>],
    policy: &AdminPolicy,
    namespace: &str,
    index_name: &str,
) -> AeroResult<Status> {
    for node in nodes {
        let command = if node.version() >= &Version::new(8, 1, 0, 0) {
            format!("sindex-stat:namespace={namespace};indexname={index_name}")
        } else {
            format!("sindex/{namespace}/{index_name}")
        };
        let Some(response) = ask(node, policy, &command).await? else {
            return Err(Error::client_error("sindex-stat failed: missing response"));
        };
        match parse_index_load(&response)? {
            Status::Complete => {}
            other => return Ok(other),
        }
    }
    Ok(Status::Complete)
}

fn parse_index_load(response: &str) -> AeroResult<Status> {
    if response.is_empty() {
        return Err(Error::client_error("sindex-stat failed: empty response"));
    }
    let find = "load_pct=";
    match response.find(find) {
        None => {
            if response.contains("FAIL") || response.contains("ERROR") {
                Err(Error::client_error(format!(
                    "sindex-stat failed: {response}"
                )))
            } else {
                Ok(Status::InProgress)
            }
        }
        Some(index) => {
            let pct_str: String = response[index + find.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if pct_str.is_empty() {
                return Err(Error::client_error(format!(
                    "sindex-stat failed: could not parse load_pct from response '{response}'"
                )));
            }
            match pct_str.parse::<usize>() {
                Ok(pct) if pct >= 100 => Ok(Status::Complete),
                Ok(_) => Ok(Status::InProgress),
                Err(_) => Err(Error::client_error(format!(
                    "sindex-stat failed: invalid load_pct value '{pct_str}'"
                ))),
            }
        }
    }
}

async fn drop_index_status(
    nodes: &[Arc<Node>],
    policy: &AdminPolicy,
    namespace: &str,
    index_name: &str,
) -> AeroResult<Status> {
    for node in nodes {
        let command = if node.version() >= &Version::new(8, 1, 0, 0) {
            format!("sindex-exists:namespace={namespace};indexname={index_name}")
        } else {
            format!("sindex-exists:ns={namespace};indexname={index_name}")
        };
        let Some(response) = ask(node, policy, &command).await? else {
            return Ok(Status::NotFound);
        };
        // "false" is the index being gone, which is completion for a drop.
        if response.to_lowercase() != "false" {
            return Ok(Status::InProgress);
        }
    }
    Ok(Status::Complete)
}

async fn udf_register_status(
    nodes: &[Arc<Node>],
    policy: &AdminPolicy,
    package: &str,
) -> AeroResult<Status> {
    let needle = format!("filename={package}");
    for node in nodes {
        let Some(response) = ask(node, policy, "udf-list").await? else {
            return Ok(Status::NotFound);
        };
        if !response.contains(&needle) {
            return Ok(Status::InProgress);
        }
    }
    Ok(Status::Complete)
}

async fn udf_remove_status(
    nodes: &[Arc<Node>],
    policy: &AdminPolicy,
    package: &str,
) -> AeroResult<Status> {
    let needle = format!("filename={package}");
    for node in nodes {
        let Some(response) = ask(node, policy, "udf-list").await? else {
            return Ok(Status::NotFound);
        };
        if response.contains(&needle) {
            return Ok(Status::InProgress);
        }
    }
    Ok(Status::Complete)
}

async fn execute_status(
    nodes: &[Arc<Node>],
    policy: &AdminPolicy,
    task_id: u64,
    scan: bool,
) -> AeroResult<Status> {
    for node in nodes {
        let command = execute_command(node.version(), scan, task_id);
        let Some(response) = ask(node, policy, &command).await? else {
            return Ok(Status::InProgress);
        };
        match parse_execute_status(&response)? {
            Status::Complete => {}
            other => return Ok(other),
        }
    }
    Ok(Status::Complete)
}

fn execute_command(version: &Version, scan: bool, task_id: u64) -> String {
    let id_key = if version >= &Version::new(8, 1, 0, 0) {
        "id"
    } else {
        "trid"
    };
    if version.supports_partition_query() {
        format!("query-show:{id_key}={task_id}")
    } else if version.supports_query_show() {
        let module = if scan { "scan" } else { "query" };
        format!("{module}-show:{id_key}={task_id}")
    } else {
        let module = if scan { "scan" } else { "query" };
        format!("jobs:module={module};cmd=get-job;{id_key}={task_id}")
    }
}

fn parse_execute_status(response: &str) -> AeroResult<Status> {
    if response.contains("ERROR:2") {
        return Ok(Status::Complete);
    }
    if response.starts_with("ERROR") {
        return Err(Error::client_error(format!(
            "Query execute failed: {response}"
        )));
    }
    if let Some(status_idx) = response.find("status=") {
        let status_str = &response[status_idx + "status=".len()..];
        let status_end = status_str.find(':').unwrap_or(status_str.len());
        if status_str[..status_end].to_lowercase().starts_with("done") {
            return Ok(Status::Complete);
        }
        return Ok(Status::InProgress);
    }
    Ok(Status::InProgress)
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
pub const fn udf_lang(language: WireUdfLang) -> UdfLang {
    match language {
        WireUdfLang::Lua => UdfLang::Lua,
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
