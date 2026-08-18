// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! The configured clusters, and the one client per cluster the daemon owns.
//!
//! Clients are built eagerly at startup: a PHP request must never pay for
//! bringing a cluster up, and a misconfigured cluster should be visible in the
//! startup log rather than on the thousandth request.
//!
//! **A bad instance must not take the daemon down.** One unreachable cluster
//! among five leaves the other four serving; requests naming the broken one
//! get [`StatusCode::CONNECTION`](aerospike_php_ipc::StatusCode::CONNECTION)
//! with the connect failure as their message, and it stays listed by `PING` —
//! it is configured, just not usable.

use std::collections::HashMap;
use std::sync::Arc;

use aerospike_core::Client;
use aerospike_php_ipc::DEFAULT_INSTANCE;

use crate::config::Config;

/// One configured cluster.
enum Instance {
    /// Connected and serving.
    Ready(Arc<Client>),
    /// Configured but unusable, with the reason.
    Unavailable(String),
}

/// The outcome of naming an instance in a request.
pub enum Lookup<'a> {
    /// Use this client.
    Ready(&'a Arc<Client>),
    /// The instance exists but could not be brought up.
    Unavailable(&'a str),
    /// Nothing is configured under that name.
    Unknown,
}

/// Every configured cluster, by name.
pub struct Instances {
    order: Vec<String>,
    map: HashMap<String, Instance>,
}

impl Instances {
    /// Connect every configured cluster, reporting rather than propagating
    /// per-instance failures.
    pub async fn connect(config: &Config) -> Instances {
        let mut order = Vec::with_capacity(config.clusters.len());
        let mut map = HashMap::with_capacity(config.clusters.len());

        for (name, cluster) in &config.clusters {
            order.push(name.clone());
            map.insert(name.clone(), connect_one(name, cluster).await);
        }

        Instances { order, map }
    }

    /// Build a registry directly, for tests.
    #[must_use]
    pub fn from_clients(clients: Vec<(String, Arc<Client>)>) -> Instances {
        let mut order = Vec::with_capacity(clients.len());
        let mut map = HashMap::with_capacity(clients.len());
        for (name, client) in clients {
            order.push(name.clone());
            map.insert(name, Instance::Ready(client));
        }
        Instances { order, map }
    }

    /// Configured instance names, in serving order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.order.clone()
    }

    /// How many instances are actually usable.
    #[must_use]
    pub fn ready_count(&self) -> usize {
        self.map
            .values()
            .filter(|i| matches!(i, Instance::Ready(_)))
            .count()
    }

    /// Resolve the instance a request named. An empty name means
    /// [`DEFAULT_INSTANCE`], matching the contract's default.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Lookup<'_> {
        let name = if name.is_empty() { DEFAULT_INSTANCE } else { name };
        match self.map.get(name) {
            Some(Instance::Ready(client)) => Lookup::Ready(client),
            Some(Instance::Unavailable(reason)) => Lookup::Unavailable(reason),
            None => Lookup::Unknown,
        }
    }

    /// Close every connected cluster. Errors are logged, never propagated:
    /// this runs on the way out and there is nothing left to abort.
    pub async fn close(&self) {
        for name in &self.order {
            if let Some(Instance::Ready(client)) = self.map.get(name) {
                match client.close().await {
                    Ok(()) => log::info!("instance '{name}': closed"),
                    Err(e) => log::warn!("instance '{name}': close failed: {e}"),
                }
            }
        }
    }
}

async fn connect_one(name: &str, cluster: &crate::config::ClusterConfig) -> Instance {
    // Both of these are re-validated at load time, so a failure here is a
    // logic error rather than a user error; handle it as an unusable instance
    // anyway instead of unwrapping.
    let policy = match cluster.to_client_policy() {
        Ok(policy) => policy,
        Err(e) => {
            log::error!("instance '{name}': invalid configuration: {e}");
            return Instance::Unavailable(e.to_string());
        }
    };
    let seeds = match cluster.seeds() {
        Ok(seeds) => seeds,
        Err(e) => {
            log::error!("instance '{name}': {e}");
            return Instance::Unavailable(e.to_string());
        }
    };

    match Client::new(&policy, &seeds).await {
        Ok(client) => {
            log::info!("instance '{name}': connected to {seeds}");
            Instance::Ready(Arc::new(client))
        }
        Err(e) => {
            log::error!(
                "instance '{name}': cannot connect to {seeds}: {e} \
                 (the daemon keeps serving; requests for this instance will fail)"
            );
            Instance::Unavailable(e.to_string())
        }
    }
}
