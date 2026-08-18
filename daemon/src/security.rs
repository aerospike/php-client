// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Users, roles and privileges: the contract's shapes ⇄ the client's.
//!
//! The contract's side is [`aerospike_php_ipc::security`]. This is the smallest
//! module of the four command families, and deliberately so: none of these verbs
//! holds state on either side, so there is nothing here but conversion. Compare
//! [`crate::query`] (a cursor registry) and [`crate::txn`] (a registry whose expiry
//! aborts) — the difference is what the command *is*, not how much care went into
//! it.
//!
//! # Privileges are checked before they are sent
//!
//! The administrative privilege codes act on the cluster, so confining one to a
//! namespace is meaningless — and the server says so with a parameter error that
//! names neither the privilege nor the reason. [`to_privilege`] refuses it here
//! instead, which is the same bargain the rest of this daemon strikes: reject what
//! the server would reject obscurely, and say which field was wrong.

use std::fmt;

use aerospike_core::{Privilege, PrivilegeCode, Role, User};
use aerospike_php_ipc::security::{
    WirePrivilege, WirePrivilegeCode, WireRole, WireUser,
};

/// A privilege the daemon refuses to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadPrivilege(String);

impl BadPrivilege {
    /// The failure detail.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BadPrivilege {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BadPrivilege {}

/// The client's privilege code.
#[must_use]
pub const fn privilege_code(code: WirePrivilegeCode) -> PrivilegeCode {
    match code {
        WirePrivilegeCode::UserAdmin => PrivilegeCode::UserAdmin,
        WirePrivilegeCode::SysAdmin => PrivilegeCode::SysAdmin,
        WirePrivilegeCode::DataAdmin => PrivilegeCode::DataAdmin,
        WirePrivilegeCode::UdfAdmin => PrivilegeCode::UDFAdmin,
        WirePrivilegeCode::SIndexAdmin => PrivilegeCode::SIndexAdmin,
        WirePrivilegeCode::Read => PrivilegeCode::Read,
        WirePrivilegeCode::ReadWrite => PrivilegeCode::ReadWrite,
        WirePrivilegeCode::ReadWriteUdf => PrivilegeCode::ReadWriteUDF,
        WirePrivilegeCode::Write => PrivilegeCode::Write,
        WirePrivilegeCode::Truncate => PrivilegeCode::Truncate,
        WirePrivilegeCode::MaskingAdmin => PrivilegeCode::MaskingAdmin,
        WirePrivilegeCode::ReadMasked => PrivilegeCode::ReadMasked,
        WirePrivilegeCode::WriteMasked => PrivilegeCode::WriteMasked,
    }
}

/// The contract's privilege code.
///
/// The inbound direction, for a role read back from the server. A code this build
/// does not know cannot happen while the daemon and the client are one binary, but
/// the match is exhaustive so an upstream addition is a compile error here rather
/// than a silent mistranslation.
#[must_use]
pub const fn from_privilege_code(code: &PrivilegeCode) -> WirePrivilegeCode {
    match code {
        PrivilegeCode::UserAdmin => WirePrivilegeCode::UserAdmin,
        PrivilegeCode::SysAdmin => WirePrivilegeCode::SysAdmin,
        PrivilegeCode::DataAdmin => WirePrivilegeCode::DataAdmin,
        PrivilegeCode::UDFAdmin => WirePrivilegeCode::UdfAdmin,
        PrivilegeCode::SIndexAdmin => WirePrivilegeCode::SIndexAdmin,
        PrivilegeCode::Read => WirePrivilegeCode::Read,
        PrivilegeCode::ReadWrite => WirePrivilegeCode::ReadWrite,
        PrivilegeCode::ReadWriteUDF => WirePrivilegeCode::ReadWriteUdf,
        PrivilegeCode::Write => WirePrivilegeCode::Write,
        PrivilegeCode::Truncate => WirePrivilegeCode::Truncate,
        PrivilegeCode::MaskingAdmin => WirePrivilegeCode::MaskingAdmin,
        PrivilegeCode::ReadMasked => WirePrivilegeCode::ReadMasked,
        PrivilegeCode::WriteMasked => WirePrivilegeCode::WriteMasked,
    }
}

/// The client's privilege, checked.
///
/// # Errors
/// [`BadPrivilege`] for a global-only code given a namespace or set, and for a set
/// named without a namespace — a set scope is *within* a namespace, so a set alone
/// describes nothing the server can act on.
pub fn to_privilege(privilege: &WirePrivilege) -> Result<Privilege, BadPrivilege> {
    if privilege.is_scoped() && !privilege.is_scopable() {
        return Err(BadPrivilege(format!(
            "the {:?} privilege applies to the whole cluster and cannot be confined to a \
             namespace or set; drop the scope, or use one of the data privileges (Read, \
             ReadWrite, ReadWriteUdf, Write, Truncate, ReadMasked, WriteMasked)",
            privilege.code
        )));
    }
    if privilege.set_name.is_some() && privilege.namespace.is_none() {
        return Err(BadPrivilege(format!(
            "the {:?} privilege names a set but no namespace; a set scope is within a \
             namespace, so the namespace has to be named too",
            privilege.code
        )));
    }
    Ok(Privilege {
        code: privilege_code(privilege.code),
        namespace: privilege.namespace.clone(),
        set_name: privilege.set_name.clone(),
    })
}

/// The client's privileges, in order.
///
/// # Errors
/// [`BadPrivilege`] from the first one that will not build.
pub fn to_privileges(privileges: &[WirePrivilege]) -> Result<Vec<Privilege>, BadPrivilege> {
    privileges.iter().map(to_privilege).collect()
}

/// The contract's privilege.
///
/// An **empty** scope is no scope. The client fills both scope fields in for any
/// privilege whose code *can* be scoped, so one the server reports unscoped comes
/// back as `Some("")` rather than `None` — which PHP would then read back as
/// `READ on test.`, with a separator and nothing after it, or `READ on ` for a
/// privilege with no namespace at all.
#[must_use]
pub fn from_privilege(privilege: &Privilege) -> WirePrivilege {
    let scope = |value: &Option<String>| value.clone().filter(|name| !name.is_empty());
    WirePrivilege {
        code: from_privilege_code(&privilege.code),
        namespace: scope(&privilege.namespace),
        set_name: scope(&privilege.set_name),
    }
}

/// The contract's user.
#[must_use]
pub fn from_user(user: &User) -> WireUser {
    WireUser {
        user: user.user.clone(),
        roles: user.roles.clone(),
        read_info: user.read_info.clone(),
        write_info: user.write_info.clone(),
        conns_in_use: user.conns_in_use,
    }
}

/// The contract's role.
#[must_use]
pub fn from_role(role: &Role) -> WireRole {
    WireRole {
        name: role.name.clone(),
        privileges: role.privileges.iter().map(from_privilege).collect(),
        allowlist: role.allowlist.clone(),
        read_quota: role.read_quota,
        write_quota: role.write_quota,
    }
}

/// Borrow a list of owned names as the `&[&str]` the client's admin commands take.
///
/// Its own function because every one of those commands needs it and the borrow has
/// to outlive the call — writing it inline would mean a temporary that does not.
#[must_use]
pub fn as_str_slice(names: &[String]) -> Vec<&str> {
    names.iter().map(String::as_str).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scoped(code: WirePrivilegeCode) -> WirePrivilege {
        WirePrivilege {
            code,
            namespace: Some("test".into()),
            set_name: None,
        }
    }

    /// The check this module exists for: a cluster-wide privilege confined to a
    /// namespace is a parameter error from the server with nothing in it.
    #[test]
    fn a_global_privilege_cannot_be_confined_to_a_namespace() {
        for code in [
            WirePrivilegeCode::UserAdmin,
            WirePrivilegeCode::SysAdmin,
            WirePrivilegeCode::DataAdmin,
            WirePrivilegeCode::UdfAdmin,
            WirePrivilegeCode::SIndexAdmin,
            WirePrivilegeCode::MaskingAdmin,
        ] {
            let error = to_privilege(&scoped(code)).expect_err("must be refused");
            assert!(error.message().contains("whole cluster"), "{error}");
            // The message has to point at the alternative, or a caller is left
            // guessing which codes *can* be scoped.
            assert!(error.message().contains("ReadWrite"), "{error}");
        }

        // Unscoped, the same codes are fine.
        for code in [WirePrivilegeCode::UserAdmin, WirePrivilegeCode::SysAdmin] {
            assert!(to_privilege(&WirePrivilege {
                code,
                namespace: None,
                set_name: None,
            })
            .is_ok());
        }
    }

    /// A set is a scope *within* a namespace, so one without the other describes
    /// nothing.
    #[test]
    fn a_set_without_a_namespace_is_refused() {
        let error = to_privilege(&WirePrivilege {
            code: WirePrivilegeCode::Read,
            namespace: None,
            set_name: Some("users".into()),
        })
        .expect_err("must be refused");
        assert!(error.message().contains("within a"), "{error}");

        // With the namespace it is exactly what a set scope means.
        let ok = to_privilege(&WirePrivilege {
            code: WirePrivilegeCode::Read,
            namespace: Some("test".into()),
            set_name: Some("users".into()),
        })
        .unwrap();
        assert_eq!(ok.namespace.as_deref(), Some("test"));
        assert_eq!(ok.set_name.as_deref(), Some("users"));
    }

    /// What the server actually sends back for a namespace-wide privilege: the
    /// set is present and empty. It must not reach PHP as a scope.
    #[test]
    fn an_empty_scope_comes_back_as_no_scope() {
        let wire = from_privilege(&Privilege {
            code: PrivilegeCode::Read,
            namespace: Some("test".into()),
            set_name: Some(String::new()),
        });
        assert_eq!(wire.namespace.as_deref(), Some("test"));
        assert_eq!(wire.set_name, None, "an empty set is not a set scope");

        // And a privilege the server reports with no scope at all.
        let wire = from_privilege(&Privilege {
            code: PrivilegeCode::Read,
            namespace: Some(String::new()),
            set_name: Some(String::new()),
        });
        assert_eq!(wire.namespace, None);
        assert_eq!(wire.set_name, None);
    }

    #[test]
    fn every_privilege_code_maps_both_ways() {
        for code in [
            WirePrivilegeCode::UserAdmin,
            WirePrivilegeCode::SysAdmin,
            WirePrivilegeCode::DataAdmin,
            WirePrivilegeCode::UdfAdmin,
            WirePrivilegeCode::SIndexAdmin,
            WirePrivilegeCode::Read,
            WirePrivilegeCode::ReadWrite,
            WirePrivilegeCode::ReadWriteUdf,
            WirePrivilegeCode::Write,
            WirePrivilegeCode::Truncate,
            WirePrivilegeCode::MaskingAdmin,
            WirePrivilegeCode::ReadMasked,
            WirePrivilegeCode::WriteMasked,
        ] {
            // A round trip through the client's type, which is what a role read
              // back from the server goes through.
            assert_eq!(
                from_privilege_code(&privilege_code(code)),
                code,
                "{code:?} did not survive the round trip"
            );
        }
    }

    #[test]
    fn a_list_of_privileges_fails_on_the_first_bad_one() {
        let privileges = vec![
            WirePrivilege {
                code: WirePrivilegeCode::Read,
                namespace: Some("test".into()),
                set_name: None,
            },
            scoped(WirePrivilegeCode::UserAdmin),
        ];
        assert!(to_privileges(&privileges).is_err());
        assert_eq!(to_privileges(&privileges[..1]).unwrap().len(), 1);
        assert!(to_privileges(&[]).unwrap().is_empty());
    }

    #[test]
    fn a_user_and_a_role_convert_to_the_contracts_shapes() {
        let user = User {
            user: "alice".into(),
            roles: vec!["read-write".into()],
            read_info: vec![100, 5],
            write_info: Vec::new(),
            conns_in_use: 2,
        };
        let wire = from_user(&user);
        assert_eq!(wire.user, "alice");
        assert_eq!(wire.roles, vec!["read-write".to_string()]);
        // The statistic lists come across as the server sent them, however long.
        assert_eq!(wire.read_info, vec![100, 5]);
        assert!(wire.write_info.is_empty());
        assert_eq!(wire.conns_in_use, 2);

        let role = Role {
            name: "auditor".into(),
            privileges: vec![Privilege {
                code: PrivilegeCode::Read,
                namespace: Some("test".into()),
                set_name: None,
            }],
            allowlist: vec!["10.0.0.0/8".into()],
            read_quota: 1_000,
            write_quota: 0,
        };
        let wire = from_role(&role);
        assert_eq!(wire.name, "auditor");
        assert_eq!(wire.privileges[0].code, WirePrivilegeCode::Read);
        assert_eq!(wire.privileges[0].namespace.as_deref(), Some("test"));
        assert_eq!(wire.allowlist, vec!["10.0.0.0/8".to_string()]);
        assert_eq!(wire.read_quota, 1_000);
        // Zero is "unlimited", and it has to survive as zero rather than being
        // treated as unset.
        assert_eq!(wire.write_quota, 0);
    }

    #[test]
    fn owned_names_borrow_as_the_clients_slice() {
        let names = vec!["read".to_string(), "write".to_string()];
        assert_eq!(as_str_slice(&names), vec!["read", "write"]);
        assert!(as_str_slice(&[]).is_empty());
    }
}
