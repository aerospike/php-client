// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! End-to-end tests: the daemon's real server loop, driven by a real iceoryx2
//! client over real shared memory, using the contract's own types.
//!
//! This is the proof-of-concept gate — it validates everything the PHP
//! extension will depend on without needing PHP. The requests are built as
//! [`RequestHeader`] values and every reply is converted back to a
//! [`ReplyHeader`] and `validate`d, so what is asserted here is the frozen
//! contract, not the daemon's private view of it.
//!
//! One daemon serves the whole test binary, and each test attaches its own
//! client port — which is also how PHP-FPM behaves: many workers, one service.
//!
//! Requires an Aerospike server with the `test` namespace: `AEROSPIKE_HOSTS`
//! says where it is (default `127.0.0.1:3000`), and the other `AEROSPIKE_*`
//! variables listed on [`cluster_keys`] cover what it takes to reach it —
//! alternate services, credentials. Tests that do not touch the database
//! (`PING`, unknown instance, malformed frame) pass without a server.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use iceoryx2::node::{Node, NodeBuilder};
use iceoryx2::port::client::Client as ClientPort;
use iceoryx2::prelude::{AllocationStrategy, ServiceName, SignalHandlingMode};

use aerospike_php_daemon::config::Config;
use aerospike_php_daemon::dispatch::Dispatcher;
use aerospike_php_daemon::instance::Instances;
use aerospike_php_daemon::server::{Server, Shutdown};
use aerospike_php_ipc::admin::{
    WireIndexCreateBody, WireIndexDropBody, WireIndexOn, WireIndexType, WireInfoBody,
    WireInfoReply, WireNodes, WireNodesBody, WireTaskHandle, WireTaskStatus, WireTaskStatusBody,
    WireTruncateBody, WireUdfExecuteBody, WireUdfLang, WireUdfList, WireUdfListBody,
    WireUdfRegisterBody, WireUdfRemoveBody, WireUdfResult,
};
use aerospike_php_ipc::op::WireExpression;
use aerospike_php_ipc::policy::{WireExpiration, WireGenerationPolicy};
use aerospike_php_ipc::query::{
    WireCursorBody, WirePartitions, WireQueryBody, WireQueryPage, WireQueryRecord, WireStatement,
};
use aerospike_php_ipc::security::{
    WireAdminTarget, WirePrivilege, WirePrivilegeCode, WireRoleAllowlistBody, WireRoleCreateBody,
    WireRoleNameBody, WireRoleQueryBody, WireRoles, WireUserCreateBody, WireUserNameBody,
    WireUserQueryBody, WireUserRolesBody, WireUsers,
};
use aerospike_php_ipc::txn::{
    WireAbortStatus, WireCommitStatus, WireTxnAbortBody, WireTxnBeginBody, WireTxnBody,
    WireTxnCommitBody, WireTxnHandle, WireTxnState,
};
use aerospike_php_ipc::wakeup::Waiter;
use aerospike_php_ipc::{
    decode_body, encode_body, opcode, rpc_service_name, Backoff, BinSelector, ClientId, ErrorBody,
    GetBody, ModifyBody, OperateBody, PongBody, PutBody, RecordBody, ReplyHeader, RequestHeader,
    StatusCode, Target, TargetBody, WireBitOp, WireBitPolicy, WireCtx, WireHllOp, WireHllPolicy,
    WireKey, WireListOp, WireListPolicy, WireMapOp, WireMapOrder, WireMapPolicy, WireMapReturn,
    WireMapReturnKind, WireOp, WirePolicy, WireValue, INITIAL_MAX_SLICE_LEN,
};

type Ipc = iceoryx2::service::ipc::Service;
type Port = ClientPort<Ipc, [u8], RequestHeader, [u8], ReplyHeader>;

/// Long enough that a slow local cluster does not look like a hang, short
/// enough that a hang does not look like a slow cluster.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

// ===== The cluster under test ==============================================
//
// Nothing here names a host. Every seed, credential and tending switch comes
// from the environment, because the cluster these tests run against is not the
// same one twice: a developer's container, a NAT'ed VM that only answers on its
// `services-alternate` addresses, a secured cluster in CI. A test that hard-coded
// any of it would report "the cluster is broken" for "the cluster is elsewhere".

/// An environment variable, treating empty as unset — `make` forwards its
/// cluster variables unconditionally, so the empty string means "not given".
fn env_opt(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

/// The seed host(s) of the cluster under test: `AEROSPIKE_HOSTS`.
fn hosts() -> String {
    env_opt("AEROSPIKE_HOSTS").unwrap_or_else(|| "127.0.0.1:3000".to_string())
}

/// The `[cluster.*]` keys that point at the cluster under test, one per line,
/// ready to be pasted into a generated configuration.
///
/// Environment:
///   `AEROSPIKE_HOSTS`                     seed host(s) (default 127.0.0.1:3000)
///   `AEROSPIKE_USE_SERVICES_ALTERNATE`    tend the alternate node list
///   `AEROSPIKE_USER`, `AEROSPIKE_PASSWORD`, `AEROSPIKE_AUTH_MODE`
fn cluster_keys() -> String {
    let mut keys = format!("host = \"{}\"\n", hosts());
    if let Some(value) = env_opt("AEROSPIKE_USE_SERVICES_ALTERNATE") {
        let on = matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        );
        keys.push_str(&format!("use-services-alternate = {on}\n"));
    }
    for (var, key) in [
        ("AEROSPIKE_USER", "user"),
        ("AEROSPIKE_PASSWORD", "password"),
        ("AEROSPIKE_AUTH_MODE", "auth"),
    ] {
        if let Some(value) = env_opt(var) {
            keys.push_str(&format!("{key} = \"{value}\"\n"));
        }
    }
    keys
}

// ===== The daemon under test ===============================================

struct Fixture {
    instance: String,
    namespace: String,
    set: String,
    ready_instances: usize,
    // Kept alive for the life of the process: the server thread borrows this
    // runtime's handle, and the static is never dropped.
    _runtime: tokio::runtime::Runtime,
    _shutdown: Shutdown,
}

fn daemon() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(start_daemon)
}

fn start_daemon() -> Fixture {
    let hosts = hosts();
    let cluster = cluster_keys();
    // Unique per process, so a test run never collides with a daemon that is
    // actually deployed on this host, nor with a previous run's shared memory.
    let instance = format!("phpd-test-{}", std::process::id());
    let set = format!("phpd{}", std::process::id());

    // Not indented: `cluster` is several lines, and a TOML key may not be
    // preceded by anything but whitespace *within* its own line — so the block
    // is assembled flush left rather than pasted into an indented literal.
    let config = Config::parse(&format!(
        "[daemon]\n\
         instance = \"{instance}\"\n\
         worker_threads = 2\n\
         default_timeout = \"5s\"\n\
         \n\
         [cluster.default]\n\
         timeout = \"3s\"\n\
         {cluster}\n\
         [cluster.secondary]\n\
         {cluster}"
    ))
    .expect("the test configuration must be valid");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let instances = std::sync::Arc::new(runtime.block_on(Instances::connect(&config)));
    let ready_instances = instances.ready_count();
    if ready_instances == 0 {
        eprintln!(
            "WARNING: no cluster at {hosts} — the PUT/GET tests will fail. \
             Point AEROSPIKE_HOSTS at a server with the '{}' namespace.",
            std::env::var("AEROSPIKE_NAMESPACE").unwrap_or_else(|_| "test".to_string())
        );
    }

    let dispatcher = std::sync::Arc::new(Dispatcher::new(
        instances,
        config.daemon.default_timeout.as_duration(),
    ));
    let tuning = config.daemon.tuning();
    let shutdown = Shutdown::new();

    // Bind on the serving thread (the port is not `Send`) and report readiness
    // before running, so no test races the service into existence.
    let (ready_tx, ready_rx) = mpsc::channel();
    let stop = shutdown.clone();
    let handle = runtime.handle().clone();
    let serving_instance = instance.clone();
    std::thread::spawn(move || {
        // The fixture takes the tuning from the configuration too, so a change to
        // how it is threaded through is a compile error here as well.
        let mut server = match Server::bind(&serving_instance, dispatcher, handle, tuning) {
            Ok(server) => server,
            Err(e) => {
                ready_tx.send(Err(e.to_string())).ok();
                return;
            }
        };
        ready_tx.send(Ok(())).ok();
        server
            .run_until(&stop)
            .expect("the server loop must not fail");
    });

    ready_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the server thread must report readiness")
        .expect("the server must bind");

    Fixture {
        instance,
        namespace: std::env::var("AEROSPIKE_NAMESPACE").unwrap_or_else(|_| "test".to_string()),
        set,
        ready_instances,
        _runtime: runtime,
        _shutdown: shutdown,
    }
}

// ===== A PHP worker, as far as the transport is concerned ===================

/// One attached client: its own port, its own id, its own sequence counter —
/// exactly what a forked PHP-FPM worker owns.
struct Worker {
    instance: String,
    port: Port,
    waiter: Waiter<Ipc>,
    client_id: ClientId,
    seq: u64,
    // The node owns the service handles the port and waiter were built from.
    _node: Node<Ipc>,
}

impl Worker {
    /// Attach to the in-process daemon this test binary runs.
    fn attach() -> Worker {
        Worker::attach_to(&daemon().instance)
    }

    fn attach_to(instance: &str) -> Worker {
        static NONCE: AtomicU32 = AtomicU32::new(1);

        let node = NodeBuilder::new()
            .signal_handling_mode(SignalHandlingMode::Disabled)
            .create::<Ipc>()
            .unwrap();
        let name = rpc_service_name(instance);
        let service_name: ServiceName = name.as_str().try_into().unwrap();

        let service = node
            .service_builder(&service_name)
            .request_response::<[u8], [u8]>()
            .request_user_header::<RequestHeader>()
            .response_user_header::<ReplyHeader>()
            .open_or_create()
            .expect("the daemon's service must be open");

        // Both ports must size their loans and grow by powers of two, or any
        // payload past the default single element is refused outright.
        let port = service
            .client_builder()
            .initial_max_slice_len(INITIAL_MAX_SLICE_LEN)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .unwrap();

        let client_id = ClientId::new(std::process::id(), NONCE.fetch_add(1, Ordering::Relaxed));
        // Attach before sending anything: in event mode this is what the daemon
        // notifies, and it must exist before the reply is written.
        let waiter = Waiter::attach(&node, instance, client_id, Backoff::default())
            .expect("a waiter must attach in either wakeup mode");

        Worker {
            instance: instance.to_string(),
            port,
            waiter,
            client_id,
            seq: 0,
            _node: node,
        }
    }

    /// Send one request and wait for its reply, exactly as the extension will:
    /// poll the response port, pausing with whatever the compiled wakeup
    /// strategy provides.
    fn call(&mut self, opcode: u16, body: &[u8]) -> (ReplyHeader, Vec<u8>) {
        self.seq += 1;
        let seq = self.seq;
        let header = RequestHeader::new(
            opcode,
            seq,
            self.client_id,
            u32::try_from(CALL_TIMEOUT.as_millis()).unwrap(),
            u32::try_from(body.len()).unwrap(),
        );

        let request = self.port.loan_slice_uninit(body.len()).unwrap();
        let mut request = request.write_from_slice(body);
        *request.user_header_mut() = header;
        let pending = request.send().unwrap();

        self.waiter.rearm(Backoff::default());
        let deadline = Instant::now() + CALL_TIMEOUT;
        loop {
            if let Some(response) = pending.receive().unwrap() {
                let reply: ReplyHeader = *response.user_header();
                let payload = response.payload().to_vec();
                // The contract's own validation, on the bytes that crossed
                // shared memory.
                reply
                    .validate(payload.len())
                    .expect("the reply must be a well-formed frame");
                assert_eq!(
                    reply.seq, seq,
                    "a reply must echo the sequence of its request"
                );
                return (reply, payload);
            }
            let now = Instant::now();
            assert!(
                now < deadline,
                "no reply from '{}' to opcode {opcode} (seq {seq}) within {CALL_TIMEOUT:?}",
                self.instance
            );
            self.waiter.wait_hint(deadline - now).unwrap();
        }
    }
}

/// A value with every map's pairs put in a canonical order.
///
/// The server stores a plain map bin unordered and returns its entries in its
/// own canonical key order (integers before strings, and so on), so a written
/// pair order never survives a round trip — the *content* does. Lists are left
/// alone: their order is data.
fn canonical(value: &WireValue) -> WireValue {
    match value {
        WireValue::List(items) => WireValue::List(items.iter().map(canonical).collect()),
        WireValue::Map(pairs) => {
            let mut pairs: Vec<(WireValue, WireValue)> = pairs
                .iter()
                .map(|(k, v)| (canonical(k), canonical(v)))
                .collect();
            pairs.sort_by_key(|pair| format!("{pair:?}"));
            WireValue::Map(pairs)
        }
        other => other.clone(),
    }
}

fn error_message(payload: &[u8]) -> String {
    let message = decode_body::<ErrorBody>(payload)
        .expect("a failure reply must carry an ErrorBody")
        .message;
    // A secured cluster refuses on privileges, and the server's word for that
    // ("RoleViolation") names no knob — so every assertion in the suite would
    // report a permissions problem as a broken feature. This suite reads and
    // writes records, creates and drops indexes, registers UDFs, truncates sets
    // and administers users, so its credentials need the privileges for all of
    // it: the stock `admin` user has `user-admin` alone and fails right here.
    if message.contains("RoleViolation") || message.contains("not authenticated") {
        return format!(
            "{message}\n    (AEROSPIKE_USER={} — a secured cluster needs a user \
             with read-write, sys-admin and data-admin, not just user-admin)",
            env_opt("AEROSPIKE_USER").unwrap_or_else(|| "<unset>".to_string())
        );
    }
    message
}

/// The boolean `DELETE` and `EXISTS` answer with: their whole payload is one
/// [`WireValue::Bool`].
fn flag(payload: &[u8]) -> bool {
    match decode_body::<WireValue>(payload) {
        Ok(WireValue::Bool(value)) => value,
        other => panic!("expected a boolean reply payload, got {other:?}"),
    }
}

/// A record in this run's own set, with no policy overrides.
fn target(key: WireKey) -> Target {
    target_with(key, WirePolicy::default())
}

fn target_with(key: WireKey, policy: WirePolicy) -> Target {
    let fixture = daemon();
    Target {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        namespace: fixture.namespace.clone(),
        set: fixture.set.clone(),
        key,
        policy,
    }
}

fn put_body(key: WireKey, bins: Vec<(String, WireValue)>) -> Vec<u8> {
    let fixture = daemon();
    encode_body(&PutBody {
        target: Target {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            namespace: fixture.namespace.clone(),
            set: fixture.set.clone(),
            key,
            policy: WirePolicy::default(),
        },
        bins,
    })
    .unwrap()
}

fn get_body(key: WireKey, bins: BinSelector) -> Vec<u8> {
    let fixture = daemon();
    encode_body(&GetBody {
        target: Target {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            namespace: fixture.namespace.clone(),
            set: fixture.set.clone(),
            key,
            policy: WirePolicy::default(),
        },
        bins,
    })
    .unwrap()
}

fn target_body(target: Target) -> Vec<u8> {
    encode_body(&TargetBody { target }).unwrap()
}

fn modify_body(target: Target, bins: Vec<(String, WireValue)>) -> Vec<u8> {
    encode_body(&ModifyBody { target, bins }).unwrap()
}

impl Worker {
    /// Send a request that must succeed, and return its reply.
    fn must(&mut self, opcode: u16, body: &[u8]) -> (ReplyHeader, Vec<u8>) {
        let (reply, payload) = self.call(opcode, body);
        assert_eq!(
            reply.status(),
            StatusCode::OK,
            "opcode {opcode} failed: {}",
            error_message(&payload)
        );
        (reply, payload)
    }

    /// Read one record's bins, by name.
    fn bins_of(&mut self, key: WireKey) -> Vec<(String, WireValue)> {
        let (_, payload) = self.must(opcode::GET, &get_body(key, BinSelector::All));
        decode_body::<RecordBody>(&payload).unwrap().bins
    }

    fn bin_of(&mut self, key: WireKey, name: &str) -> WireValue {
        self.bins_of(key)
            .into_iter()
            .find(|(bin, _)| bin == name)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("bin '{name}' is missing from the reply"))
    }
}

/// Read until the answer settles, or fail saying what never arrived.
///
/// Security changes are the one place the server answers before what it just did
/// is visible: a role read back straight after `create-role` can be missing
/// (result code 70) or carry no quota yet, and how long that takes is the
/// cluster's business — one node or twenty, quotas enabled or not. Retrying the
/// *read* is what the server's own tools do; asserting on the first answer pins
/// the test to timing rather than to behaviour.
fn eventually<T>(
    worker: &mut Worker,
    what: &str,
    mut read: impl FnMut(&mut Worker) -> Option<T>,
) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = read(worker) {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "{what} never became visible; the cluster had 10s to catch up"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn require_cluster() -> &'static Fixture {
    let fixture = daemon();
    assert!(
        fixture.ready_instances > 0,
        "no Aerospike cluster is reachable at {} — set AEROSPIKE_HOSTS \
         (and AEROSPIKE_USE_SERVICES_ALTERNATE, if the cluster needs it)",
        hosts()
    );
    fixture
}

// ===== Tests ================================================================

#[test]
fn ping_reports_the_daemon_version_and_its_instances() {
    let mut worker = Worker::attach();
    let (reply, payload) = worker.call(opcode::PING, &[]);

    assert_eq!(reply.status(), StatusCode::OK, "{}", reply.status().label());
    let pong: PongBody = decode_body(&payload).unwrap();
    assert_eq!(pong.daemon_version, aerospike_php_daemon::VERSION);
    assert_eq!(
        pong.instances,
        vec!["default".to_string(), "secondary".to_string()]
    );
}

#[test]
fn put_then_get_returns_the_same_bins() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("round-trip".into());

    // One of every shape the contract carries, including collections nested
    // inside collections.
    let bins = vec![
        ("i".to_string(), WireValue::Int(-42)),
        ("f".to_string(), WireValue::Float(1.5)),
        ("b".to_string(), WireValue::Bool(true)),
        ("s".to_string(), WireValue::Str("héllo ☃".into())),
        ("blob".to_string(), WireValue::Blob(vec![0, 1, 254, 255])),
        (
            "list".to_string(),
            WireValue::List(vec![
                WireValue::Int(1),
                WireValue::Str("two".into()),
                WireValue::List(vec![WireValue::Int(3), WireValue::Nil]),
                WireValue::Map(vec![(WireValue::Str("in-list".into()), WireValue::Int(4))]),
            ]),
        ),
        (
            "map".to_string(),
            WireValue::Map(vec![
                (WireValue::Str("plain".into()), WireValue::Int(1)),
                (
                    WireValue::Str("nested".into()),
                    WireValue::Map(vec![(
                        WireValue::Str("deep".into()),
                        WireValue::List(vec![WireValue::Int(1), WireValue::Int(2)]),
                    )]),
                ),
                (WireValue::Int(7), WireValue::Str("int key".into())),
            ]),
        ),
    ];

    let put = encode_body(&PutBody {
        target: Target {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            namespace: fixture.namespace.clone(),
            set: fixture.set.clone(),
            key: key.clone(),
            policy: WirePolicy::default(),
        },
        bins: bins.clone(),
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::PUT, &put);
    assert_eq!(
        reply.status(),
        StatusCode::OK,
        "PUT failed: {}",
        error_message(&payload)
    );
    assert!(payload.is_empty(), "a write reply carries no payload");

    let get = encode_body(&GetBody {
        target: Target {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            namespace: fixture.namespace.clone(),
            set: fixture.set.clone(),
            key,
            policy: WirePolicy::default(),
        },
        bins: BinSelector::All,
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::GET, &get);
    assert_eq!(
        reply.status(),
        StatusCode::OK,
        "GET failed: {}",
        error_message(&payload)
    );
    assert!(reply.generation >= 1, "a stored record has a generation");

    let record: RecordBody = decode_body(&payload).unwrap();
    for (name, expected) in &bins {
        let actual = record
            .bins
            .iter()
            .find(|(bin, _)| bin == name)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("bin '{name}' is missing from the reply"));
        assert_eq!(
            canonical(actual),
            canonical(expected),
            "bin '{name}' did not round-trip"
        );
    }

    // Whatever order the map came back in, no pair was lost, gained or
    // reshaped — the map bin holds exactly three entries, one of them nested.
    let map = record
        .bins
        .iter()
        .find(|(bin, _)| bin == "map")
        .map(|(_, value)| value)
        .unwrap();
    match map {
        WireValue::Map(pairs) => assert_eq!(pairs.len(), 3, "{pairs:?}"),
        other => panic!("the map bin came back as {other:?}"),
    }
}

#[test]
fn a_bin_selector_narrows_what_comes_back() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Int(770);

    let put = encode_body(&PutBody {
        target: Target {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            namespace: fixture.namespace.clone(),
            set: fixture.set.clone(),
            key: key.clone(),
            policy: WirePolicy::default(),
        },
        bins: vec![
            ("wanted".into(), WireValue::Int(1)),
            ("unwanted".into(), WireValue::Int(2)),
        ],
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::PUT, &put);
    assert_eq!(
        reply.status(),
        StatusCode::OK,
        "{}",
        error_message(&payload)
    );

    let get = |bins: BinSelector| {
        encode_body(&GetBody {
            target: Target {
                instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
                namespace: fixture.namespace.clone(),
                set: fixture.set.clone(),
                key: key.clone(),
                policy: WirePolicy::default(),
            },
            bins,
        })
        .unwrap()
    };

    let (reply, payload) = worker.call(
        opcode::GET,
        &get(BinSelector::Only(vec!["wanted".to_string()])),
    );
    assert_eq!(
        reply.status(),
        StatusCode::OK,
        "{}",
        error_message(&payload)
    );
    let record: RecordBody = decode_body(&payload).unwrap();
    assert_eq!(record.bins, vec![("wanted".to_string(), WireValue::Int(1))]);

    // Metadata only: the generation still arrives, the bins do not.
    let (reply, payload) = worker.call(opcode::GET, &get(BinSelector::None));
    assert_eq!(
        reply.status(),
        StatusCode::OK,
        "{}",
        error_message(&payload)
    );
    let record: RecordBody = decode_body(&payload).unwrap();
    assert!(record.bins.is_empty(), "{:?}", record.bins);
    assert!(reply.generation >= 1);
}

#[test]
fn get_of_a_missing_key_is_record_not_found() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();

    let get = encode_body(&GetBody {
        target: Target {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            namespace: fixture.namespace.clone(),
            set: fixture.set.clone(),
            key: WireKey::Str(format!("absent-{}", std::process::id())),
            policy: WirePolicy::default(),
        },
        bins: BinSelector::All,
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::GET, &get);

    assert_eq!(
        reply.status(),
        StatusCode::RECORD_NOT_FOUND,
        "got {}: {}",
        reply.status().label(),
        error_message(&payload)
    );
    // A missing record is an ordinary outcome, not a server malfunction: it
    // must not arrive as SERVER, and it must still explain itself.
    assert!(!error_message(&payload).is_empty());
}

#[test]
fn an_unknown_instance_is_reported_as_such() {
    let mut worker = Worker::attach();

    let get = encode_body(&GetBody {
        target: Target {
            instance: "no-such-instance".into(),
            namespace: "test".into(),
            set: "s".into(),
            key: WireKey::Int(1),
            policy: WirePolicy::default(),
        },
        bins: BinSelector::All,
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::GET, &get);

    assert_eq!(reply.status(), StatusCode::UNKNOWN_INSTANCE);
    let message = error_message(&payload);
    assert!(message.contains("no-such-instance"), "{message}");
    // The message names what *is* served, so a misconfigured worker is
    // diagnosable from the exception alone.
    assert!(message.contains("default"), "{message}");
}

#[test]
fn a_malformed_payload_is_an_invalid_request() {
    let mut worker = Worker::attach();

    // Bytes that are not a PutBody at all.
    let (reply, payload) = worker.call(opcode::PUT, &[0xFF; 32]);
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    assert!(!error_message(&payload).is_empty());

    // A body that is truncated mid-value rather than random.
    let valid = encode_body(&PutBody {
        target: Target {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.into(),
            namespace: "test".into(),
            set: "s".into(),
            key: WireKey::Str("k".into()),
            policy: WirePolicy::default(),
        },
        bins: vec![("b".into(), WireValue::Str("value".into()))],
    })
    .unwrap();
    let (reply, _) = worker.call(opcode::PUT, &valid[..valid.len() - 3]);
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);

    // An opcode this build does not serve.
    let (reply, payload) = worker.call(9_999, &[]);
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    assert!(
        error_message(&payload).contains("9999"),
        "{}",
        error_message(&payload)
    );

    // And the daemon is still healthy afterwards.
    let (reply, _) = worker.call(opcode::PING, &[]);
    assert!(reply.status().is_ok());
}

#[test]
fn replies_echo_the_request_sequence_across_many_calls_and_workers() {
    let mut worker = Worker::attach();
    let mut seqs = Vec::new();
    for _ in 0..25 {
        let (reply, _) = worker.call(opcode::PING, &[]);
        assert!(reply.status().is_ok());
        seqs.push(reply.seq);
    }
    assert_eq!(seqs, (1..=25).collect::<Vec<u64>>());

    // A second worker has its own sequence space, and one service carries
    // both — the many-clients-one-server case PHP-FPM produces.
    let mut other = Worker::attach();
    let (reply, _) = other.call(opcode::PING, &[]);
    assert_eq!(reply.seq, 1);
    let (reply, _) = worker.call(opcode::PING, &[]);
    assert_eq!(reply.seq, 26);
}

#[test]
fn a_frame_from_a_different_protocol_is_rejected_not_acted_on() {
    // Not something the extension can produce, but the daemon must survive it:
    // this is the case the magic and version fields exist for.
    let mut worker = Worker::attach();

    let mut header = RequestHeader::new(opcode::PING, 7, worker.client_id, 1_000, 0);
    header.magic ^= 0xFFFF_FFFF;

    let request = worker.port.loan_slice_uninit(0).unwrap();
    let mut request = request.write_from_slice(&[]);
    *request.user_header_mut() = header;
    let pending = request.send().unwrap();

    let deadline = Instant::now() + CALL_TIMEOUT;
    let (reply, payload) = loop {
        if let Some(response) = pending.receive().unwrap() {
            let reply: ReplyHeader = *response.user_header();
            break (reply, response.payload().to_vec());
        }
        let now = Instant::now();
        assert!(now < deadline, "a bad frame must still be answered");
        worker.waiter.wait_hint(deadline - now).unwrap();
    };

    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    assert!(
        error_message(&payload).contains("marker"),
        "{}",
        error_message(&payload)
    );

    // The daemon kept serving, and a fresh worker is unaffected.
    let mut healthy = Worker::attach();
    let (reply, _) = healthy.call(opcode::PING, &[]);
    assert!(reply.status().is_ok());
}

// ===== Phase-two verbs ======================================================

#[test]
fn delete_says_whether_the_record_was_there() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("delete-me".into());

    worker.must(
        opcode::PUT,
        &put_body(key.clone(), vec![("b".into(), WireValue::Int(1))]),
    );

    let (_, payload) = worker.must(opcode::DELETE, &target_body(target(key.clone())));
    assert!(
        flag(&payload),
        "deleting a stored record must report that it existed"
    );

    // Deleting again is not an error — the record is gone either way — but the
    // answer changes, which is the whole point of carrying the boolean.
    let (_, payload) = worker.must(opcode::DELETE, &target_body(target(key.clone())));
    assert!(!flag(&payload));

    // And it really is gone.
    let (reply, _) = worker.call(opcode::GET, &get_body(key, BinSelector::All));
    assert_eq!(reply.status(), StatusCode::RECORD_NOT_FOUND);
}

#[test]
fn delete_of_a_key_that_never_existed_is_ok_and_false() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("never-existed-{}", std::process::id()));

    let (reply, payload) = worker.call(opcode::DELETE, &target_body(target(key)));
    assert_eq!(
        reply.status(),
        StatusCode::OK,
        "a delete of a missing record is an answer, not a failure: {}",
        error_message(&payload)
    );
    assert!(!flag(&payload));
}

#[test]
fn exists_answers_both_ways_without_reading_the_record() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("exists-me".into());

    let absent = WireKey::Str(format!("exists-not-{}", std::process::id()));
    let (_, payload) = worker.must(opcode::EXISTS, &target_body(target(absent)));
    assert!(!flag(&payload));

    worker.must(
        opcode::PUT,
        &put_body(key.clone(), vec![("b".into(), WireValue::Int(1))]),
    );
    let (reply, payload) = worker.must(opcode::EXISTS, &target_body(target(key)));
    assert!(flag(&payload));
    assert_eq!(
        reply.generation, 0,
        "EXISTS answers a question and carries no record metadata; a caller that \
         wants the generation asks for a metadata-only GET"
    );
}

#[test]
fn touch_bumps_the_generation() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("touch-me".into());

    worker.must(
        opcode::PUT,
        &put_body(key.clone(), vec![("b".into(), WireValue::Int(1))]),
    );
    let (before, _) = worker.must(opcode::GET, &get_body(key.clone(), BinSelector::None));

    worker.must(opcode::TOUCH, &target_body(target(key.clone())));

    let (after, _) = worker.must(opcode::GET, &get_body(key.clone(), BinSelector::None));
    assert_eq!(
        after.generation,
        before.generation + 1,
        "a touch is a write: it bumps the generation"
    );

    // And a touch of a record that is not there fails rather than creating one.
    let absent = WireKey::Str(format!("touch-absent-{}", std::process::id()));
    let (reply, _) = worker.call(opcode::TOUCH, &target_body(target(absent.clone())));
    assert_eq!(reply.status(), StatusCode::RECORD_NOT_FOUND);
    let (reply, _) = worker.call(opcode::EXISTS, &target_body(target(absent)));
    assert!(reply.status().is_ok());
}

#[test]
fn an_expiration_policy_sets_the_ttl_the_next_read_sees() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("ttl-me".into());

    worker.must(
        opcode::PUT,
        &put_body(key.clone(), vec![("b".into(), WireValue::Int(1))]),
    );

    // 1234 seconds from now, named rather than encoded as a magic number.
    if finite_ttls_allowed() {
        let policy = WirePolicy {
            expiration: Some(WireExpiration::Seconds(1_234)),
            ..WirePolicy::default()
        };
        worker.must(
            opcode::TOUCH,
            &target_body(target_with(key.clone(), policy)),
        );

        let (reply, _) = worker.must(opcode::GET, &get_body(key.clone(), BinSelector::None));
        let ttl = reply
            .time_to_live()
            .expect("a record with a finite TTL must not read as never-expiring");
        assert!(
            (1_200..=1_234).contains(&ttl),
            "expected a TTL near 1234s, got {ttl}"
        );
    } else {
        eprintln!(
            "skip: namespace '{}' refuses a finite TTL — its reaper is off and \
             allow-ttl-without-nsup is not set; only Expiration::Never is asserted",
            daemon().namespace
        );
    }

    // `Never` is the other end of the same field, and must not be confused with
    // "use the namespace default".
    let policy = WirePolicy {
        expiration: Some(WireExpiration::Never),
        ..WirePolicy::default()
    };
    worker.must(
        opcode::TOUCH,
        &target_body(target_with(key.clone(), policy)),
    );
    let (reply, _) = worker.must(opcode::GET, &get_body(key, BinSelector::None));
    assert_eq!(
        reply.time_to_live(),
        None,
        "Expiration::Never must arrive as the never-expires sentinel"
    );
}

#[test]
fn add_append_and_prepend_modify_the_bins_in_place() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("modify-me".into());

    worker.must(
        opcode::PUT,
        &put_body(
            key.clone(),
            vec![
                ("count".into(), WireValue::Int(10)),
                ("text".into(), WireValue::Str("middle".into())),
            ],
        ),
    );

    worker.must(
        opcode::ADD,
        &modify_body(
            target(key.clone()),
            vec![("count".into(), WireValue::Int(5))],
        ),
    );
    assert_eq!(worker.bin_of(key.clone(), "count"), WireValue::Int(15));

    // A negative delta is the same verb, and the only way to count down.
    worker.must(
        opcode::ADD,
        &modify_body(
            target(key.clone()),
            vec![("count".into(), WireValue::Int(-6))],
        ),
    );
    assert_eq!(worker.bin_of(key.clone(), "count"), WireValue::Int(9));

    worker.must(
        opcode::APPEND,
        &modify_body(
            target(key.clone()),
            vec![("text".into(), WireValue::Str("-end".into()))],
        ),
    );
    worker.must(
        opcode::PREPEND,
        &modify_body(
            target(key.clone()),
            vec![("text".into(), WireValue::Str("start-".into()))],
        ),
    );
    assert_eq!(
        worker.bin_of(key.clone(), "text"),
        WireValue::Str("start-middle-end".into())
    );

    // Adding to a string bin is the server's error to report, not a panic here.
    let (reply, payload) = worker.call(
        opcode::ADD,
        &modify_body(target(key), vec![("text".into(), WireValue::Int(1))]),
    );
    assert_eq!(
        reply.status(),
        StatusCode::SERVER,
        "got {}: {}",
        reply.status().label(),
        error_message(&payload)
    );
    assert!(reply.result_code().is_some());
}

#[test]
fn a_generation_guarded_write_fails_with_the_servers_own_code() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("guarded".into());

    worker.must(
        opcode::PUT,
        &put_body(key.clone(), vec![("n".into(), WireValue::Int(1))]),
    );
    let (reply, _) = worker.must(opcode::GET, &get_body(key.clone(), BinSelector::None));
    let generation = reply.generation;

    let guarded = |generation: u32| WirePolicy {
        generation_policy: Some(WireGenerationPolicy::ExpectGenEqual),
        generation: Some(generation),
        ..WirePolicy::default()
    };

    // The generation the record actually has: the write lands.
    worker.must(
        opcode::ADD,
        &modify_body(
            target_with(key.clone(), guarded(generation)),
            vec![("n".into(), WireValue::Int(1))],
        ),
    );
    assert_eq!(worker.bin_of(key.clone(), "n"), WireValue::Int(2));

    // A stale generation: refused, with result code 3 (generation error) so PHP
    // can tell a lost race from a broken cluster.
    let (reply, payload) = worker.call(
        opcode::ADD,
        &modify_body(
            target_with(key.clone(), guarded(generation)),
            vec![("n".into(), WireValue::Int(1))],
        ),
    );
    assert_eq!(
        reply.status(),
        StatusCode::SERVER,
        "got {}: {}",
        reply.status().label(),
        error_message(&payload)
    );
    assert_eq!(reply.result_code(), Some(3), "{}", error_message(&payload));
    assert!(!reply.in_doubt(), "a refused write provably did not land");

    // And the refusal really did not change the record.
    assert_eq!(worker.bin_of(key, "n"), WireValue::Int(2));
}

#[test]
fn a_read_refuses_a_write_only_policy_field_by_name() {
    require_cluster();
    let mut worker = Worker::attach();

    let policy = WirePolicy {
        expiration: Some(WireExpiration::Seconds(60)),
        ..WirePolicy::default()
    };
    let (reply, payload) = worker.call(
        opcode::EXISTS,
        &target_body(target_with(WireKey::Str("read-policy".into()), policy)),
    );

    assert_eq!(
        reply.status(),
        StatusCode::INVALID_REQUEST,
        "a read that silently drops a TTL is a bug that surfaces much later"
    );
    let message = error_message(&payload);
    assert!(message.contains("expiration"), "{message}");

    // The daemon is still healthy, and the same read without the field works.
    let (reply, payload) = worker.call(
        opcode::EXISTS,
        &target_body(target(WireKey::Str("read-policy".into()))),
    );
    assert_eq!(
        reply.status(),
        StatusCode::OK,
        "{}",
        error_message(&payload)
    );
}

#[test]
fn a_server_compiled_filter_is_applied_rather_than_ignored() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("filtered".into());

    worker.must(
        opcode::PUT,
        &put_body(key.clone(), vec![("n".into(), WireValue::Int(10))]),
    );

    let with_filter = |ael: &str| WirePolicy {
        filter: Some(WireExpression::Ael(ael.to_string())),
        ..WirePolicy::default()
    };

    // A filter the record satisfies: the write lands.
    let (reply, payload) = worker.call(
        opcode::TOUCH,
        &target_body(target_with(key.clone(), with_filter("$.n == 10"))),
    );
    if reply.status() == StatusCode::INVALID_REQUEST {
        let message = error_message(&payload);
        assert!(
            message.contains("8.1.3"),
            "the only acceptable refusal here is the version gate: {message}"
        );
        eprintln!("skipping: this cluster cannot compile AEL ({message})");
        return;
    }
    assert_eq!(
        reply.status(),
        StatusCode::OK,
        "{}",
        error_message(&payload)
    );

    // One it does not: the server refuses the write with result code 27
    // (filtered out) rather than applying it.
    let (reply, payload) = worker.call(
        opcode::TOUCH,
        &target_body(target_with(key, with_filter("$.n == 11"))),
    );
    assert_eq!(
        reply.status(),
        StatusCode::SERVER,
        "got {}: {}",
        reply.status().label(),
        error_message(&payload)
    );
    assert_eq!(reply.result_code(), Some(27), "{}", error_message(&payload));
}

#[test]
fn the_value_shapes_added_in_phase_two_survive_the_server() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str("shapes".into());

    let sorted = WireValue::SortedMap(vec![
        (WireValue::Str("a".into()), WireValue::Int(1)),
        (WireValue::Str("b".into()), WireValue::Int(2)),
    ]);
    let ordered = WireValue::OrderedMap(vec![
        (WireValue::Str("x".into()), WireValue::Int(1)),
        (WireValue::Str("y".into()), WireValue::Int(2)),
    ]);
    let geo = WireValue::GeoJson("{\"type\":\"Point\",\"coordinates\":[13.5,42.0]}".into());

    worker.must(
        opcode::PUT,
        &put_body(
            key.clone(),
            vec![
                ("sorted".into(), sorted.clone()),
                ("ordered".into(), ordered),
                ("geo".into(), geo),
            ],
        ),
    );

    // A K-ordered map is distinguishable in storage, so it comes back as one.
    assert_eq!(worker.bin_of(key.clone(), "sorted"), sorted);
    // An insertion-ordered map is not: the server has no such type, so it comes
    // back as a plain map in the server's own key order.
    assert_eq!(
        canonical(&worker.bin_of(key.clone(), "ordered")),
        canonical(&WireValue::Map(vec![
            (WireValue::Str("x".into()), WireValue::Int(1)),
            (WireValue::Str("y".into()), WireValue::Int(2)),
        ]))
    );
    // GeoJSON must not degrade to a string: the server stores it as geometry.
    match worker.bin_of(key, "geo") {
        WireValue::GeoJson(json) => assert!(json.contains("Point"), "{json}"),
        other => panic!("the GeoJSON bin came back as {other:?}"),
    }
}

#[test]
fn a_result_only_value_is_refused_and_names_its_bin() {
    require_cluster();
    let mut worker = Worker::attach();

    let (reply, payload) = worker.call(
        opcode::PUT,
        &put_body(
            WireKey::Str("result-only".into()),
            vec![
                ("fine".into(), WireValue::Int(1)),
                (
                    "impossible".into(),
                    WireValue::MultiResult(vec![WireValue::Int(1)]),
                ),
            ],
        ),
    );

    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    let message = error_message(&payload);
    assert!(message.contains("impossible"), "{message}");

    // Nothing was written: the whole request was refused before it reached the
    // cluster, not applied for the bins that happened to be fine.
    let (_, payload) = worker.must(
        opcode::EXISTS,
        &target_body(target(WireKey::Str("result-only".into()))),
    );
    assert!(!flag(&payload));
}

/// The shipped binary, the shipped example configuration, a PING, and a
/// graceful SIGTERM — the deployment path, not just the library.
#[test]
#[cfg(unix)]
fn the_binary_starts_against_the_example_configuration_and_answers_a_ping() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let instance = format!("phpd-bin-{}", std::process::id());

    // The example verbatim, with only the two things a test must not inherit:
    // the instance name (which would collide with a deployed daemon) and the
    // cluster it points at (which is wherever this run's server is).
    let example = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("aerospike-daemon.toml.example"),
    )
    .expect("the example configuration must be readable");
    let seed = "host = \"127.0.0.1:3000\"";
    assert!(
        example.contains(seed),
        "the example configuration no longer has the seed line this test rewrites"
    );
    let text = example
        .replace(
            "instance = \"default\"",
            &format!("instance = \"{instance}\""),
        )
        .replace(seed, cluster_keys().trim_end());

    let path = std::env::temp_dir().join(format!("{instance}.toml"));
    std::fs::write(&path, text).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_aerospike-php-daemon"))
        .arg("--config")
        .arg(&path)
        // Not RUST_LOG from the environment: this test reads the startup log.
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the daemon binary must start");

    // Wait for the daemon to announce its service rather than racing it, and
    // keep draining the pipe so it can never fill and block the child.
    let stderr = BufReader::new(child.stderr.take().unwrap());
    let (serving_tx, serving_rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in stderr.lines().map_while(Result::ok) {
            eprintln!("daemon: {line}");
            if line.contains("serving '") {
                serving_tx.send(()).ok();
            }
        }
    });
    let started = serving_rx.recv_timeout(Duration::from_secs(30)).is_ok();

    let ping = if started {
        let mut worker = Worker::attach_to(&instance);
        let (reply, payload) = worker.call(opcode::PING, &[]);
        Some((reply.status(), decode_body::<PongBody>(&payload).ok()))
    } else {
        None
    };

    // SIGTERM, not SIGKILL: shutting down cleanly is part of what is being
    // tested. Reap the child before asserting, so a failure cannot leave a
    // daemon behind holding the service name.
    Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .expect("kill must run");
    let status = child.wait().expect("the daemon must be reapable");
    std::fs::remove_file(&path).ok();

    assert!(started, "the daemon never reported that it was serving");
    let (status_code, pong) = ping.unwrap();
    assert_eq!(status_code, StatusCode::OK);
    let pong = pong.expect("PING must answer with a PongBody");
    assert_eq!(pong.daemon_version, aerospike_php_daemon::VERSION);
    assert_eq!(
        pong.instances,
        vec![aerospike_php_ipc::DEFAULT_INSTANCE.to_string()],
        "the example configures exactly one cluster"
    );
    assert!(
        status.success(),
        "SIGTERM must shut the daemon down cleanly, got {status:?}"
    );
}

// ===== OPERATE ==============================================================

fn operate_body(key: WireKey, ops: Vec<WireOp>) -> Vec<u8> {
    encode_body(&OperateBody {
        target: target(key),
        ops,
    })
    .unwrap()
}

/// The ordering guarantee is the reason `operate` exists, so it is what this
/// pins: a write, then a read of what that write produced, in one round trip.
#[test]
fn operate_runs_its_operations_in_order() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-order-{}", std::process::id()));

    worker.must(opcode::DELETE, &target_body(target(key.clone())));

    let (reply, payload) = worker.must(
        opcode::OPERATE,
        &operate_body(
            key.clone(),
            vec![
                WireOp::Put {
                    bin: "n".into(),
                    value: WireValue::Int(10),
                },
                WireOp::Add {
                    bin: "n".into(),
                    value: WireValue::Int(5),
                },
                WireOp::GetBin { bin: "n".into() },
            ],
        ),
    );
    let record: RecordBody = decode_body(&payload).unwrap();
    assert_eq!(
        record.bins,
        vec![("n".into(), WireValue::Int(15))],
        "the read must see the sum of the write and the increment before it"
    );
    assert_eq!(reply.generation, 1, "one operate is one write");
}

/// Two operations that both report a result for one bin cannot each own the bin
/// name, so the server answers with a list — and that shape has to survive the
/// contract rather than being flattened into the last one.
#[test]
fn two_results_for_one_bin_come_back_as_a_list() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-multi-{}", std::process::id()));

    worker.must(opcode::DELETE, &target_body(target(key.clone())));

    let (_, payload) = worker.must(
        opcode::OPERATE,
        &operate_body(
            key,
            vec![
                WireOp::List {
                    bin: "items".into(),
                    ctx: vec![],
                    op: WireListOp::AppendItems {
                        policy: WireListPolicy::default(),
                        values: vec![WireValue::Int(1), WireValue::Int(2)],
                    },
                },
                WireOp::List {
                    bin: "items".into(),
                    ctx: vec![],
                    op: WireListOp::Size,
                },
            ],
        ),
    );
    let record: RecordBody = decode_body(&payload).unwrap();
    assert_eq!(
        record.bins,
        vec![(
            "items".into(),
            WireValue::MultiResult(vec![WireValue::Int(2), WireValue::Int(2)])
        )],
        "the append's new size and the size read must both be reported"
    );
}

/// A path is the difference between operating on the bin and operating on
/// something inside it, so a nested operation must reach the inner list and
/// leave the outer map alone.
#[test]
fn a_context_path_reaches_a_list_inside_a_map() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-nested-{}", std::process::id()));

    worker.must(
        opcode::PUT,
        &put_body(
            key.clone(),
            vec![(
                "profile".into(),
                WireValue::Map(vec![(
                    WireValue::Str("roles".into()),
                    WireValue::List(vec![WireValue::Str("reader".into())]),
                )]),
            )],
        ),
    );

    worker.must(
        opcode::OPERATE,
        &operate_body(
            key.clone(),
            vec![WireOp::List {
                bin: "profile".into(),
                ctx: vec![WireCtx::MapKey {
                    key: WireValue::Str("roles".into()),
                }],
                op: WireListOp::Append {
                    policy: WireListPolicy::default(),
                    value: WireValue::Str("admin".into()),
                },
            }],
        ),
    );

    let (_, payload) = worker.must(opcode::GET, &get_body(key, BinSelector::All));
    let record: RecordBody = decode_body(&payload).unwrap();
    assert_eq!(
        canonical(&record.bins[0].1),
        canonical(&WireValue::Map(vec![(
            WireValue::Str("roles".into()),
            WireValue::List(vec![
                WireValue::Str("reader".into()),
                WireValue::Str("admin".into())
            ]),
        )])),
        "the append must have gone into the nested list, leaving the map itself intact"
    );
}

/// A request that asks nothing has no answer, and the daemon should say so
/// rather than let the server reject it with a bare parameter error.
#[test]
fn operate_with_no_operations_is_an_invalid_request() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-empty-{}", std::process::id()));

    let (reply, payload) = worker.call(opcode::OPERATE, &operate_body(key, vec![]));
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    assert!(
        error_message(&payload).contains("at least one operation"),
        "the message must say what was missing: {}",
        error_message(&payload)
    );
}

/// A read-only `operate` on a record that is not there is the same answer a
/// `GET` gives, because nothing in the call would have created it.
#[test]
fn a_read_only_operate_on_a_missing_record_is_record_not_found() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-absent-{}", std::process::id()));

    let (reply, _) = worker.call(
        opcode::OPERATE,
        &operate_body(
            key,
            vec![WireOp::List {
                bin: "items".into(),
                ctx: vec![],
                op: WireListOp::Size,
            }],
        ),
    );
    assert_eq!(reply.status(), StatusCode::RECORD_NOT_FOUND);
}

/// A map operation must be able to reach a map nested in a map, and the two
/// halves of an entry must come back as the return type asked for.
#[test]
fn map_operations_read_keys_values_or_both() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-map-{}", std::process::id()));

    worker.must(opcode::DELETE, &target_body(target(key.clone())));

    let map = |op| WireOp::Map {
        bin: "scores".into(),
        ctx: vec![],
        op,
    };
    let (_, payload) = worker.must(
        opcode::OPERATE,
        &operate_body(
            key,
            vec![
                map(WireMapOp::PutItems {
                    policy: WireMapPolicy::default(),
                    items: vec![
                        (WireValue::Str("alice".into()), WireValue::Int(10)),
                        (WireValue::Str("bob".into()), WireValue::Int(20)),
                    ],
                }),
                map(WireMapOp::GetByRank {
                    rank: -1,
                    return_type: WireMapReturn::of(WireMapReturnKind::Key),
                }),
                map(WireMapOp::GetByRank {
                    rank: -1,
                    return_type: WireMapReturn::of(WireMapReturnKind::Value),
                }),
            ],
        ),
    );
    let record: RecordBody = decode_body(&payload).unwrap();
    assert_eq!(
        record.bins,
        vec![(
            "scores".into(),
            WireValue::MultiResult(vec![
                WireValue::Int(2),
                WireValue::Str("bob".into()),
                WireValue::Int(20),
            ])
        )],
        "the size, then the largest entry's key, then its value"
    );
}

/// `maps::create` folds its order flag into the last context element and
/// degrades to `set_order` without one, so the daemon has to pass the path as a
/// constructor argument rather than attaching it afterwards. This is what proves
/// it does: a create at a path must build the map *there*.
#[test]
fn map_create_uses_the_path_it_was_given() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-map-create-{}", std::process::id()));

    worker.must(opcode::DELETE, &target_body(target(key.clone())));

    worker.must(
        opcode::OPERATE,
        &operate_body(
            key.clone(),
            vec![
                // Create the outer map, then a nested one under "inner", then
                // write into the nested map.
                WireOp::Map {
                    bin: "tree".into(),
                    ctx: vec![],
                    op: WireMapOp::SetOrder {
                        order: WireMapOrder::KeyOrdered,
                    },
                },
                WireOp::Map {
                    bin: "tree".into(),
                    ctx: vec![WireCtx::MapKeyCreate {
                        key: WireValue::Str("inner".into()),
                        order: WireMapOrder::KeyOrdered,
                    }],
                    op: WireMapOp::Put {
                        policy: WireMapPolicy::default(),
                        key: WireValue::Str("leaf".into()),
                        value: WireValue::Int(7),
                    },
                },
            ],
        ),
    );

    let (_, payload) = worker.must(opcode::GET, &get_body(key, BinSelector::All));
    let record: RecordBody = decode_body(&payload).unwrap();
    assert_eq!(
        canonical(&record.bins[0].1),
        // A `SortedMap`, not a `Map`: the outer map was set key-ordered, and
        // unlike insertion order, *key* order survives storage and comes back as
        // a distinguishable kind. The inner map was created by the path's
        // `MapKeyCreate`, which was told KeyOrdered too — but it holds one entry,
        // which the server reports as a plain map.
        canonical(&WireValue::SortedMap(vec![(
            WireValue::Str("inner".into()),
            WireValue::Map(vec![(WireValue::Str("leaf".into()), WireValue::Int(7))]),
        )])),
        "the write must have landed in a map created at the path"
    );
}

/// The bit family is the one whose units change from operation to operation, so
/// this pins that a byte-sized resize and a bit-sized read agree about the same
/// blob.
#[test]
fn bit_operations_work_in_bits_and_bytes_on_one_blob() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-bits-{}", std::process::id()));

    worker.must(
        opcode::PUT,
        &put_body(
            key.clone(),
            vec![("flags".into(), WireValue::Blob(vec![0x01, 0x02]))],
        ),
    );

    let bit = |op| WireOp::Bit {
        bin: "flags".into(),
        ctx: vec![],
        op,
    };
    let (_, payload) = worker.must(
        opcode::OPERATE,
        &operate_body(
            key.clone(),
            vec![
                // Two bytes, three set bits between them.
                bit(WireBitOp::Count {
                    bit_offset: 0,
                    bit_size: 16,
                }),
                // Grow to four bytes, then count again: the new bytes are zero.
                bit(WireBitOp::Resize {
                    byte_size: 4,
                    flags: None,
                    policy: WireBitPolicy::default(),
                }),
                bit(WireBitOp::Count {
                    bit_offset: 0,
                    bit_size: 32,
                }),
            ],
        ),
    );
    let record: RecordBody = decode_body(&payload).unwrap();
    assert_eq!(
        record.bins,
        vec![(
            "flags".into(),
            WireValue::MultiResult(vec![WireValue::Int(2), WireValue::Int(2)])
        )],
        "resize reports nothing, and growing in bytes adds no set bits"
    );
}

/// A HyperLogLog answers with estimates, so this pins the shape and the exact
/// small-cardinality answers rather than anything statistical.
#[test]
fn hll_counts_distinct_values_and_describes_itself() {
    require_cluster();
    let mut worker = Worker::attach();
    let key = WireKey::Str(format!("operate-hll-{}", std::process::id()));

    worker.must(opcode::DELETE, &target_body(target(key.clone())));

    let hll = |op| WireOp::Hll {
        bin: "seen".into(),
        ctx: vec![],
        op,
    };
    let value = |text: &str| WireValue::Str(text.to_owned());

    let (_, payload) = worker.must(
        opcode::OPERATE,
        &operate_body(
            key,
            vec![
                hll(WireHllOp::Add {
                    values: vec![value("a"), value("b"), value("a")],
                    index_bit_count: Some(12),
                    min_hash_bit_count: None,
                    policy: WireHllPolicy::default(),
                }),
                hll(WireHllOp::GetCount),
                hll(WireHllOp::Describe),
            ],
        ),
    );
    let record: RecordBody = decode_body(&payload).unwrap();
    assert_eq!(
        record.bins,
        vec![(
            "seen".into(),
            WireValue::MultiResult(vec![
                // Two registers changed: the duplicate "a" changes nothing,
                // which is the whole point of the structure.
                WireValue::Int(2),
                WireValue::Int(2),
                WireValue::List(vec![WireValue::Int(12), WireValue::Int(0)]),
            ])
        )]
    );
}

// ===== Scans, in pages ======================================================

/// How many records the scan tests seed. Small enough to be quick, large enough
/// that a page size of four needs several pages.
const SCANNED: usize = 25;

/// A set of this run's own, seeded once.
///
/// Its own set because a scan sees everything in one, and every other test in
/// this binary writes to the shared set — in parallel. A scan of that would
/// count records other tests were still creating.
fn scanned_set() -> &'static String {
    static SET: OnceLock<String> = OnceLock::new();
    SET.get_or_init(|| {
        let fixture = require_cluster();
        let set = format!("{}scan", fixture.set);
        let mut worker = Worker::attach();
        for index in 0..SCANNED {
            let body = encode_body(&PutBody {
                target: Target {
                    instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
                    namespace: fixture.namespace.clone(),
                    set: set.clone(),
                    key: WireKey::Int(index as i64),
                    policy: WirePolicy {
                        // So the scanned records carry their user keys: without
                        // this the server stores only a digest, and a scan has
                        // no way to say which record it found.
                        send_key: Some(true),
                        ..WirePolicy::default()
                    },
                },
                bins: vec![("n".into(), WireValue::Int(index as i64))],
            })
            .unwrap();
            worker.must(opcode::PUT, &body);
        }
        set
    })
}

fn scan_request(page_size: u32, max_records: u64, include_bin_data: bool) -> Vec<u8> {
    let fixture = daemon();
    encode_body(&WireQueryBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        policy: WirePolicy::default(),
        statement: WireStatement {
            namespace: fixture.namespace.clone(),
            set: scanned_set().clone(),
            bins: BinSelector::All,
            filter: None,
        },
        partitions: WirePartitions::All,
        page_size,
        max_records,
        records_per_second: 0,
        include_bin_data,
    })
    .unwrap()
}

impl Worker {
    /// Read a whole traversal, page by page, and report how many pages it took.
    ///
    /// The loop is the contract's: keep asking while a cursor comes back, and
    /// stop only when one does not. An empty page is *not* a stop condition —
    /// the server hands a page's budget out per node, so a page can be empty
    /// with partitions still unread.
    fn drain_scan(&mut self, first: Vec<u8>) -> (Vec<WireQueryRecord>, usize) {
        let (_, payload) = self.must(opcode::QUERY, &first);
        let mut page: WireQueryPage = decode_body(&payload).unwrap();
        let mut records = page.records;
        let mut pages = 1;

        while let Some(cursor) = page.cursor {
            let body = encode_body(&WireCursorBody { cursor }).unwrap();
            let (_, payload) = self.must(opcode::QUERY_NEXT, &body);
            page = decode_body(&payload).unwrap();
            records.extend(page.records.drain(..));
            pages += 1;
            assert!(
                pages < 200,
                "a {SCANNED}-record scan must not take 200 pages"
            );
        }
        (records, pages)
    }
}

#[test]
fn a_paged_scan_returns_every_record_exactly_once() {
    let mut worker = Worker::attach();
    let (records, pages) = worker.drain_scan(scan_request(4, 0, true));

    assert!(pages > 1, "a page of four cannot hold {SCANNED} records");
    assert_eq!(records.len(), SCANNED, "the scan must be complete");

    // Every record once, identified by its digest — which is the identity a
    // scan always has, whether or not the key was stored.
    let mut digests: Vec<[u8; 20]> = records.iter().map(|r| r.key.digest).collect();
    digests.sort_unstable();
    digests.dedup();
    assert_eq!(digests.len(), SCANNED, "a record was returned twice");

    let mut seen: Vec<i64> = records
        .iter()
        .map(|record| match &record.bins[..] {
            [(name, WireValue::Int(n))] if name == "n" => *n,
            other => panic!("unexpected bins {other:?}"),
        })
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..SCANNED as i64).collect::<Vec<_>>());

    // These records were written with `send_key`, so their user keys came back
    // as well as their digests.
    for record in &records {
        assert!(
            matches!(record.key.user_key, Some(WireKey::Int(_))),
            "a record written with send_key must come back with it: {:?}",
            record.key
        );
        assert_eq!(record.key.set, *scanned_set());
        assert_eq!(record.generation, 1);
    }
}

/// The ceiling is the other way a traversal finishes, and it must finish *and*
/// stop asking: a cursor handed out after the ceiling was reached would be one
/// nobody could ever exhaust.
#[test]
fn a_record_ceiling_ends_the_scan_even_though_partitions_remain() {
    let mut worker = Worker::attach();
    let (_, payload) = worker.must(opcode::QUERY, &scan_request(100, 5, true));
    let page: WireQueryPage = decode_body(&payload).unwrap();

    assert_eq!(page.records.len(), 5);
    assert_eq!(
        page.cursor, None,
        "the ceiling was reached, so there is nothing more to ask for"
    );
}

/// A traversal that fits in one page must not register a cursor at all: that is
/// what makes an ordinary query cost one round trip and leave nothing behind.
#[test]
fn a_scan_that_fits_in_one_page_leaves_no_cursor() {
    let mut worker = Worker::attach();
    let (_, payload) = worker.must(opcode::QUERY, &scan_request(1_000, 0, true));
    let page: WireQueryPage = decode_body(&payload).unwrap();

    assert_eq!(page.records.len(), SCANNED);
    assert_eq!(page.cursor, None);
}

#[test]
fn a_scan_can_ask_for_metadata_without_bins() {
    let mut worker = Worker::attach();
    let (records, _) = worker.drain_scan(scan_request(10, 0, false));

    assert_eq!(records.len(), SCANNED);
    for record in &records {
        assert!(
            record.bins.is_empty(),
            "include_bin_data was false, so no bin should have come back: {:?}",
            record.bins
        );
        // The identity and the metadata still do, which is what makes this
        // worth asking for: a set can be counted, or its digests collected,
        // without moving its data.
        assert_ne!(record.key.digest, [0u8; 20]);
        assert_eq!(record.generation, 1);
    }
}

/// Closing a traversal releases the cursor, and the next request for it says so
/// rather than answering with an empty page — the distinction the whole design
/// rests on.
#[test]
fn a_closed_scan_reports_its_cursor_as_expired() {
    let mut worker = Worker::attach();
    let (_, payload) = worker.must(opcode::QUERY, &scan_request(1, 0, true));
    let page: WireQueryPage = decode_body(&payload).unwrap();
    let cursor = page
        .cursor
        .expect("a page of one cannot have finished 25 records");

    let body = encode_body(&WireCursorBody { cursor }).unwrap();
    worker.must(opcode::QUERY_CLOSE, &body);
    // Twice, because a destructor cannot know whether the scan was read out.
    worker.must(opcode::QUERY_CLOSE, &body);

    let (reply, payload) = worker.call(opcode::QUERY_NEXT, &body);
    assert_eq!(reply.status(), StatusCode::CURSOR_EXPIRED);
    let message = error_message(&payload);
    assert!(message.contains(&cursor.to_string()), "{message}");
    assert!(message.contains("start again"), "{message}");
}

/// One scan divided between workers by partition range: each sees a disjoint
/// part of the ring, and together they see all of it. This is how a scan is
/// parallelised, and it only works if the ranges really do partition the set.
#[test]
fn a_partition_range_divides_a_scan_without_overlap() {
    let fixture = require_cluster();
    let set = scanned_set().clone();
    let quarter = |begin: u32| -> Vec<u8> {
        encode_body(&WireQueryBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            policy: WirePolicy::default(),
            statement: WireStatement {
                namespace: fixture.namespace.clone(),
                set: set.clone(),
                bins: BinSelector::None,
                filter: None,
            },
            partitions: WirePartitions::Range {
                begin,
                count: 1_024,
            },
            page_size: 100,
            max_records: 0,
            records_per_second: 0,
            include_bin_data: false,
        })
        .unwrap()
    };

    let mut all: Vec<[u8; 20]> = Vec::new();
    for begin in [0, 1_024, 2_048, 3_072] {
        let mut worker = Worker::attach();
        let (records, _) = worker.drain_scan(quarter(begin));
        all.extend(records.iter().map(|record| record.key.digest));
    }

    assert_eq!(
        all.len(),
        SCANNED,
        "the four ranges must cover the ring once"
    );
    all.sort_unstable();
    let before = all.len();
    all.dedup();
    assert_eq!(all.len(), before, "two ranges returned the same record");
}

// ===== Cluster management ===================================================

/// The one Lua module these tests register. Deliberately trivial: what is under
/// test is that a module crosses the channel, registers, runs and is removed —
/// not the Lua.
const UDF_SOURCE: &str = r#"
function bump(rec, name, amount)
    if not aerospike:exists(rec) then aerospike:create(rec) end
    if rec[name] == nil then rec[name] = 0 end
    rec[name] = rec[name] + amount
    aerospike:update(rec)
    return rec[name]
end
"#;

impl Worker {
    /// Poll a task handle until it completes, as PHP does.
    ///
    /// The wait is the *caller's* loop here for the same reason it is in the
    /// extension: nothing in the daemon blocks, so no request outlives a
    /// worker's reply deadline.
    fn await_task(&mut self, handle: &WireTaskHandle) -> WireTaskStatus {
        let body = encode_body(&WireTaskStatusBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            handle: handle.clone(),
        })
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let (_, payload) = self.must(opcode::TASK_STATUS, &body);
            match decode_body::<WireTaskStatus>(&payload).unwrap() {
                WireTaskStatus::Complete => return WireTaskStatus::Complete,
                other => assert!(
                    Instant::now() < deadline,
                    "task {handle:?} was still {other:?} after 30s"
                ),
            }
        }
    }
}

#[test]
fn the_node_list_is_the_daemons_own_view() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();
    let body = encode_body(&WireNodesBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::NODES, &body);
    let nodes: WireNodes = decode_body(&payload).unwrap();

    assert!(!nodes.nodes.is_empty(), "a reachable cluster has nodes");
    for node in &nodes.nodes {
        assert!(!node.name.is_empty(), "a node has a name");
        assert!(node.address.contains(':'), "{}", node.address);
        // Four parts, because that is what the client's own version type has and
        // what a caller comparing versions needs.
        assert_eq!(
            node.version.split('.').count(),
            4,
            "version {} is not major.minor.patch.build",
            node.version
        );
        assert!(node.active, "a node the daemon routes to is active");
    }
    let _ = fixture;
}

#[test]
fn info_answers_every_command_in_order() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();
    let body = encode_body(&WireInfoBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
        node: None,
        commands: vec!["build".into(), "namespaces".into()],
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::INFO, &body);
    let reply: WireInfoReply = decode_body(&payload).unwrap();

    // One answer per command, in the order asked — which is the whole reason the
    // contract carries pairs rather than a map.
    let asked: Vec<&str> = reply.values.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(asked, vec!["build", "namespaces"]);
    assert!(
        !reply.values[0].1.is_empty(),
        "the server reported no build"
    );
    assert!(
        reply.values[1]
            .1
            .split(';')
            .any(|ns| ns == fixture.namespace),
        "namespaces = {}",
        reply.values[1].1
    );

    // A named node, which is what the per-node commands need.
    let named = encode_body(&WireInfoBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
        node: Some(worker_node_name(&mut worker)),
        commands: vec!["build".into()],
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::INFO, &named);
    let from_node: WireInfoReply = decode_body(&payload).unwrap();
    assert_eq!(from_node.values[0].1, reply.values[0].1);
}

/// An info request with no commands is refused rather than sent.
#[test]
fn info_with_no_commands_is_an_invalid_request() {
    let mut worker = Worker::attach();
    let body = encode_body(&WireInfoBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
        node: None,
        commands: Vec::new(),
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::INFO, &body);
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    assert!(
        error_message(&payload).contains("at least one command"),
        "{}",
        error_message(&payload)
    );
}

/// The name of any node, for the info test that needs one.
fn worker_node_name(worker: &mut Worker) -> String {
    let body = encode_body(&WireNodesBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::NODES, &body);
    decode_body::<WireNodes>(&payload).unwrap().nodes[0]
        .name
        .clone()
}

/// The whole UDF lifecycle, and with it the task-handle mechanism: a handle is
/// rebuilt from its description on every status call, so waiting works with
/// nothing kept in the daemon between requests.
#[test]
fn a_udf_registers_runs_and_is_removed() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();
    let module = format!("phpd_{}.lua", std::process::id());

    let register = encode_body(&WireUdfRegisterBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
        server_path: module.clone(),
        source: UDF_SOURCE.as_bytes().to_vec(),
        language: WireUdfLang::Lua,
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::UDF_REGISTER, &register);
    let handle: WireTaskHandle = decode_body(&payload).unwrap();
    assert_eq!(
        handle,
        WireTaskHandle::UdfRegister {
            package: module.clone()
        }
    );
    assert_eq!(worker.await_task(&handle), WireTaskStatus::Complete);

    // The module is listed, with the hash the server computed for it.
    let list = encode_body(&WireUdfListBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::UDF_LIST, &list);
    let modules: WireUdfList = decode_body(&payload).unwrap();
    let mine = modules
        .modules
        .iter()
        .find(|m| m.name == module)
        .unwrap_or_else(|| panic!("{module} is not in {:?}", modules.modules));
    assert!(!mine.hash.is_empty(), "a listed module has a hash");
    assert_eq!(mine.language, "LUA");

    // Run it. The package name drops the extension in a call but not in a
    // registration — that asymmetry is the server's.
    let package = module.trim_end_matches(".lua").to_string();
    let key = WireKey::Str("udf-target".into());
    worker.must(opcode::DELETE, &target_body(target(key.clone())));

    let call = encode_body(&WireUdfExecuteBody {
        target: target(key.clone()),
        package: package.clone(),
        function: "bump".into(),
        args: vec![WireValue::Str("hits".into()), WireValue::Int(5)],
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::UDF_EXECUTE, &call);
    let result: WireUdfResult = decode_body(&payload).unwrap();
    assert_eq!(result.value, Some(WireValue::Int(5)));
    assert_eq!(worker.bin_of(key.clone(), "hits"), WireValue::Int(5));

    // Again, so the function's own state — the bin it increments — is what
    // changed rather than the call being idempotent by accident.
    let (_, payload) = worker.must(opcode::UDF_EXECUTE, &call);
    assert_eq!(
        decode_body::<WireUdfResult>(&payload).unwrap().value,
        Some(WireValue::Int(10))
    );

    // A function the module does not have is the server refusing, and it comes
    // back as a SERVER failure with a result code rather than as an INTERNAL
    // one: the command reached a node.
    let missing = encode_body(&WireUdfExecuteBody {
        target: target(key.clone()),
        package,
        function: "nosuchfunction".into(),
        args: Vec::new(),
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::UDF_EXECUTE, &missing);
    assert_eq!(
        reply.status(),
        StatusCode::SERVER,
        "{}",
        error_message(&payload)
    );
    assert_eq!(reply.result_code(), Some(100), "UDF_BAD_RESPONSE");

    let remove = encode_body(&WireUdfRemoveBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
        server_path: module.clone(),
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::UDF_REMOVE, &remove);
    let handle: WireTaskHandle = decode_body(&payload).unwrap();
    assert_eq!(handle, WireTaskHandle::UdfRemove { package: module });
    assert_eq!(worker.await_task(&handle), WireTaskStatus::Complete);

    let _ = fixture;
}

/// An index is created, a query uses it, and it is dropped — with a task for
/// each, whose completion means opposite things.
#[test]
fn an_index_is_created_used_and_dropped() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();
    let set = format!("{}sidx", fixture.set);
    let index = format!("phpd_idx_{}", std::process::id());

    for i in 0..10i64 {
        let body = encode_body(&PutBody {
            target: Target {
                instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
                namespace: fixture.namespace.clone(),
                set: set.clone(),
                key: WireKey::Int(i),
                policy: WirePolicy::default(),
            },
            bins: vec![("age".into(), WireValue::Int(20 + i))],
        })
        .unwrap();
        worker.must(opcode::PUT, &body);
    }

    let drop_body = encode_body(&WireIndexDropBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
        namespace: fixture.namespace.clone(),
        set: set.clone(),
        index_name: index.clone(),
    })
    .unwrap();
    // A leftover index from a previous run would make this depend on how that
    // run ended, so drop first and ignore the failure when there is nothing to
    // drop.
    let _ = worker.call(opcode::INDEX_DROP, &drop_body);

    let create = encode_body(&WireIndexCreateBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
        namespace: fixture.namespace.clone(),
        set: set.clone(),
        index_name: index.clone(),
        on: WireIndexOn::Bin {
            name: "age".into(),
            ctx: Vec::new(),
        },
        index_type: WireIndexType::Numeric,
        collection: aerospike_php_ipc::query::WireCollectionIndex::Default,
    })
    .unwrap();
    let (_, payload) = worker.must(opcode::INDEX_CREATE, &create);
    let creating: WireTaskHandle = decode_body(&payload).unwrap();
    assert_eq!(
        creating,
        WireTaskHandle::Index {
            namespace: fixture.namespace.clone(),
            index_name: index.clone(),
            dropping: false,
        }
    );
    assert_eq!(worker.await_task(&creating), WireTaskStatus::Complete);

    // The query that would have been result code 201 without an index.
    let query = encode_body(&WireQueryBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        policy: WirePolicy::default(),
        statement: WireStatement {
            namespace: fixture.namespace.clone(),
            set: set.clone(),
            bins: BinSelector::All,
            filter: Some(aerospike_php_ipc::query::WireFilter {
                target: aerospike_php_ipc::query::WireFilterTarget::Bin("age".into()),
                kind: aerospike_php_ipc::query::WireFilterKind::Range { begin: 22, end: 25 },
                collection: aerospike_php_ipc::query::WireCollectionIndex::Default,
                context: Vec::new(),
                expression: None,
            }),
        },
        partitions: WirePartitions::All,
        page_size: 100,
        max_records: 0,
        records_per_second: 0,
        include_bin_data: true,
    })
    .unwrap();
    let (records, _) = worker.drain_scan(query);
    let mut ages: Vec<i64> = records
        .iter()
        .filter_map(|record| match &record.bins[..] {
            [(_, WireValue::Int(age))] => Some(*age),
            _ => None,
        })
        .collect();
    ages.sort_unstable();
    assert_eq!(ages, vec![22, 23, 24, 25]);

    let (_, payload) = worker.must(opcode::INDEX_DROP, &drop_body);
    let dropping: WireTaskHandle = decode_body(&payload).unwrap();
    // The same index, and *not* the same handle: for a drop, the index being
    // gone is completion.
    assert_eq!(
        dropping,
        WireTaskHandle::Index {
            namespace: fixture.namespace.clone(),
            index_name: index,
            dropping: true,
        }
    );
    assert_ne!(dropping, creating);
    assert_eq!(worker.await_task(&dropping), WireTaskStatus::Complete);
}

#[test]
fn truncate_empties_a_set_and_honours_a_cutoff() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();
    let set = format!("{}trunc", fixture.set);

    let seed = |worker: &mut Worker| {
        for i in 0..5i64 {
            let body = encode_body(&PutBody {
                target: Target {
                    instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
                    namespace: fixture.namespace.clone(),
                    set: set.clone(),
                    key: WireKey::Int(i),
                    policy: WirePolicy::default(),
                },
                bins: vec![("n".into(), WireValue::Int(i))],
            })
            .unwrap();
            worker.must(opcode::PUT, &body);
        }
    };
    let count = |worker: &mut Worker| -> usize {
        let body = encode_body(&WireQueryBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            policy: WirePolicy::default(),
            statement: WireStatement {
                namespace: fixture.namespace.clone(),
                set: set.clone(),
                bins: BinSelector::None,
                filter: None,
            },
            partitions: WirePartitions::All,
            page_size: 100,
            max_records: 0,
            records_per_second: 0,
            include_bin_data: false,
        })
        .unwrap();
        worker.drain_scan(body).0.len()
    };
    let truncate = |before_nanos| {
        encode_body(&WireTruncateBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            timeout_ms: None,
            namespace: fixture.namespace.clone(),
            set: set.clone(),
            before_nanos,
        })
        .unwrap()
    };

    seed(&mut worker);
    assert_eq!(count(&mut worker), 5);

    // A cutoff an hour ago leaves records written since. Asserted this way round
    // because a cutoff of "now" races the server's clock — it refuses one ahead
    // of its own — while a past cutoff needs no clock agreement and proves the
    // same thing: the argument is applied rather than ignored.
    let an_hour_ago = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    )
    .unwrap()
        - 3_600_000_000_000i64;
    worker.must(opcode::TRUNCATE, &truncate(Some(an_hour_ago)));
    assert_eq!(
        count(&mut worker),
        5,
        "a cutoff before the writes must leave them"
    );

    // No cutoff takes everything, and reads stop seeing the records at once even
    // though the space is reclaimed in the background.
    worker.must(opcode::TRUNCATE, &truncate(None));
    assert_eq!(count(&mut worker), 0);
}

// ===== Multi-record transactions ============================================

/// The namespace the transactional tests use.
///
/// Multi-record transactions require a **strong-consistency** namespace, and the
/// rest of this file requires one that is *not*: SC forbids non-durable deletes,
/// and durable deletes leave tombstones that change what `delete` and `exists`
/// report — which several tests here deliberately pin. So the two halves can need
/// two namespaces, and `AEROSPIKE_SC_NAMESPACE` names the SC one.
///
/// Defaults to `testsc`, the conventional name for one. A cluster without such a
/// namespace answers the probe with nothing recognisable, so the test self-skips
/// and says why.
fn sc_namespace() -> String {
    std::env::var("AEROSPIKE_SC_NAMESPACE").unwrap_or_else(|_| "testsc".to_string())
}

/// One namespace's `namespace/<name>` info block, or `None` if the cluster does
/// not have that namespace (or cannot be asked).
///
/// The tests that need to know how a namespace is *configured* all ask through
/// here: what a server allows is not a property of this client, and a suite that
/// assumed it would report the server's policy as a broken feature.
fn namespace_detail(namespace: &str) -> Option<String> {
    let mut worker = Worker::attach();
    let body = encode_body(&WireInfoBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
        node: None,
        commands: vec![format!("namespace/{namespace}")],
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::INFO, &body);
    if !reply.status().is_ok() {
        return None;
    }
    let detail = decode_body::<WireInfoReply>(&payload)
        .ok()?
        .values
        .into_iter()
        .next()?
        .1;
    // A missing namespace answers with an error string rather than a field list.
    detail.contains('=').then_some(detail)
}

/// One `key=value` out of an info block. Whole-field, not a substring match:
/// `nsup-period=0` also reads as a prefix of nothing, but `contains` would
/// happily find `...-period=0` inside a neighbouring field's value.
fn info_field(detail: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    detail
        .split(';')
        .find_map(|field| field.trim().strip_prefix(&prefix))
        .map(str::to_string)
}

/// Whether [`sc_namespace`] is configured for strong consistency.
///
/// Everything that does *not* need it is asserted either way, which is most of the
/// transaction surface: the registry, the expiry rule, and every refusal.
fn strong_consistency() -> bool {
    static SC: OnceLock<bool> = OnceLock::new();
    *SC.get_or_init(|| {
        namespace_detail(&sc_namespace())
            .and_then(|detail| info_field(&detail, "strong-consistency"))
            .is_some_and(|value| value == "true")
    })
}

/// Whether the test namespace accepts a **finite** TTL.
///
/// A namespace whose reaper is off (`nsup-period=0`) refuses any write that sets
/// an expiration — result code 22, `FailForbidden` — unless it was configured
/// with `allow-ttl-without-nsup`. Both are legitimate server configurations, so
/// the TTL assertions announce a skip rather than failing on one of them.
fn finite_ttls_allowed() -> bool {
    static ALLOWED: OnceLock<bool> = OnceLock::new();
    *ALLOWED.get_or_init(|| {
        // Unknown means "assume yes": a probe that could not be answered must not
        // silently turn a real TTL regression into a skip.
        let Some(detail) = namespace_detail(&daemon().namespace) else {
            return true;
        };
        info_field(&detail, "nsup-period").is_none_or(|period| period != "0")
            || info_field(&detail, "allow-ttl-without-nsup").is_some_and(|v| v == "true")
    })
}

impl Worker {
    /// Open a transaction and return its id.
    fn begin_txn(&mut self) -> i64 {
        let body = encode_body(&WireTxnBeginBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            timeout_ms: None,
        })
        .unwrap();
        let (_, payload) = self.must(opcode::TXN_BEGIN, &body);
        decode_body::<WireTxnHandle>(&payload).unwrap().id
    }

    /// A body naming one transaction, for asking its state.
    fn txn_body(id: i64) -> Vec<u8> {
        encode_body(&WireTxnBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            id,
        })
        .unwrap()
    }

    /// A commit with no policy overrides: the client's own tuning for both phases.
    fn txn_commit_body(id: i64) -> Vec<u8> {
        Worker::txn_commit_body_with(id, None, None)
    }

    /// A commit with overrides for either phase.
    fn txn_commit_body_with(
        id: i64,
        verify: Option<WirePolicy>,
        roll: Option<WirePolicy>,
    ) -> Vec<u8> {
        encode_body(&WireTxnCommitBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            id,
            verify,
            roll,
        })
        .unwrap()
    }

    /// An abort with no policy override.
    fn txn_abort_body(id: i64) -> Vec<u8> {
        Worker::txn_abort_body_with(id, None)
    }

    /// An abort with an override for its roll-back batch.
    fn txn_abort_body_with(id: i64, roll: Option<WirePolicy>) -> Vec<u8> {
        encode_body(&WireTxnAbortBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            id,
            roll,
        })
        .unwrap()
    }
}

#[test]
fn a_transaction_opens_reports_its_state_and_aborts() {
    require_cluster();
    let mut worker = Worker::attach();
    let id = worker.begin_txn();
    let body = Worker::txn_body(id);

    let (_, payload) = worker.must(opcode::TXN_STATE, &body);
    assert_eq!(
        decode_body::<WireTxnState>(&payload).unwrap(),
        WireTxnState::Open
    );

    // Two transactions are two transactions, and the id is the client's own — so
    // it may be negative, which a registry key invented by the daemon would not.
    assert_ne!(worker.begin_txn(), id);

    let (_, payload) = worker.must(opcode::TXN_ABORT, &Worker::txn_abort_body(id));
    assert_eq!(
        decode_body::<WireAbortStatus>(&payload).unwrap(),
        WireAbortStatus::Ok
    );

    // The daemon forgets a finished transaction, so asking again says so rather
    // than reporting a state for something it no longer holds.
    let (reply, payload) = worker.call(opcode::TXN_STATE, &body);
    assert_eq!(reply.status(), StatusCode::TXN_EXPIRED);
    let message = error_message(&payload);
    assert!(message.contains(&id.to_string()), "{message}");
    assert!(message.contains("start again"), "{message}");
}

/// A transaction belongs to the cluster it was opened on. Joining it from another
/// instance must fail rather than quietly running the command outside it.
#[test]
fn a_transaction_is_invisible_from_another_instance() {
    require_cluster();
    let mut worker = Worker::attach();
    let id = worker.begin_txn();

    let elsewhere = encode_body(&WireTxnBody {
        instance: "secondary".to_string(),
        id,
    })
    .unwrap();
    let (reply, _) = worker.call(opcode::TXN_STATE, &elsewhere);
    assert_eq!(reply.status(), StatusCode::TXN_EXPIRED);

    // And it is still open on its own instance, so the lookup failed rather than
    // the transaction being consumed.
    let (_, payload) = worker.must(opcode::TXN_STATE, &Worker::txn_body(id));
    assert_eq!(
        decode_body::<WireTxnState>(&payload).unwrap(),
        WireTxnState::Open
    );
    worker.must(opcode::TXN_ABORT, &Worker::txn_abort_body(id));
}

/// A command naming a transaction that is not open must fail, not run outside it.
/// This is the failure that would otherwise write data no transaction covers.
#[test]
fn a_command_cannot_join_a_transaction_that_is_not_open() {
    require_cluster();
    let mut worker = Worker::attach();
    let id = worker.begin_txn();
    worker.must(opcode::TXN_ABORT, &Worker::txn_abort_body(id));

    let body = encode_body(&PutBody {
        target: target_with(
            WireKey::Str("txn-late".into()),
            WirePolicy {
                txn: Some(id),
                ..WirePolicy::default()
            },
        ),
        bins: vec![("n".into(), WireValue::Int(1))],
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::PUT, &body);
    assert_eq!(reply.status(), StatusCode::TXN_EXPIRED);
    assert!(
        error_message(&payload).contains(&id.to_string()),
        "{}",
        error_message(&payload)
    );

    // An id that was never a transaction is the same answer.
    let never = encode_body(&PutBody {
        target: target_with(
            WireKey::Str("txn-late".into()),
            WirePolicy {
                txn: Some(0xDEAD),
                ..WirePolicy::default()
            },
        ),
        bins: vec![("n".into(), WireValue::Int(1))],
    })
    .unwrap();
    assert_eq!(
        worker.call(opcode::PUT, &never).0.status(),
        StatusCode::TXN_EXPIRED
    );
}

/// A traversal names no keys, so it cannot be transactional. Refused rather than
/// run outside the transaction the caller believed it was in.
#[test]
fn a_query_refuses_a_transaction() {
    require_cluster();
    let mut worker = Worker::attach();
    let id = worker.begin_txn();

    let body = encode_body(&WireQueryBody {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        policy: WirePolicy {
            txn: Some(id),
            ..WirePolicy::default()
        },
        statement: WireStatement {
            namespace: daemon().namespace.clone(),
            set: scanned_set().clone(),
            bins: BinSelector::None,
            filter: None,
        },
        partitions: WirePartitions::All,
        page_size: 10,
        max_records: 0,
        records_per_second: 0,
        include_bin_data: false,
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::QUERY, &body);
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    assert!(
        error_message(&payload).contains("named by key"),
        "{}",
        error_message(&payload)
    );

    worker.must(opcode::TXN_ABORT, &Worker::txn_abort_body(id));
}

/// The behaviour a transaction exists for: either both writes land or neither
/// does. Needs a strong-consistency namespace, so it self-skips without one.
#[test]
fn a_transaction_commits_together_and_rolls_back_together() {
    require_cluster();
    if !strong_consistency() {
        eprintln!(
            "SKIP: namespace '{}' is not configured with strong-consistency, which \
             multi-record transactions require. Set AEROSPIKE_SC_NAMESPACE to one that is.",
            sc_namespace()
        );
        return;
    }
    let mut worker = Worker::attach();
    let a = WireKey::Str("txn-a".into());
    let b = WireKey::Str("txn-b".into());

    // In the SC namespace, not the fixture's: these two halves need different
    // namespaces, for the reason `sc_namespace` explains.
    let sc_target = |key: WireKey, policy: WirePolicy| Target {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        namespace: sc_namespace(),
        set: daemon().set.clone(),
        key,
        policy,
    };
    let seed = |worker: &mut Worker, key: WireKey, balance: i64| {
        let body = encode_body(&PutBody {
            target: sc_target(key, WirePolicy::default()),
            bins: vec![("balance".into(), WireValue::Int(balance))],
        })
        .unwrap();
        worker.must(opcode::PUT, &body);
    };
    let in_txn = |key: WireKey, id: i64, balance: i64| {
        encode_body(&PutBody {
            target: sc_target(
                key,
                WirePolicy {
                    txn: Some(id),
                    ..WirePolicy::default()
                },
            ),
            bins: vec![("balance".into(), WireValue::Int(balance))],
        })
        .unwrap()
    };
    let read = |worker: &mut Worker, key: WireKey| -> WireValue {
        let body = encode_body(&GetBody {
            target: sc_target(key, WirePolicy::default()),
            bins: BinSelector::All,
        })
        .unwrap();
        let (_, payload) = worker.must(opcode::GET, &body);
        decode_body::<RecordBody>(&payload)
            .unwrap()
            .bins
            .into_iter()
            .find(|(name, _)| name == "balance")
            .map(|(_, value)| value)
            .expect("the balance bin must come back")
    };

    seed(&mut worker, a.clone(), 100);
    seed(&mut worker, b.clone(), 0);

    // ---- commit ----
    let id = worker.begin_txn();
    worker.must(opcode::PUT, &in_txn(a.clone(), id, 70));
    worker.must(opcode::PUT, &in_txn(b.clone(), id, 30));
    let (_, payload) = worker.must(opcode::TXN_COMMIT, &Worker::txn_commit_body(id));
    assert_eq!(
        decode_body::<WireCommitStatus>(&payload).unwrap(),
        WireCommitStatus::Ok
    );
    assert_eq!(read(&mut worker, a.clone()), WireValue::Int(70));
    assert_eq!(read(&mut worker, b.clone()), WireValue::Int(30));

    // ---- commit, with a policy for each of its two phases ----
    //
    // A commit is a verify batch and then a roll batch, and each takes its own
    // policy. Longer timeouts than the tuned defaults, which is the direction that
    // is safe to move them: a commit that gives up leaves locks held.
    let longer = WirePolicy {
        total_timeout_ms: Some(30_000),
        socket_timeout_ms: Some(5_000),
        max_retries: Some(8),
        ..WirePolicy::default()
    };
    let id = worker.begin_txn();
    worker.must(opcode::PUT, &in_txn(a.clone(), id, 55));
    worker.must(opcode::PUT, &in_txn(b.clone(), id, 45));
    let (_, payload) = worker.must(
        opcode::TXN_COMMIT,
        &Worker::txn_commit_body_with(id, Some(longer.clone()), Some(longer.clone())),
    );
    assert_eq!(
        decode_body::<WireCommitStatus>(&payload).unwrap(),
        WireCommitStatus::Ok
    );
    assert_eq!(read(&mut worker, a.clone()), WireValue::Int(55));
    assert_eq!(read(&mut worker, b.clone()), WireValue::Int(45));

    // A commit whose policies name a field neither phase can honour is refused
    // before anything is sent, and the transaction is still open afterwards — a
    // rejected policy must not consume the transaction.
    let id = worker.begin_txn();
    worker.must(opcode::PUT, &in_txn(a.clone(), id, -1));
    let with_ttl = WirePolicy {
        expiration: Some(aerospike_php_ipc::policy::WireExpiration::Seconds(60)),
        ..WirePolicy::default()
    };
    let (reply, payload) = worker.call(
        opcode::TXN_COMMIT,
        &Worker::txn_commit_body_with(id, None, Some(with_ttl)),
    );
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    let message = error_message(&payload);
    assert!(message.contains("'expiration'"), "{message}");
    // Still open, so the work can still be finished or thrown away deliberately.
    let (_, payload) = worker.must(opcode::TXN_STATE, &Worker::txn_body(id));
    assert_eq!(
        decode_body::<WireTxnState>(&payload).unwrap(),
        WireTxnState::Open
    );
    worker.must(opcode::TXN_ABORT, &Worker::txn_abort_body(id));
    assert_eq!(
        read(&mut worker, a.clone()),
        WireValue::Int(55),
        "the refused commit and the abort that followed must both leave the record alone"
    );

    // ---- abort, with a policy for its roll-back ----
    let id = worker.begin_txn();
    worker.must(opcode::PUT, &in_txn(a.clone(), id, -1000));
    let (_, payload) = worker.must(
        opcode::TXN_ABORT,
        &Worker::txn_abort_body_with(id, Some(longer)),
    );
    assert_eq!(
        decode_body::<WireAbortStatus>(&payload).unwrap(),
        WireAbortStatus::Ok
    );
    assert_eq!(
        read(&mut worker, a.clone()),
        WireValue::Int(55),
        "an aborted write must leave the record as the last commit left it"
    );

    // Durable deletes, because a strong-consistency namespace **forbids** the
    // ordinary kind — result code 22, `FailForbidden`. That rule is also why the
    // rest of this file cannot run against an SC namespace.
    let expunge = WirePolicy {
        durable_delete: Some(true),
        ..WirePolicy::default()
    };
    for key in [a, b] {
        let body = encode_body(&TargetBody {
            target: sc_target(key, expunge.clone()),
        })
        .unwrap();
        worker.must(opcode::DELETE, &body);
    }
}

// ===== Users, roles and privileges ==========================================

/// Whether the cluster has security enabled.
///
/// Every command in that family needs it, and a development cluster usually does not
/// — so the ones that touch it self-skip. What is *not* skipped is the refusal
/// itself: reaching result code 52 proves the whole path from the contract through
/// the daemon to the server works, which is most of what these tests are for.
fn security_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let mut worker = Worker::attach();
        let body = encode_body(&WireRoleQueryBody {
            target: admin_target(),
            role: None,
        })
        .unwrap();
        let (reply, _) = worker.call(opcode::ROLE_QUERY, &body);
        reply.status().is_ok()
    })
}

fn admin_target() -> WireAdminTarget {
    WireAdminTarget {
        instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
        timeout_ms: None,
    }
}

/// A cluster without security says so with its own result code, and the daemon
/// forwards it rather than inventing a diagnosis.
#[test]
fn security_commands_report_the_servers_own_refusal() {
    require_cluster();
    if security_enabled() {
        eprintln!("SKIP: this cluster has security enabled, so there is no refusal to check");
        return;
    }
    let mut worker = Worker::attach();
    let body = encode_body(&WireRoleQueryBody {
        target: admin_target(),
        role: None,
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::ROLE_QUERY, &body);

    assert_eq!(
        reply.status(),
        StatusCode::SERVER,
        "the server spoke, so this is its failure and not the daemon's: {}",
        error_message(&payload)
    );
    // 52 is SECURITY_NOT_ENABLED. Forwarding the code is what lets a caller tell
    // "not configured for this" from "you may not do this".
    assert_eq!(reply.result_code(), Some(52), "{}", error_message(&payload));
}

/// A privilege the server cannot act on is refused *before* it is sent, because the
/// server's own complaint names neither the privilege nor the reason.
#[test]
fn a_cluster_wide_privilege_with_a_namespace_never_reaches_the_server() {
    require_cluster();
    let mut worker = Worker::attach();
    let body = encode_body(&WireRoleCreateBody {
        target: admin_target(),
        role: "phpd_scope_check".into(),
        privileges: vec![WirePrivilege {
            code: WirePrivilegeCode::SysAdmin,
            namespace: Some(daemon().namespace.clone()),
            set_name: None,
        }],
        allowlist: Vec::new(),
        read_quota: 0,
        write_quota: 0,
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::ROLE_CREATE, &body);

    // INVALID_REQUEST, not SERVER: the daemon refused it, which is the point — and
    // it holds whether or not the cluster has security enabled, because the request
    // never gets that far.
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    let message = error_message(&payload);
    assert!(message.contains("whole cluster"), "{message}");
    assert!(message.contains("ReadWrite"), "{message}");
}

/// The empty-list rules, which differ from every other family here and are worth
/// pinning because both directions matter.
#[test]
fn a_role_needs_privileges_but_an_allowlist_may_be_empty() {
    require_cluster();
    let mut worker = Worker::attach();

    // A role with no privileges permits nothing, which nobody means.
    let empty = encode_body(&WireRoleCreateBody {
        target: admin_target(),
        role: "phpd_empty".into(),
        privileges: Vec::new(),
        allowlist: Vec::new(),
        read_quota: 0,
        write_quota: 0,
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::ROLE_CREATE, &empty);
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    assert!(
        error_message(&payload).contains("permits nothing"),
        "{}",
        error_message(&payload)
    );

    // Granting no roles is the same kind of mistake.
    let no_roles = encode_body(&WireUserRolesBody {
        target: admin_target(),
        user: "phpd_user".into(),
        roles: Vec::new(),
    })
    .unwrap();
    let (reply, payload) = worker.call(opcode::USER_GRANT_ROLES, &no_roles);
    assert_eq!(reply.status(), StatusCode::INVALID_REQUEST);
    assert!(
        error_message(&payload).contains("at least one role"),
        "{}",
        error_message(&payload)
    );

    // But an empty *allowlist* is how a restriction is cleared, so it must reach
    // the server — which then refuses it only because security is off.
    let cleared = encode_body(&WireRoleAllowlistBody {
        target: admin_target(),
        role: "phpd_empty".into(),
        allowlist: Vec::new(),
    })
    .unwrap();
    let (reply, _) = worker.call(opcode::ROLE_ALLOWLIST, &cleared);
    assert_ne!(
        reply.status(),
        StatusCode::INVALID_REQUEST,
        "an empty allowlist clears the restriction and must not be refused here"
    );
}

/// The whole lifecycle, on a cluster that can run it.
#[test]
fn a_user_and_a_role_are_created_read_back_and_dropped() {
    require_cluster();
    if !security_enabled() {
        eprintln!(
            "SKIP: this cluster does not have security enabled (result code 52), which every \
             user and role command requires"
        );
        return;
    }
    let mut worker = Worker::attach();
    let role = format!("phpd_role_{}", std::process::id());
    let user = format!("phpd_user_{}", std::process::id());

    // A previous run may have left them, so this does not depend on how it ended.
    let _ = worker.call(
        opcode::USER_DROP,
        &encode_body(&WireUserNameBody {
            target: admin_target(),
            user: user.clone(),
        })
        .unwrap(),
    );
    let drop_role = encode_body(&WireRoleNameBody {
        target: admin_target(),
        role: role.clone(),
    })
    .unwrap();
    let _ = worker.call(opcode::ROLE_DROP, &drop_role);

    worker.must(
        opcode::ROLE_CREATE,
        &encode_body(&WireRoleCreateBody {
            target: admin_target(),
            role: role.clone(),
            privileges: vec![WirePrivilege {
                code: WirePrivilegeCode::Read,
                namespace: Some(daemon().namespace.clone()),
                set_name: None,
            }],
            allowlist: Vec::new(),
            read_quota: 1_000,
            write_quota: 0,
        })
        .unwrap(),
    );

    let role_query = encode_body(&WireRoleQueryBody {
        target: admin_target(),
        role: Some(role.clone()),
    })
    .unwrap();
    // The quota is what settles last, so it is what the wait is keyed on.
    let created = eventually(
        &mut worker,
        &format!("role '{role}' with its read quota"),
        |worker| {
            let (reply, payload) = worker.call(opcode::ROLE_QUERY, &role_query);
            if !reply.status().is_ok() {
                return None;
            }
            let roles: WireRoles = decode_body(&payload).unwrap();
            let found = roles.roles.first()?;
            (roles.roles.len() == 1 && found.read_quota == 1_000).then(|| found.clone())
        },
    );
    assert_eq!(created.name, role);
    assert_eq!(created.privileges[0].code, WirePrivilegeCode::Read);
    assert_eq!(created.read_quota, 1_000);
    // Zero came back as zero, which is the server's "unlimited" — not an absence.
    assert_eq!(created.write_quota, 0);

    worker.must(
        opcode::USER_CREATE,
        &encode_body(&WireUserCreateBody {
            target: admin_target(),
            user: user.clone(),
            password: "phpd-secret".into(),
            roles: vec![role.clone()],
        })
        .unwrap(),
    );

    let user_query = encode_body(&WireUserQueryBody {
        target: admin_target(),
        user: Some(user.clone()),
    })
    .unwrap();
    // Its role list settles a moment after the user does, so that is the
    // condition — reading the user alone would race the grant.
    let created = eventually(
        &mut worker,
        &format!("user '{user}' holding role '{role}'"),
        |worker| {
            let (reply, payload) = worker.call(opcode::USER_QUERY, &user_query);
            if !reply.status().is_ok() {
                return None;
            }
            let users: WireUsers = decode_body(&payload).unwrap();
            let found = users.users.first()?;
            (users.users.len() == 1 && found.roles.contains(&role)).then(|| found.clone())
        },
    );
    assert_eq!(created.user, user);
    assert!(created.roles.contains(&role));

    worker.must(
        opcode::USER_DROP,
        &encode_body(&WireUserNameBody {
            target: admin_target(),
            user,
        })
        .unwrap(),
    );
    worker.must(opcode::ROLE_DROP, &drop_role);
}

/// What a handle for work that never existed actually reports — which differs by
/// kind, and is worth pinning because the difference is not obvious.
#[test]
fn a_task_that_was_never_started_reports_what_the_server_says() {
    let fixture = require_cluster();
    let mut worker = Worker::attach();
    let ask = |handle| {
        encode_body(&WireTaskStatusBody {
            instance: aerospike_php_ipc::DEFAULT_INSTANCE.to_string(),
            handle,
        })
        .unwrap()
    };

    // An index that was never created is a **failure**, not `NotFound`: the
    // server answers `sindex-stat` with `ERROR:201:no index`, and the client
    // turns that into a bad-response error. Reporting it as `NotFound` would
    // mean inventing a status the client never produced — and "no such index" is
    // more useful to a caller than "not found yet", which is what `NotFound`
    // means for a command that was issued.
    let (reply, payload) = worker.call(
        opcode::TASK_STATUS,
        &ask(WireTaskHandle::Index {
            namespace: fixture.namespace.clone(),
            index_name: "phpd_no_such_index".into(),
            dropping: false,
        }),
    );
    assert_eq!(reply.status(), StatusCode::INTERNAL);
    let message = error_message(&payload);
    assert!(message.contains("no index"), "{message}");

    // A background job id the server never knew reports **Complete**, and that
    // is not a bug to fix here: the server tracks *running* jobs, so "no node is
    // running it" is the only thing it can say, and the client reports that as
    // done. The consequence a caller has to live with: **a background job that
    // finished and one that never existed are indistinguishable.** Waiting on a
    // task from a command that succeeded is therefore safe; inventing a handle
    // and waiting on it always says "complete" immediately.
    let (_, payload) = worker.must(
        opcode::TASK_STATUS,
        &ask(WireTaskHandle::Execute {
            task_id: 0xDEAD_BEEF,
            scan: true,
        }),
    );
    assert_eq!(
        decode_body::<WireTaskStatus>(&payload).unwrap(),
        WireTaskStatus::Complete,
        "the server tracks running jobs, so an unknown id reads as finished"
    );
}
