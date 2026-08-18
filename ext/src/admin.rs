// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

// `waitTillComplete`'s parameters are camelCase because PHP named arguments use
// the parameter name exactly as written; see `crate::policy` for why the lint
// cannot be scoped more tightly than the module.
#![allow(non_snake_case)]

//! The results of the cluster-management commands: [`Task`], [`UdfModule`] and
//! [`Node`].
//!
//! # Waiting happens here, not in the daemon
//!
//! Registering a UDF or creating an index returns once one node has accepted it;
//! the work then propagates, and an index on a large set can take minutes. A
//! [`Task`] is what reports on that, and **it polls from this process**.
//!
//! That is deliberate. A blocking wait served by the daemon would be a request
//! held open for minutes by a worker whose reply deadline is measured in seconds —
//! so the deadline would have to be plumbed through the transport, and a worker
//! that gave up would leave the daemon working on nothing. Polling puts the
//! timeout where the caller sets it, and every individual request stays short.
//!
//! # A task is a description, so it outlives everything
//!
//! A [`Task`] holds the identifiers the server needs to be asked — a namespace and
//! an index name, a package name, a task id — and nothing else. Nothing is
//! registered in the daemon, so a task survives the daemon restarting and the
//! worker that made it exiting, and `serialize()`-ing one to use from a later
//! request works.

use aerospike_php_ipc::admin::{
    WireNode, WireTaskHandle, WireTaskStatus, WireTaskStatusBody, WireUdfModule,
};
use aerospike_php_ipc::{decode_body, encode_body, opcode};
use ext_php_rs::prelude::*;

use crate::arg::Given;
use crate::enums::{Case, TaskStatus};
use crate::error::{AeroError, AeroResult};
use crate::settings::Settings;
use crate::transport;

/// Poll interval a wait uses when the caller names none.
///
/// A second, which is what `aerospike-core`'s own `wait_till_complete` uses: the
/// commands this waits on take seconds at least, so polling faster only adds info
/// traffic.
const DEFAULT_POLL_MS: i64 = 1_000;

/// A long-running server command, and how far it has got.
///
/// Returned by `registerUdf`, `removeUdf`, `createIndexOnBin`,
/// `createIndexUsingExpression`, `dropIndex` and `queryExecuteUdf` — each of which
/// returns as soon as the server has *accepted* the work, not when it is done.
///
/// ```php
/// $task = $client->createIndexOnBin(
///     null, 'test', 'users', 'age', 'age_idx', Aerospike\IndexType::Numeric
/// );
/// $task->waitTillComplete(30_000);     // up to 30 seconds
/// // …the index is now built on every node, and a query may use it.
/// ```
///
/// Or without blocking, for a command whose progress a request wants to report:
///
/// ```php
/// match ($task->status()) {
///     Aerospike\TaskStatus::Complete   => 'done',
///     Aerospike\TaskStatus::InProgress => 'building',
///     Aerospike\TaskStatus::NotFound   => 'gone',
/// };
/// ```
///
/// There is no constructor: a task describes work some command started, and one
/// built by hand would describe work nobody asked for.
///
/// # What the three statuses can actually mean
///
/// The server answers about work it is *doing*, which makes two of the answers
/// less informative than they look — worth knowing before branching on them:
///
/// - **A background job that finished and one that never existed are
///   indistinguishable**, and both report `Complete`. The server tracks running
///   jobs, so "no node is running it" is all it can say. Waiting on a task from a
///   command that succeeded is therefore safe; a handle for work that was never
///   started would report `Complete` at once.
/// - **An index that does not exist throws** rather than reporting `NotFound`:
///   the server answers with result code 201, `no index`, which is a more useful
///   thing to be told than "not found yet".
///
/// So `NotFound` is rare in practice. Treat `Complete` as "not running", which is
/// what it means, and rely on the command that produced the task having succeeded.
#[php_class]
#[php(name = "Aerospike\\Task")]
#[derive(Debug, Clone)]
pub struct Task {
    instance: String,
    settings: Settings,
    handle: WireTaskHandle,
}

#[php_impl]
impl Task {
    /// Ask the server how far the command has got. One round trip; never blocks.
    pub fn status(&self) -> PhpResult<Case<TaskStatus>> {
        Ok(Case(TaskStatus::of(self.ask()?)))
    }

    /// Whether the command has finished on every node.
    ///
    /// A convenience for the common test. It is `status() === Complete`, so it
    /// costs a round trip too — do not call it in a tight loop; use
    /// `Task::waitTillComplete()`, which paces itself.
    pub fn is_complete(&self) -> PhpResult<bool> {
        Ok(self.ask()? == WireTaskStatus::Complete)
    }

    /// Block until the command finishes, or until `$timeoutMs` elapses.
    ///
    /// Polls every `$pollMs` (a second by default). `$timeoutMs` of `null` waits
    /// as long as it takes, which is only reasonable in a script — a web request
    /// should pass a bound, or poll `status()` across requests instead.
    ///
    /// **Sleeps before the first check.** A command the server has only just
    /// accepted can report `NotFound` for a moment, and treating that first answer
    /// as final would fail a command that was about to start. That is what
    /// `aerospike-core`'s own wait does, for the same reason.
    ///
    /// Throws when the timeout passes, and when the server reports `NotFound`
    /// after the command should have been running — which means the work is not
    /// there to wait for.
    #[php(defaults(timeoutMs = None, pollMs = None))]
    pub fn wait_till_complete(
        &self,
        timeoutMs: Option<Given<i64>>,
        pollMs: Option<Given<i64>>,
    ) -> PhpResult<Case<TaskStatus>> {
        let timeout = Given::or_none(timeoutMs, "timeoutMs")?;
        let poll = Given::or_none(pollMs, "pollMs")?.unwrap_or(DEFAULT_POLL_MS);
        Ok(Case(TaskStatus::of(self.wait(timeout, poll)?)))
    }

    /// What this task is, for a log or a test failure.
    pub fn __to_string(&self) -> String {
        self.describe()
    }

    /// The same, as a method.
    pub fn describe(&self) -> String {
        match &self.handle {
            WireTaskHandle::Index {
                namespace,
                index_name,
                dropping,
            } => format!(
                "{} index {namespace}.{index_name}",
                if *dropping { "dropping" } else { "creating" }
            ),
            WireTaskHandle::UdfRegister { package } => format!("registering UDF {package}"),
            WireTaskHandle::UdfRemove { package } => format!("removing UDF {package}"),
            WireTaskHandle::Execute { task_id, scan } => format!(
                "background {} {task_id}",
                if *scan { "scan" } else { "query" }
            ),
        }
    }
}

impl Task {
    /// Build one for the work a command just started.
    #[must_use]
    pub fn new(instance: String, settings: Settings, handle: WireTaskHandle) -> Task {
        Task {
            instance,
            settings,
            handle,
        }
    }

    /// One status round trip.
    fn ask(&self) -> AeroResult<WireTaskStatus> {
        let body = WireTaskStatusBody {
            instance: self.instance.clone(),
            handle: self.handle.clone(),
        };
        let payload = encode_body(&body).map_err(AeroError::codec)?;
        let (header, reply) = transport::call(
            &self.instance,
            &self.settings,
            opcode::TASK_STATUS,
            "task status",
            &payload,
        )?;
        if !header.status().is_ok() {
            return Err(AeroError::from_reply(&header, &reply, "task status"));
        }
        decode_body(&reply).map_err(AeroError::codec)
    }

    /// The polling loop, split out so its rules are testable without PHP.
    ///
    /// # Errors
    /// [`AeroError`] when the deadline passes, or when the server reports the work
    /// as absent.
    fn wait(&self, timeout_ms: Option<i64>, poll_ms: i64) -> AeroResult<WireTaskStatus> {
        let poll = std::time::Duration::from_millis(u64::try_from(poll_ms.max(1)).unwrap_or(1));
        let deadline = timeout_ms
            .map(|ms| {
                std::time::Instant::now()
                    + std::time::Duration::from_millis(u64::try_from(ms.max(0)).unwrap_or(0))
            });
        let started = std::time::Instant::now();

        loop {
            // Sleep first: a command the server has only just accepted may not
            // be visible yet, and the first answer is the least reliable one.
            std::thread::sleep(poll);

            match self.ask()? {
                WireTaskStatus::Complete => return Ok(WireTaskStatus::Complete),
                WireTaskStatus::InProgress => {}
                WireTaskStatus::NotFound => {
                    return Err(AeroError::client(format!(
                        "the server has no record of {}, so there is nothing to wait for. Either \
                         it finished and was forgotten, or it never started — check \
                         status() before waiting if the difference matters",
                        self.describe()
                    )))
                }
            }

            if let Some(deadline) = deadline {
                // The next sleep would take us past the deadline, so stop now
                // rather than overshooting it by a whole poll interval.
                if std::time::Instant::now() + poll > deadline {
                    return Err(AeroError::client(format!(
                        "{} was still in progress after {}ms; it has not failed, and status() will \
                         keep reporting on it",
                        self.describe(),
                        started.elapsed().as_millis()
                    )));
                }
            }
        }
    }

    /// The work this task describes, for the tests.
    #[must_use]
    pub const fn handle(&self) -> &WireTaskHandle {
        &self.handle
    }
}

/// One UDF module the cluster holds.
///
/// Produced by `listUdf()`; there is no constructor, because a module that no
/// server reported would be a fiction.
///
/// ```php
/// foreach ($client->listUdf(null) as $module) {
///     echo $module->name(), ' ', $module->hash(), "\n";
/// }
/// ```
#[php_class]
#[php(name = "Aerospike\\UdfModule")]
#[derive(Debug, Clone)]
pub struct UdfModule {
    module: WireUdfModule,
}

#[php_impl]
impl UdfModule {
    /// The module's name on the server, e.g. `example.lua`.
    ///
    /// This is what `removeUdf()` takes, and what a UDF call names as its
    /// package — **without** the `.lua`, in the call.
    pub fn name(&self) -> String {
        self.module.name.clone()
    }

    /// The server's hash of the module's contents.
    ///
    /// How two nodes agree they hold the same module, and how a caller can tell
    /// whether the module on the server is the one it has locally.
    pub fn hash(&self) -> String {
        self.module.hash.clone()
    }

    /// The language, as the server reported it — a string rather than a
    /// `UdfLanguage`, because it is the server's answer and a language this build
    /// does not know must still be reportable.
    pub fn language(&self) -> String {
        self.module.language.clone()
    }

    /// `name (language)`, for logs.
    pub fn __to_string(&self) -> String {
        format!("{} ({})", self.module.name, self.module.language)
    }
}

impl UdfModule {
    /// Build one from the contract's.
    #[must_use]
    pub const fn from_wire(module: WireUdfModule) -> UdfModule {
        UdfModule { module }
    }
}

/// One node of a cluster, as the daemon's tend loop last saw it.
///
/// Produced by `nodes()`. The daemon's view rather than a fresh query: this is
/// where the client would route a command right now, which is the useful answer.
#[php_class]
#[php(name = "Aerospike\\Node")]
#[derive(Debug, Clone)]
pub struct Node {
    node: WireNode,
}

#[php_impl]
impl Node {
    /// The node's name, which is what `info()`'s `$node` argument takes.
    pub fn name(&self) -> String {
        self.node.name.clone()
    }

    /// The address the daemon reaches it on.
    pub fn address(&self) -> String {
        self.node.address.clone()
    }

    /// Its server version, as `major.minor.patch.build`.
    pub fn version(&self) -> String {
        self.node.version.clone()
    }

    /// Whether the daemon currently considers it usable.
    ///
    /// A node the daemon has stopped believing in is still listed, because
    /// "listed but inactive" is information a caller wants — a list that quietly
    /// omitted it would look like a smaller cluster.
    pub fn is_active(&self) -> bool {
        self.node.active
    }

    /// `name@address`, for logs.
    pub fn __to_string(&self) -> String {
        format!("{}@{}", self.node.name, self.node.address)
    }
}

impl Node {
    /// Build one from the contract's.
    #[must_use]
    pub const fn from_wire(node: WireNode) -> Node {
        Node { node }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(handle: WireTaskHandle) -> Task {
        Task::new("default".into(), Settings::default(), handle)
    }

    #[test]
    fn a_task_describes_the_work_it_names() {
        assert_eq!(
            task(WireTaskHandle::Index {
                namespace: "test".into(),
                index_name: "age_idx".into(),
                dropping: false,
            })
            .describe(),
            "creating index test.age_idx"
        );
        // A create and a drop of the same index must not read the same, because
        // "the index exists" means opposite things for the two.
        assert_eq!(
            task(WireTaskHandle::Index {
                namespace: "test".into(),
                index_name: "age_idx".into(),
                dropping: true,
            })
            .describe(),
            "dropping index test.age_idx"
        );
        assert_eq!(
            task(WireTaskHandle::UdfRegister {
                package: "example.lua".into()
            })
            .describe(),
            "registering UDF example.lua"
        );
        assert_eq!(
            task(WireTaskHandle::UdfRemove {
                package: "example.lua".into()
            })
            .describe(),
            "removing UDF example.lua"
        );
        assert_eq!(
            task(WireTaskHandle::Execute {
                task_id: 7,
                scan: true
            })
            .describe(),
            "background scan 7"
        );
        assert_eq!(
            task(WireTaskHandle::Execute {
                task_id: 7,
                scan: false
            })
            .describe(),
            "background query 7"
        );
    }

    #[test]
    fn a_task_status_maps_from_the_contract() {
        assert_eq!(
            TaskStatus::of(WireTaskStatus::NotFound),
            TaskStatus::NotFound
        );
        assert_eq!(
            TaskStatus::of(WireTaskStatus::InProgress),
            TaskStatus::InProgress
        );
        assert_eq!(
            TaskStatus::of(WireTaskStatus::Complete),
            TaskStatus::Complete
        );
    }

    #[test]
    fn a_udf_module_reports_what_the_server_said() {
        let module = UdfModule::from_wire(WireUdfModule {
            name: "example.lua".into(),
            hash: "6c9c1e2b".into(),
            language: "LUA".into(),
        });
        assert_eq!(module.name(), "example.lua");
        assert_eq!(module.hash(), "6c9c1e2b");
        // The server's word for the language, not this build's: a language it
        // does not know must still come through.
        assert_eq!(module.language(), "LUA");
        assert_eq!(module.__to_string(), "example.lua (LUA)");
    }

    #[test]
    fn a_node_is_listed_even_when_it_is_not_active() {
        let node = Node::from_wire(WireNode {
            name: "BB9".into(),
            address: "127.0.0.1:3000".into(),
            version: "8.1.3.0".into(),
            active: false,
        });
        assert_eq!(node.name(), "BB9");
        assert_eq!(node.address(), "127.0.0.1:3000");
        assert_eq!(node.version(), "8.1.3.0");
        // Listed but inactive is information; omitting it would look like a
        // smaller cluster.
        assert!(!node.is_active());
        assert_eq!(node.__to_string(), "BB9@127.0.0.1:3000");
    }
}
