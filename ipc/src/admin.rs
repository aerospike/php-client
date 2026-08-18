// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The commands that manage a cluster rather than its records: user-defined
//! functions, secondary indexes, truncation, info and node listing.
//!
//! # Long-running commands answer with a handle, not a result
//!
//! Registering a UDF or creating an index returns as soon as *one* node has
//! accepted it. The work then propagates, and how long that takes is the
//! server's business: an index on a large set can take minutes. So these answer
//! with a [`WireTaskHandle`], and [`TASK_STATUS`](crate::opcode::TASK_STATUS)
//! reports where it has got to.
//!
//! **The handle is a description, not a registration.** It holds the identifiers
//! the server needs to be asked about — a namespace and an index name, a package
//! name, a task id — so the daemon rebuilds the client's task object from it on
//! every status call and keeps nothing between them. Three things follow, and all
//! three matter for PHP:
//!
//! - a handle survives the daemon restarting, and survives the worker that made
//!   it exiting;
//! - it can be stored, sent to a browser, and used from a later request;
//! - waiting is the *caller's* loop, not a request the daemon blocks in. Nothing
//!   here can outlive a worker's reply deadline, because nothing here waits.
//!
//! That last point is why there is no `TASK_WAIT` opcode. A blocking wait would
//! be a request open for minutes, held by a worker that has a timeout measured
//! in seconds; polling from PHP puts the deadline where the caller can set it.

use serde::{Deserialize, Serialize};

use crate::op::{WireCtx, WireExpression};
use crate::query::{WireCollectionIndex, WireStatement};
use crate::{Target, WirePolicy, WireValue};

/// What a long-running server command is, in enough detail to ask after it.
///
/// Deliberately the *identifiers* rather than an opaque number: see the module
/// docs. Each variant maps to one of `aerospike-core`'s task types, which the
/// daemon reconstructs from these fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireTaskHandle {
    /// A secondary index being built, or dropped.
    ///
    /// One variant for both because the server is asked the same question —
    /// whether the index exists and is built — and the two differ only in which
    /// answer means "done". [`dropping`](Self::Index::dropping) says which.
    Index {
        /// Namespace the index is in.
        namespace: String,
        /// Index name.
        index_name: String,
        /// Whether this is a drop, for which the index *disappearing* is
        /// completion.
        dropping: bool,
    },
    /// A UDF module being registered on every node.
    UdfRegister {
        /// The module's name on the server, e.g. `example.lua`.
        package: String,
    },
    /// A UDF module being removed from every node.
    UdfRemove {
        /// The module's name on the server.
        package: String,
    },
    /// A background query applying a UDF to every matching record.
    Execute {
        /// The server's task id.
        task_id: u64,
        /// Whether the query was a scan (no filter), which the server tracks
        /// under a different info command.
        scan: bool,
    },
}

/// Payload of a [`TASK_STATUS`](crate::opcode::TASK_STATUS) request.
///
/// The instance travels beside the handle rather than inside it: a task belongs
/// to the cluster the command was issued against, and a daemon serving several
/// clusters could otherwise be asked about an index name that exists in two of
/// them. Keeping it out of [`WireTaskHandle`] also keeps the handle itself
/// portable — the same description is the same work whichever client asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTaskStatusBody {
    /// Cluster instance the command was issued against.
    pub instance: String,
    /// What to ask about.
    pub handle: WireTaskHandle,
}

/// How far a long-running command has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireTaskStatus {
    /// The server has no record of it.
    ///
    /// For a freshly issued command this can mean "not started yet" as much as
    /// "never existed", which is why a caller polls rather than treating the
    /// first `NotFound` as a failure.
    NotFound,
    /// Still running.
    InProgress,
    /// Finished on every node.
    Complete,
}

/// The language a UDF module is written in.
///
/// One case, as in the server and in `aerospike-core`. It is an enum rather than
/// an implied constant so that a second language is an additive change here
/// rather than a new parameter everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WireUdfLang {
    /// Lua.
    #[default]
    Lua,
}

/// What kind of value a secondary index covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireIndexType {
    /// Integers.
    Numeric,
    /// Strings.
    String,
    /// Geospatial points and regions, on a sphere.
    Geo2DSphere,
    /// Byte strings. Needs server 7.0 or later.
    Blob,
}

/// What a secondary index is built over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireIndexOn {
    /// A bin, optionally reached through a path into a nested collection.
    Bin {
        /// Bin name.
        name: String,
        /// Path into a nested collection, for an index on something inside one.
        ctx: Vec<WireCtx>,
    },
    /// An expression evaluated over the record.
    ///
    /// The index covers whatever the expression produces, so a query using it
    /// must name it by index name — there is no bin to look it up by.
    Expression(WireExpression),
}

/// Payload of an [`INDEX_CREATE`](crate::opcode::INDEX_CREATE) request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireIndexCreateBody {
    /// Cluster instance.
    pub instance: String,
    /// Socket timeout for the info command, in milliseconds. `None` uses the
    /// daemon's default.
    pub timeout_ms: Option<u32>,
    /// Namespace to index.
    pub namespace: String,
    /// Set to index; empty means the whole namespace.
    pub set: String,
    /// Name for the new index. Unique within the namespace.
    pub index_name: String,
    /// What to index.
    pub on: WireIndexOn,
    /// The type of the indexed value.
    pub index_type: WireIndexType,
    /// Which part of a collection to index.
    pub collection: WireCollectionIndex,
}

/// Payload of an [`INDEX_DROP`](crate::opcode::INDEX_DROP) request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireIndexDropBody {
    /// Cluster instance.
    pub instance: String,
    /// Info-command timeout.
    pub timeout_ms: Option<u32>,
    /// Namespace the index is in.
    pub namespace: String,
    /// Set the index was created on.
    pub set: String,
    /// Index to drop.
    pub index_name: String,
}

/// Payload of a [`UDF_REGISTER`](crate::opcode::UDF_REGISTER) request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireUdfRegisterBody {
    /// Cluster instance.
    pub instance: String,
    /// Info-command timeout.
    pub timeout_ms: Option<u32>,
    /// The module's name on the server, e.g. `example.lua`.
    pub server_path: String,
    /// The module's source.
    ///
    /// Bytes rather than a `String`, and sent from the caller rather than read
    /// from a path by the daemon: the daemon may not share a filesystem with the
    /// worker, and a path that resolved differently on the two sides would
    /// register the wrong file.
    pub source: Vec<u8>,
    /// The language it is written in.
    pub language: WireUdfLang,
}

/// Payload of a [`UDF_REMOVE`](crate::opcode::UDF_REMOVE) request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireUdfRemoveBody {
    /// Cluster instance.
    pub instance: String,
    /// Info-command timeout.
    pub timeout_ms: Option<u32>,
    /// The module to remove.
    pub server_path: String,
}

/// Payload of a [`UDF_LIST`](crate::opcode::UDF_LIST) request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireUdfListBody {
    /// Cluster instance.
    pub instance: String,
    /// Info-command timeout.
    pub timeout_ms: Option<u32>,
}

/// One UDF module the server holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUdfModule {
    /// The module's name on the server.
    pub name: String,
    /// The server's hash of its contents, which is how two nodes agree they hold
    /// the same module.
    pub hash: String,
    /// The language, as the server reported it — a string rather than
    /// [`WireUdfLang`] because it is the *server's* answer, and a language this
    /// build does not know must be reportable rather than a decode failure.
    pub language: String,
}

/// Payload of a [`UDF_LIST`](crate::opcode::UDF_LIST) reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUdfList {
    /// The modules, in the order the server listed them.
    pub modules: Vec<WireUdfModule>,
}

/// Payload of a [`UDF_EXECUTE`](crate::opcode::UDF_EXECUTE) request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireUdfExecuteBody {
    /// Which record to run it against, and under which policy.
    pub target: Target,
    /// The module's name on the server.
    pub package: String,
    /// The function within it.
    pub function: String,
    /// Positional arguments.
    pub args: Vec<WireValue>,
}

/// Payload of a [`UDF_EXECUTE`](crate::opcode::UDF_EXECUTE) reply.
///
/// A named body rather than a bare `Option<WireValue>` because a UDF returning
/// nil and a UDF whose bin was absent are different things, and a wrapper leaves
/// room to say which without changing the shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireUdfResult {
    /// What the function returned, or `None` when it returned nothing.
    pub value: Option<WireValue>,
}

/// Payload of a [`QUERY_UDF`](crate::opcode::QUERY_UDF) request: a UDF applied
/// to every record a statement matches, in the background.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireQueryUdfBody {
    /// Cluster instance.
    pub instance: String,
    /// Write policy for the records the UDF changes.
    pub policy: WirePolicy,
    /// Which records to apply it to. A statement with no filter applies it to
    /// the whole set.
    pub statement: WireStatement,
    /// The module's name on the server.
    pub package: String,
    /// The function within it.
    pub function: String,
    /// Positional arguments.
    pub args: Vec<WireValue>,
}

/// Payload of a [`TRUNCATE`](crate::opcode::TRUNCATE) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTruncateBody {
    /// Cluster instance.
    pub instance: String,
    /// Info-command timeout.
    pub timeout_ms: Option<u32>,
    /// Namespace to truncate.
    pub namespace: String,
    /// Set to truncate; empty truncates the whole namespace.
    pub set: String,
    /// Delete only records last updated before this time, in nanoseconds since
    /// the Unix epoch. `None` deletes every record.
    ///
    /// Nanoseconds since the epoch, not the server's own epoch: the conversion
    /// belongs on one side of the channel, and this is the unit a caller has.
    pub before_nanos: Option<i64>,
}

/// Payload of an [`INFO`](crate::opcode::INFO) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireInfoBody {
    /// Cluster instance.
    pub instance: String,
    /// Info-command timeout.
    pub timeout_ms: Option<u32>,
    /// Which node to ask, by name. `None` asks any one node.
    ///
    /// Most info commands answer for the whole cluster whichever node is asked;
    /// the ones that do not — statistics, latencies — are exactly the ones worth
    /// naming a node for.
    pub node: Option<String>,
    /// The commands, e.g. `build`, `namespaces`, `sets/test`.
    pub commands: Vec<String>,
}

/// Payload of an [`INFO`](crate::opcode::INFO) reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireInfoReply {
    /// One entry per command, in request order, as the server answered.
    ///
    /// Pairs rather than a map, so the server's order survives — an info
    /// response is a list of answers to questions asked in a sequence.
    pub values: Vec<(String, String)>,
}

/// Payload of a [`NODES`](crate::opcode::NODES) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireNodesBody {
    /// Cluster instance.
    pub instance: String,
}

/// One node in the daemon's current view of a cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireNode {
    /// The node's name, which is what [`WireInfoBody::node`] takes.
    pub name: String,
    /// The address the daemon reaches it on.
    pub address: String,
    /// Its server version, as `major.minor.patch.build`.
    pub version: String,
    /// Whether the daemon currently considers it usable.
    pub active: bool,
}

/// Payload of a [`NODES`](crate::opcode::NODES) reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireNodes {
    /// The nodes, as the daemon's tend loop last saw them.
    ///
    /// The daemon's view, not a fresh query: this is what the client would route
    /// a command to right now, which is the useful answer.
    pub nodes: Vec<WireNode>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::WireCollectionIndex;
    use crate::{decode_body, encode_body, BinSelector, WireKey};

    #[test]
    fn every_task_handle_round_trips() {
        for handle in [
            WireTaskHandle::Index {
                namespace: "test".into(),
                index_name: "age_idx".into(),
                dropping: false,
            },
            WireTaskHandle::Index {
                namespace: "test".into(),
                index_name: "age_idx".into(),
                dropping: true,
            },
            WireTaskHandle::UdfRegister {
                package: "example.lua".into(),
            },
            WireTaskHandle::UdfRemove {
                package: "example.lua".into(),
            },
            WireTaskHandle::Execute {
                task_id: u64::MAX,
                scan: true,
            },
        ] {
            let bytes = encode_body(&handle).unwrap();
            assert_eq!(decode_body::<WireTaskHandle>(&bytes).unwrap(), handle);
        }
    }

    /// A create and a drop of the same index must not be the *same* handle: the
    /// index existing means opposite things for the two.
    #[test]
    fn creating_and_dropping_an_index_are_different_handles() {
        let creating = WireTaskHandle::Index {
            namespace: "test".into(),
            index_name: "age_idx".into(),
            dropping: false,
        };
        let dropping = WireTaskHandle::Index {
            namespace: "test".into(),
            index_name: "age_idx".into(),
            dropping: true,
        };
        assert_ne!(creating, dropping);
    }

    #[test]
    fn a_handle_is_small_enough_to_hand_to_a_browser() {
        // The point of a handle being a description is that it can be stored and
        // used later. That is only true if it is small.
        let handle = WireTaskHandle::Index {
            namespace: "test".into(),
            index_name: "age_idx".into(),
            dropping: false,
        };
        assert!(encode_body(&handle).unwrap().len() < 64);
    }

    #[test]
    fn an_index_request_round_trips_over_a_bin_and_over_an_expression() {
        let base = WireIndexCreateBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            timeout_ms: Some(3_000),
            namespace: "test".into(),
            set: "users".into(),
            index_name: "age_idx".into(),
            on: WireIndexOn::Bin {
                name: "age".into(),
                ctx: vec![WireCtx::MapKey {
                    key: WireValue::Str("inner".into()),
                }],
            },
            index_type: WireIndexType::Numeric,
            collection: WireCollectionIndex::MapValues,
        };
        let bytes = encode_body(&base).unwrap();
        assert_eq!(decode_body::<WireIndexCreateBody>(&bytes).unwrap(), base);

        let expression = WireIndexCreateBody {
            on: WireIndexOn::Expression(WireExpression::Ael("$.a + $.b".into())),
            index_type: WireIndexType::String,
            collection: WireCollectionIndex::Default,
            ..base
        };
        let bytes = encode_body(&expression).unwrap();
        assert_eq!(
            decode_body::<WireIndexCreateBody>(&bytes).unwrap(),
            expression
        );
    }

    #[test]
    fn every_index_type_round_trips() {
        for index_type in [
            WireIndexType::Numeric,
            WireIndexType::String,
            WireIndexType::Geo2DSphere,
            WireIndexType::Blob,
        ] {
            let bytes = encode_body(&index_type).unwrap();
            assert_eq!(decode_body::<WireIndexType>(&bytes).unwrap(), index_type);
        }
    }

    #[test]
    fn a_udf_registration_carries_its_source_as_bytes() {
        let body = WireUdfRegisterBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            timeout_ms: None,
            server_path: "example.lua".into(),
            // Not valid UTF-8: the source travels as bytes, so a module with an
            // odd byte in a comment still registers unchanged.
            source: vec![b'-', b'-', 0xFF, b'\n'],
            language: WireUdfLang::Lua,
        };
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireUdfRegisterBody>(&bytes).unwrap(), body);
    }

    #[test]
    fn a_udf_call_and_its_result_round_trip() {
        let body = WireUdfExecuteBody {
            target: Target {
                instance: crate::DEFAULT_INSTANCE.into(),
                namespace: "test".into(),
                set: "users".into(),
                key: WireKey::Str("alice".into()),
                policy: WirePolicy::default(),
            },
            package: "example".into(),
            function: "touch".into(),
            args: vec![WireValue::Int(1), WireValue::Str("x".into())],
        };
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireUdfExecuteBody>(&bytes).unwrap(), body);

        // A function that returned nothing and one that returned nil are
        // different answers, and both have to survive.
        for value in [None, Some(WireValue::Nil), Some(WireValue::Int(7))] {
            let result = WireUdfResult { value };
            let bytes = encode_body(&result).unwrap();
            assert_eq!(decode_body::<WireUdfResult>(&bytes).unwrap(), result);
        }
    }

    #[test]
    fn a_background_query_udf_carries_its_statement() {
        let body = WireQueryUdfBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            policy: WirePolicy::default(),
            statement: WireStatement {
                namespace: "test".into(),
                set: "users".into(),
                bins: BinSelector::All,
                filter: None,
            },
            package: "example".into(),
            function: "bump".into(),
            args: Vec::new(),
        };
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireQueryUdfBody>(&bytes).unwrap(), body);
    }

    #[test]
    fn a_truncate_distinguishes_the_whole_namespace_from_one_set() {
        let namespace = WireTruncateBody {
            instance: crate::DEFAULT_INSTANCE.into(),
            timeout_ms: None,
            namespace: "test".into(),
            set: String::new(),
            before_nanos: None,
        };
        let one_set = WireTruncateBody {
            set: "users".into(),
            before_nanos: Some(1_700_000_000_000_000_000),
            ..namespace.clone()
        };
        for body in [namespace, one_set] {
            let bytes = encode_body(&body).unwrap();
            assert_eq!(decode_body::<WireTruncateBody>(&bytes).unwrap(), body);
        }
    }

    #[test]
    fn an_info_reply_keeps_the_order_the_commands_were_asked_in() {
        let reply = WireInfoReply {
            values: vec![
                ("build".into(), "8.1.3".into()),
                ("namespaces".into(), "test".into()),
            ],
        };
        let bytes = encode_body(&reply).unwrap();
        let decoded: WireInfoReply = decode_body(&bytes).unwrap();
        // Pairs, not a map: an info response answers questions in a sequence,
        // and a map would lose which came first.
        assert_eq!(decoded.values[0].0, "build");
        assert_eq!(decoded, reply);
    }

    #[test]
    fn a_node_list_round_trips() {
        let nodes = WireNodes {
            nodes: vec![WireNode {
                name: "BB9".into(),
                address: "127.0.0.1:3000".into(),
                version: "8.1.3.0".into(),
                active: true,
            }],
        };
        let bytes = encode_body(&nodes).unwrap();
        assert_eq!(decode_body::<WireNodes>(&bytes).unwrap(), nodes);
    }

    /// The instance is beside the handle, not in it — so the same handle can be
    /// asked about on a named cluster, and a daemon serving two of them cannot
    /// answer about the wrong one.
    #[test]
    fn a_status_request_names_the_cluster_the_task_belongs_to() {
        let body = WireTaskStatusBody {
            instance: "analytics".into(),
            handle: WireTaskHandle::Index {
                namespace: "test".into(),
                index_name: "age_idx".into(),
                dropping: false,
            },
        };
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireTaskStatusBody>(&bytes).unwrap(), body);

        // The handle on its own carries no instance, which is what keeps it the
        // same description whichever client holds it.
        let alone = encode_body(&body.handle).unwrap();
        assert!(!String::from_utf8_lossy(&alone).contains("analytics"));
    }

    #[test]
    fn every_task_status_round_trips() {
        for status in [
            WireTaskStatus::NotFound,
            WireTaskStatus::InProgress,
            WireTaskStatus::Complete,
        ] {
            let bytes = encode_body(&status).unwrap();
            assert_eq!(decode_body::<WireTaskStatus>(&bytes).unwrap(), status);
        }
    }
}
