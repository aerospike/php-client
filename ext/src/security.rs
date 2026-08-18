// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

// `Privilege`'s parameters are camelCase because PHP named arguments use the
// parameter name exactly as written; see `crate::policy` for why the lint cannot be
// scoped more tightly than the module.
#![allow(non_snake_case)]

//! `Aerospike\User`, `Aerospike\Role` and `Aerospike\Privilege`.
//!
//! The three things the security commands take and return. [`Privilege`] is the only
//! one a caller builds; [`User`] and [`Role`] are answers, so neither has a
//! constructor.
//!
//! # Every command here needs security enabled
//!
//! On a cluster without `security { enable-security true }` they all fail with
//! result code **52**, `SecurityNotEnabled`. That is not gated client-side: it is a
//! configuration the operator chose, the server names it exactly, and a probe on
//! every call would cost a round trip to say what the failure already says.

use aerospike_php_ipc::security::{WirePrivilege, WireRole, WireUser};
use ext_php_rs::prelude::*;

use crate::arg::Given;
use crate::enums::{Case, PrivilegeCode};
use crate::error::{AeroError, AeroResult};

/// One privilege: what may be done, and where.
///
/// ```php
/// use Aerospike\{Privilege, PrivilegeCode};
///
/// new Privilege(PrivilegeCode::ReadWrite);                        // every namespace
/// new Privilege(PrivilegeCode::Read, 'test');                     // one namespace
/// new Privilege(PrivilegeCode::Read, 'test', 'users');            // one set
/// new Privilege(PrivilegeCode::SysAdmin);                         // cluster-wide
/// ```
///
/// **The administrative codes cannot be confined to a namespace.** `UserAdmin`,
/// `SysAdmin`, `DataAdmin`, `UdfAdmin`, `SIndexAdmin` and `MaskingAdmin` act on the
/// cluster, so a namespace means nothing for them — and the server refuses the
/// combination with a parameter error that names neither the privilege nor the
/// reason. This constructor refuses it instead, and says which codes *can* be
/// scoped.
#[php_class]
#[php(name = "Aerospike\\Privilege")]
#[derive(Debug, Clone)]
pub struct Privilege {
    privilege: WirePrivilege,
}

#[php_impl]
impl Privilege {
    /// Name a privilege, optionally confined to a namespace and a set.
    ///
    /// A set without a namespace is refused: a set scope is *within* a namespace,
    /// so a set alone describes nothing the server can act on.
    #[php(defaults(namespace = None, setName = None))]
    pub fn __construct(
        code: Given<PrivilegeCode>,
        namespace: Option<Given<String>>,
        setName: Option<Given<String>>,
    ) -> PhpResult<Privilege> {
        Ok(Privilege::checked(
            code.required("code")?,
            Given::or_none(namespace, "namespace")?,
            Given::or_none(setName, "setName")?,
        )?)
    }

    /// What is permitted.
    pub fn code(&self) -> Case<PrivilegeCode> {
        Case(PrivilegeCode::of(self.privilege.code))
    }

    /// The namespace this is confined to, or `null` for every namespace.
    pub fn namespace(&self) -> Option<String> {
        self.privilege.namespace.clone()
    }

    /// The set this is confined to, or `null` for every set.
    pub fn set_name(&self) -> Option<String> {
        self.privilege.set_name.clone()
    }

    /// Whether this privilege names a namespace or a set.
    pub fn is_scoped(&self) -> bool {
        self.privilege.is_scoped()
    }

    /// A readable form, for logs and test failures.
    pub fn __to_string(&self) -> String {
        match (&self.privilege.namespace, &self.privilege.set_name) {
            (Some(namespace), Some(set)) => format!(
                "{} on {namespace}.{set}",
                PrivilegeCode::of(self.privilege.code).label()
            ),
            (Some(namespace), None) => format!(
                "{} on {namespace}",
                PrivilegeCode::of(self.privilege.code).label()
            ),
            _ => PrivilegeCode::of(self.privilege.code).label().to_owned(),
        }
    }
}

impl Privilege {
    /// [`Privilege::__construct`] without the PHP wrapping, so the scope rules are
    /// testable without a running PHP.
    ///
    /// # Errors
    /// `AeroError` for a cluster-wide code given a scope, and for a set named
    /// without a namespace.
    pub fn checked(
        code: PrivilegeCode,
        namespace: Option<String>,
        set_name: Option<String>,
    ) -> AeroResult<Privilege> {
        let privilege = WirePrivilege {
            code: code.to_wire(),
            namespace,
            set_name,
        };
        if privilege.is_scoped() && !privilege.is_scopable() {
            return Err(AeroError::client(format!(
                "the {} privilege applies to the whole cluster and cannot be confined to a \
                 namespace or set. The ones that can are Read, ReadWrite, ReadWriteUdf, Write, \
                 Truncate, ReadMasked and WriteMasked",
                code.label()
            )));
        }
        if privilege.set_name.is_some() && privilege.namespace.is_none() {
            return Err(AeroError::client(format!(
                "the {} privilege names a set but no namespace; a set scope is within a \
                 namespace, so name the namespace too",
                code.label()
            )));
        }
        Ok(Privilege { privilege })
    }

    /// The contract's privilege.
    #[must_use]
    pub fn to_wire(&self) -> WirePrivilege {
        self.privilege.clone()
    }

    /// Build one from the contract's, for a role read back from the server.
    #[must_use]
    pub const fn from_wire(privilege: WirePrivilege) -> Privilege {
        Privilege { privilege }
    }
}

/// One user the cluster knows.
///
/// Produced by `queryUsers()`. There is no constructor: `createUser()` makes a user,
/// and one built here would describe nobody.
#[php_class]
#[php(name = "Aerospike\\User")]
#[derive(Debug, Clone)]
pub struct User {
    user: WireUser,
}

#[php_impl]
impl User {
    /// The user name.
    pub fn name(&self) -> String {
        self.user.user.clone()
    }

    /// The roles assigned to them.
    pub fn roles(&self) -> Vec<String> {
        self.user.roles.clone()
    }

    /// Whether they hold this role.
    pub fn has_role(&self, role: &str) -> bool {
        self.user.roles.iter().any(|held| held == role)
    }

    /// Read statistics, in the server's order: the quota in records per second, the
    /// single-record rate, the scan/query record rate, and the number of limitless
    /// read scans and queries.
    ///
    /// A list rather than named accessors because it is the *server's* list and a
    /// future release may append to it. May be empty, when the server reported none.
    pub fn read_info(&self) -> Vec<i64> {
        self.user.read_info.iter().map(|n| i64::from(*n)).collect()
    }

    /// Write statistics, in the same shape as `User::readInfo()`.
    pub fn write_info(&self) -> Vec<i64> {
        self.user.write_info.iter().map(|n| i64::from(*n)).collect()
    }

    /// Connections this user currently holds open.
    pub fn conns_in_use(&self) -> i64 {
        i64::from(self.user.conns_in_use)
    }

    /// `name (role, role)`, for logs.
    pub fn __to_string(&self) -> String {
        format!("{} ({})", self.user.user, self.user.roles.join(", "))
    }
}

impl User {
    /// Build one from the contract's.
    #[must_use]
    pub const fn from_wire(user: WireUser) -> User {
        User { user }
    }
}

/// One role the cluster knows.
///
/// Produced by `queryRoles()`; `createRole()` makes one.
#[php_class]
#[php(name = "Aerospike\\Role")]
#[derive(Debug, Clone)]
pub struct Role {
    role: WireRole,
}

#[php_impl]
impl Role {
    /// The role name.
    pub fn name(&self) -> String {
        self.role.name.clone()
    }

    /// What the role permits.
    pub fn privileges(&self) -> Vec<Privilege> {
        self.role
            .privileges
            .iter()
            .cloned()
            .map(Privilege::from_wire)
            .collect()
    }

    /// Addresses a holder may connect from. Empty means anywhere.
    pub fn allowlist(&self) -> Vec<String> {
        self.role.allowlist.clone()
    }

    /// Reads per second the role is limited to, or `null` when unlimited.
    ///
    /// `null` rather than `0`: zero is the server's way of saying "no limit", and a
    /// caller doing arithmetic on it would get a limit of nothing.
    pub fn read_quota(&self) -> Option<i64> {
        (self.role.read_quota != 0).then(|| i64::from(self.role.read_quota))
    }

    /// Writes per second the role is limited to, or `null` when unlimited.
    pub fn write_quota(&self) -> Option<i64> {
        (self.role.write_quota != 0).then(|| i64::from(self.role.write_quota))
    }

    /// `name: privilege, privilege`, for logs.
    pub fn __to_string(&self) -> String {
        let privileges: Vec<String> = self
            .role
            .privileges
            .iter()
            .cloned()
            .map(|privilege| Privilege::from_wire(privilege).__to_string())
            .collect();
        format!("{}: {}", self.role.name, privileges.join(", "))
    }
}

impl Role {
    /// Build one from the contract's.
    #[must_use]
    pub const fn from_wire(role: WireRole) -> Role {
        Role { role }
    }
}

/// Read a `Privilege[]` argument, checking every element.
///
/// As with `$bins` and `$ops`: PHP can type the parameter as `array` but not as an
/// array *of privileges*, so this is where that half is enforced — by position,
/// because the position is what a caller needs to find the wrong one.
///
/// # Errors
/// A PHP `TypeError` for an element that is not an `Aerospike\Privilege`, and a
/// client failure for an empty list: a role with no privileges permits nothing, and
/// granting none is a request the server cannot act on.
pub fn privilege_list(privileges: &[&ext_php_rs::types::Zval]) -> PhpResult<Vec<WirePrivilege>> {
    if privileges.is_empty() {
        return Err(AeroError::client(
            "at least one Aerospike\\Privilege is needed; a role with none permits nothing",
        )
        .into());
    }
    let mut list = Vec::with_capacity(privileges.len());
    for (index, zval) in privileges.iter().enumerate() {
        let privilege = zval.extract::<&Privilege>().ok_or_else(|| {
            crate::arg::type_error(format!(
                "$privileges must be a list of Aerospike\\Privilege; item {index} is {}",
                crate::arg::type_of(zval)
            ))
        })?;
        list.push(privilege.to_wire());
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aerospike_php_ipc::security::WirePrivilegeCode;

    /// The rule this class exists to enforce: a cluster-wide privilege confined to
    /// a namespace is a parameter error from the server with nothing in it.
    #[test]
    fn a_cluster_wide_privilege_cannot_be_scoped() {
        for code in [
            PrivilegeCode::UserAdmin,
            PrivilegeCode::SysAdmin,
            PrivilegeCode::DataAdmin,
            PrivilegeCode::UdfAdmin,
            PrivilegeCode::SIndexAdmin,
            PrivilegeCode::MaskingAdmin,
        ] {
            let error = Privilege::checked(code, Some("test".into()), None)
                .expect_err("a scoped cluster privilege must be refused");
            assert!(error.to_string().contains("whole cluster"), "{error}");
            // The message must point at the codes that *can* be scoped, or a
            // caller is left guessing.
            assert!(error.to_string().contains("ReadWrite"), "{error}");

            // Unscoped, the same code is exactly right.
            assert!(Privilege::checked(code, None, None).is_ok());
        }
    }

    #[test]
    fn a_set_without_a_namespace_is_refused() {
        let error = Privilege::checked(PrivilegeCode::Read, None, Some("users".into()))
            .expect_err("a set with no namespace must be refused");
        assert!(error.to_string().contains("within a namespace"), "{error}");

        let ok = Privilege::checked(
            PrivilegeCode::Read,
            Some("test".into()),
            Some("users".into()),
        )
        .unwrap();
        assert!(ok.is_scoped());
        assert_eq!(ok.to_wire().namespace.as_deref(), Some("test"));
        assert_eq!(ok.to_wire().set_name.as_deref(), Some("users"));
    }

    #[test]
    fn a_privilege_describes_itself_at_every_scope() {
        assert_eq!(
            Privilege::checked(PrivilegeCode::SysAdmin, None, None)
                .unwrap()
                .__to_string(),
            "SYS_ADMIN"
        );
        assert_eq!(
            Privilege::checked(PrivilegeCode::Read, Some("test".into()), None)
                .unwrap()
                .__to_string(),
            "READ on test"
        );
        assert_eq!(
            Privilege::checked(
                PrivilegeCode::Read,
                Some("test".into()),
                Some("users".into())
            )
            .unwrap()
            .__to_string(),
            "READ on test.users"
        );
    }

    #[test]
    fn a_user_reports_what_the_server_said() {
        let user = User::from_wire(WireUser {
            user: "alice".into(),
            roles: vec!["read-write".into(), "sys-admin".into()],
            read_info: vec![100, 5, 0, 2],
            write_info: Vec::new(),
            conns_in_use: 3,
        });
        assert_eq!(user.name(), "alice");
        assert!(user.has_role("sys-admin"));
        assert!(!user.has_role("user-admin"));
        assert_eq!(user.read_info(), vec![100, 5, 0, 2]);
        // Empty rather than invented: the server reported no write statistics.
        assert!(user.write_info().is_empty());
        assert_eq!(user.conns_in_use(), 3);
        assert_eq!(user.__to_string(), "alice (read-write, sys-admin)");
    }

    /// Zero is the server's "no limit". Reporting it as `0` would invite arithmetic
    /// that concluded the opposite.
    #[test]
    fn an_unlimited_quota_is_null_and_not_zero() {
        let role = Role::from_wire(WireRole {
            name: "auditor".into(),
            privileges: vec![WirePrivilege {
                code: WirePrivilegeCode::Read,
                namespace: Some("test".into()),
                set_name: None,
            }],
            allowlist: vec!["10.0.0.0/8".into()],
            read_quota: 1_000,
            write_quota: 0,
        });
        assert_eq!(role.name(), "auditor");
        assert_eq!(role.read_quota(), Some(1_000));
        assert_eq!(role.write_quota(), None, "zero means unlimited");
        assert_eq!(role.allowlist(), vec!["10.0.0.0/8".to_string()]);
        assert_eq!(role.privileges().len(), 1);
        assert_eq!(role.__to_string(), "auditor: READ on test");
    }

    #[test]
    fn a_role_with_no_allowlist_may_be_connected_to_from_anywhere() {
        let role = Role::from_wire(WireRole {
            name: "open".into(),
            privileges: Vec::new(),
            allowlist: Vec::new(),
            read_quota: 0,
            write_quota: 0,
        });
        assert!(role.allowlist().is_empty());
        assert_eq!(role.read_quota(), None);
        assert_eq!(role.write_quota(), None);
    }
}
