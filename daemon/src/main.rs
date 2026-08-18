// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The daemon executable: read the configuration, connect the clusters, serve
//! until asked to stop, then close cleanly.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;

use aerospike_php_daemon::config::{Config, DEFAULT_CONFIG_PATH};
use aerospike_php_daemon::dispatch::Dispatcher;
use aerospike_php_daemon::instance::Instances;
use aerospike_php_daemon::server::{Server, Shutdown};

/// Exit code for a configuration or startup failure, as distinct from a crash.
const EXIT_STARTUP_FAILURE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "aerospike-php-daemon",
    version,
    about = "Serves Aerospike operations to local PHP workers over shared memory"
)]
struct Cli {
    /// Path to the configuration file.
    #[arg(short, long, default_value = DEFAULT_CONFIG_PATH, value_name = "PATH")]
    config: PathBuf,

    /// Validate the configuration and exit without connecting to anything.
    #[arg(long)]
    check: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Logging is configured from the file, so the file has to be read first;
    // until then, complain on stderr.
    let config = match Config::load(&cli.config) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("aerospike-php-daemon: {e}");
            return ExitCode::from(EXIT_STARTUP_FAILURE);
        }
    };

    init_logging(&config.daemon.log_level);

    // Exactly one line, naming each key that was accepted and is not applied,
    // so nobody has to guess whether it took effect.
    if !config.ignored.is_empty() {
        log::info!(
            "these configuration keys are accepted but not implemented, and have no effect: {}",
            config.ignored.join(", ")
        );
    }

    if cli.check {
        println!(
            "{}: ok — instance '{}', clusters: {}",
            cli.config.display(),
            config.daemon.instance,
            config.cluster_names().join(", ")
        );
        return ExitCode::SUCCESS;
    }

    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all().thread_name("aerospike-daemon");
    if let Some(threads) = config.daemon.worker_threads {
        builder.worker_threads(threads.max(1));
    }
    let runtime = match builder.build() {
        Ok(runtime) => runtime,
        Err(e) => {
            log::error!("cannot start the async runtime: {e}");
            return ExitCode::from(EXIT_STARTUP_FAILURE);
        }
    };

    // Clients are built eagerly, and a cluster that cannot be reached does not
    // stop the others from being served.
    let instances = Arc::new(runtime.block_on(Instances::connect(&config)));
    if instances.ready_count() == 0 {
        log::error!(
            "no configured cluster could be reached; serving anyway — PING works and \
             every operation will report CONNECTION until a cluster comes back"
        );
    }

    let dispatcher = Arc::new(
        Dispatcher::new(
            instances.clone(),
            config.daemon.default_timeout.as_duration(),
        )
        .with_paging(
            config.daemon.page_size,
            config.daemon.cursor_idle_timeout.as_duration(),
            config.daemon.max_cursors,
        )
        .with_transactions(
            config.daemon.txn_idle_timeout.as_duration(),
            config.daemon.max_transactions,
        ),
    );

    let shutdown = Shutdown::new();
    spawn_signal_handler(&runtime, shutdown.clone());
    spawn_cursor_reaper(&runtime, dispatcher.clone());
    spawn_transaction_reaper(&runtime, dispatcher.clone());

    // The port is not `Send`, so it is created on the thread that will drive
    // it; binding there also means a name clash is reported before anything
    // starts depending on this daemon.
    let instance = config.daemon.instance.clone();
    let tuning = config.daemon.tuning();
    let handle = runtime.handle().clone();
    let stop = shutdown.clone();
    let serving = std::thread::Builder::new()
        .name("aerospike-ipc".to_string())
        .spawn(move || -> Result<(), String> {
            let mut server = Server::bind(&instance, dispatcher, handle, tuning)
                .map_err(|e| format!("cannot serve instance '{instance}': {e}"))?;
            server.run_until(&stop).map_err(|e| e.to_string())
        });

    let serving = match serving {
        Ok(handle) => handle,
        Err(e) => {
            log::error!("cannot start the IPC thread: {e}");
            return ExitCode::from(EXIT_STARTUP_FAILURE);
        }
    };

    let outcome = match serving.join() {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(e)) => {
            log::error!("{e}");
            ExitCode::from(EXIT_STARTUP_FAILURE)
        }
        Err(_) => {
            log::error!("the IPC thread panicked");
            ExitCode::FAILURE
        }
    };

    runtime.block_on(instances.close());
    outcome
}

/// `RUST_LOG` wins over the file, so a running daemon can be debugged without
/// editing its configuration.
fn init_logging(level: &str) {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();
}

/// Reclaim scan and query cursors nobody is going to ask for again.
///
/// A PHP worker that is killed mid-scan never sends `QUERY_CLOSE`, so its cursor
/// has to expire instead. It runs on a timer rather than on access precisely
/// because the abandoned cursors are the ones nothing will touch: checking at
/// use time would reclaim every cursor except the ones that need it.
///
/// The tick is a fraction of the idle timeout, so a cursor is reclaimed within
/// about a quarter of it — soon enough to matter, rarely enough to cost nothing
/// on an idle daemon.
fn spawn_cursor_reaper(runtime: &tokio::runtime::Runtime, dispatcher: Arc<Dispatcher>) {
    let tick = (dispatcher.cursors().idle_timeout() / 4).max(std::time::Duration::from_secs(1));
    runtime.spawn(async move {
        let mut interval = tokio::time::interval(tick);
        loop {
            interval.tick().await;
            let expired = dispatcher.cursors().expire();
            if expired > 0 {
                log::debug!(
                    "expired {expired} idle scan cursor(s); {} still open",
                    dispatcher.cursors().open_count()
                );
            }
        }
    });
}

/// Abort multi-record transactions nobody is going to finish.
///
/// Stronger than the cursor reaper, and it has to be: an abandoned transaction
/// holds record locks on the server, so every other writer of those records waits
/// behind it. This does not merely forget an idle one — it rolls it back.
///
/// The server's own MRT timeout is the backstop if the daemon dies too, but
/// "eventually" is the wrong answer for a lock, so the tick is a fraction of the
/// idle timeout.
fn spawn_transaction_reaper(runtime: &tokio::runtime::Runtime, dispatcher: Arc<Dispatcher>) {
    let tick =
        (dispatcher.transactions().idle_timeout() / 4).max(std::time::Duration::from_secs(1));
    runtime.spawn(async move {
        let mut interval = tokio::time::interval(tick);
        loop {
            interval.tick().await;
            let aborted = dispatcher
                .transactions()
                .expire(dispatcher.instances())
                .await;
            if aborted > 0 {
                log::info!(
                    "aborted {aborted} idle transaction(s); {} still open",
                    dispatcher.transactions().open_count()
                );
            }
        }
    });
}

/// Raise the stop flag on SIGINT or SIGTERM. A second signal is left to the
/// default disposition, so an operator can always insist.
fn spawn_signal_handler(runtime: &tokio::runtime::Runtime, shutdown: Shutdown) {
    runtime.spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut terminate = match signal(SignalKind::terminate()) {
                Ok(stream) => stream,
                Err(e) => {
                    log::warn!("cannot listen for SIGTERM: {e}");
                    return;
                }
            };
            tokio::select! {
                _ = tokio::signal::ctrl_c() => log::info!("received SIGINT"),
                _ = terminate.recv() => log::info!("received SIGTERM"),
            }
        }
        #[cfg(not(unix))]
        {
            if tokio::signal::ctrl_c().await.is_err() {
                return;
            }
            log::info!("received an interrupt");
        }
        shutdown.signal();
    });
}
