// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

//! Users, roles and privileges.
//!
//! Fourteen verbs, and unlike everything else in this contract **none of them
//! carries state on either side**: each is one admin exchange with one node,
//! answering with nothing, a list of users, or a list of roles. So there is no
//! registry, no handle and no expiry here — the whole family is request in, answer
//! out.
//!
//! # These need security enabled, which is not a version gate
//!
//! Every command here fails on a cluster without `security { enable-security true }`
//! — result code **52**, `SecurityNotEnabled`. That is deliberately *not* gated by
//! the daemon the way [`crate::txn`]'s version requirement is: it is a
//! configuration the operator chose, the server's own code names it exactly, and a
//! probe on every call would cost a round trip to say what the failure already
//! says.
//!
//! # Passwords cross shared memory in the clear
//!
//! [`WireUserCreateBody::password`] is the password as PHP supplied it. It travels
//! through shared memory to the daemon, which sends it to the server, hashed by the
//! server under `INTERNAL` auth.
//!
//! Shared memory is local to the host and readable only by the user the daemon and
//! the workers run as, so this is no weaker than the PHP process holding the
//! password in the first place — but it is worth stating, because the alternative
//! (hashing on the PHP side) is not available: the server decides the hash, and
//! `aerospike-core`'s admin commands take the plaintext.

use serde::{Deserialize, Serialize};

/// A default privilege the server defines.
///
/// The discriminants are the server's own, which is why they are not contiguous:
/// the administrative privileges are 0–4 and the data ones start at 10. That gap is
/// load-bearing — see [`WirePrivilege::is_scopable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WirePrivilegeCode {
    /// Edit and remove other users. Global scope only.
    UserAdmin,
    /// Systems administration that is not user administration — server
    /// configuration, for instance. Global scope only.
    SysAdmin,
    /// UDF and secondary-index administration. Global scope only.
    DataAdmin,
    /// UDF administration alone. Global scope only; needs server 6+.
    UdfAdmin,
    /// Secondary-index administration alone. Global scope only; needs server 6+.
    SIndexAdmin,
    /// Read data.
    Read,
    /// Read and write data.
    ReadWrite,
    /// Read and write data through user-defined functions.
    ReadWriteUdf,
    /// Write data.
    Write,
    /// Truncate data. Needs server 6+.
    Truncate,
    /// Data-masking administration. Global scope only.
    MaskingAdmin,
    /// Read masked data.
    ReadMasked,
    /// Write masked data.
    WriteMasked,
}

/// One privilege: what may be done, and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WirePrivilege {
    /// What is permitted.
    pub code: WirePrivilegeCode,
    /// Namespace the privilege is confined to. `None` means every namespace.
    pub namespace: Option<String>,
    /// Set within [`namespace`](Self::namespace) the privilege is confined to.
    /// `None` means every set.
    pub set_name: Option<String>,
}

impl WirePrivilege {
    /// Whether this privilege's code may be confined to a namespace or set.
    ///
    /// The administrative codes may not: they act on the cluster, so a namespace
    /// means nothing for them, and the server rejects the combination with a
    /// parameter error that names neither the privilege nor the reason. Checking it
    /// where the privilege is built is what turns that into something actionable.
    #[must_use]
    pub const fn is_scopable(&self) -> bool {
        matches!(
            self.code,
            WirePrivilegeCode::Read
                | WirePrivilegeCode::ReadWrite
                | WirePrivilegeCode::ReadWriteUdf
                | WirePrivilegeCode::Write
                | WirePrivilegeCode::Truncate
                | WirePrivilegeCode::ReadMasked
                | WirePrivilegeCode::WriteMasked
        )
    }

    /// Whether this privilege names a scope.
    #[must_use]
    pub const fn is_scoped(&self) -> bool {
        self.namespace.is_some() || self.set_name.is_some()
    }
}

/// One user the cluster knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUser {
    /// User name.
    pub user: String,
    /// Roles assigned to them.
    pub roles: Vec<String>,
    /// Read statistics, in the server's order: quota in records per second, the
    /// single-record rate, the scan/query record rate, and the number of
    /// limitless read scans and queries.
    ///
    /// A list rather than named fields because it is the *server's* list, and a
    /// future release may append to it — which a struct would turn into a decode
    /// failure. May be empty.
    pub read_info: Vec<u32>,
    /// Write statistics, in the same shape as [`read_info`](Self::read_info).
    pub write_info: Vec<u32>,
    /// Connections this user currently holds open.
    pub conns_in_use: u32,
}

/// One role the cluster knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRole {
    /// Role name.
    pub name: String,
    /// What the role permits.
    pub privileges: Vec<WirePrivilege>,
    /// Addresses a holder of the role may connect from. Empty means anywhere.
    pub allowlist: Vec<String>,
    /// Reads per second the role is limited to. `0` means unlimited.
    pub read_quota: u32,
    /// Writes per second the role is limited to. `0` means unlimited.
    pub write_quota: u32,
}

/// The fields every command here shares: which cluster, and how long to wait.
///
/// Its own type rather than two fields repeated eleven times — and it means a new
/// shared field is one change rather than eleven.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WireAdminTarget {
    /// Cluster instance.
    pub instance: String,
    /// Socket timeout for the admin exchange. `None` uses the daemon's default.
    pub timeout_ms: Option<u32>,
}

/// Payload of a [`USER_CREATE`](crate::opcode::USER_CREATE) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUserCreateBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The user to create.
    pub user: String,
    /// Their password, in the clear. See the module docs.
    pub password: String,
    /// Roles to assign at once. May be empty; roles can be granted later.
    pub roles: Vec<String>,
}

/// Payload of the requests that name a user and a set of roles:
/// [`USER_CREATE_PKI`](crate::opcode::USER_CREATE_PKI),
/// [`USER_GRANT_ROLES`](crate::opcode::USER_GRANT_ROLES) and
/// [`USER_REVOKE_ROLES`](crate::opcode::USER_REVOKE_ROLES).
///
/// One shape for three verbs because it genuinely is one shape. A PKI user has no
/// password — the client certificate identifies them — which is why creating one is
/// this body and not [`WireUserCreateBody`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUserRolesBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The user.
    pub user: String,
    /// The roles.
    pub roles: Vec<String>,
}

/// Payload of a [`USER_DROP`](crate::opcode::USER_DROP) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUserNameBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The user.
    pub user: String,
}

/// Payload of a [`USER_PASSWORD`](crate::opcode::USER_PASSWORD) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUserPasswordBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The user whose password changes.
    pub user: String,
    /// The new password, in the clear. See the module docs.
    pub password: String,
}

/// Payload of a [`USER_QUERY`](crate::opcode::USER_QUERY) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUserQueryBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The user to describe, or `None` for every user.
    pub user: Option<String>,
}

/// Payload of a [`USER_QUERY`](crate::opcode::USER_QUERY) reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUsers {
    /// The users, as the server listed them.
    pub users: Vec<WireUser>,
}

/// Payload of a [`ROLE_CREATE`](crate::opcode::ROLE_CREATE) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRoleCreateBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The role to create.
    pub role: String,
    /// What it permits.
    pub privileges: Vec<WirePrivilege>,
    /// Addresses a holder may connect from. Empty means anywhere.
    pub allowlist: Vec<String>,
    /// Reads per second, or `0` for unlimited.
    pub read_quota: u32,
    /// Writes per second, or `0` for unlimited.
    pub write_quota: u32,
}

/// Payload of a [`ROLE_DROP`](crate::opcode::ROLE_DROP) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRoleNameBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The role.
    pub role: String,
}

/// Payload of a [`ROLE_QUERY`](crate::opcode::ROLE_QUERY) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRoleQueryBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The role to describe, or `None` for every role.
    pub role: Option<String>,
}

/// Payload of a [`ROLE_QUERY`](crate::opcode::ROLE_QUERY) reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRoles {
    /// The roles, as the server listed them.
    pub roles: Vec<WireRole>,
}

/// Payload of the requests that change a role's privileges:
/// [`ROLE_GRANT_PRIVILEGES`](crate::opcode::ROLE_GRANT_PRIVILEGES) and
/// [`ROLE_REVOKE_PRIVILEGES`](crate::opcode::ROLE_REVOKE_PRIVILEGES).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRolePrivilegesBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The role.
    pub role: String,
    /// The privileges to grant or revoke.
    pub privileges: Vec<WirePrivilege>,
}

/// Payload of a [`ROLE_ALLOWLIST`](crate::opcode::ROLE_ALLOWLIST) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRoleAllowlistBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The role.
    pub role: String,
    /// The addresses a holder may connect from. **Empty clears the list**, which
    /// means "anywhere" — so this is how a restriction is removed as well as how
    /// one is set.
    pub allowlist: Vec<String>,
}

/// Payload of a [`ROLE_QUOTAS`](crate::opcode::ROLE_QUOTAS) request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRoleQuotasBody {
    /// Which cluster, and the timeout.
    pub target: WireAdminTarget,
    /// The role.
    pub role: String,
    /// Reads per second. **`0` means unlimited**, so this is how a quota is
    /// lifted as well as how one is set.
    pub read_quota: u32,
    /// Writes per second. `0` means unlimited.
    pub write_quota: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_body, encode_body};

    fn target() -> WireAdminTarget {
        WireAdminTarget {
            instance: crate::DEFAULT_INSTANCE.into(),
            timeout_ms: Some(3_000),
        }
    }

    /// Every privilege code has to survive the round trip, because a role read
    /// back from the server carries whichever ones it was created with.
    #[test]
    fn every_privilege_code_round_trips() {
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
            let bytes = encode_body(&code).unwrap();
            assert_eq!(decode_body::<WirePrivilegeCode>(&bytes).unwrap(), code);
        }
    }

    /// The administrative codes act on the cluster, so a namespace means nothing
    /// for them. Which ones may be scoped is the server's rule, and it is the gap
    /// in the discriminants: 0–4 administrative, 10+ data.
    #[test]
    fn only_the_data_privileges_may_be_scoped() {
        let scopable = |code| WirePrivilege {
            code,
            namespace: None,
            set_name: None,
        }
        .is_scopable();

        for code in [
            WirePrivilegeCode::UserAdmin,
            WirePrivilegeCode::SysAdmin,
            WirePrivilegeCode::DataAdmin,
            WirePrivilegeCode::UdfAdmin,
            WirePrivilegeCode::SIndexAdmin,
            WirePrivilegeCode::MaskingAdmin,
        ] {
            assert!(!scopable(code), "{code:?} is global-only");
        }
        for code in [
            WirePrivilegeCode::Read,
            WirePrivilegeCode::ReadWrite,
            WirePrivilegeCode::ReadWriteUdf,
            WirePrivilegeCode::Write,
            WirePrivilegeCode::Truncate,
            WirePrivilegeCode::ReadMasked,
            WirePrivilegeCode::WriteMasked,
        ] {
            assert!(scopable(code), "{code:?} may be confined to a namespace");
        }
    }

    #[test]
    fn a_privilege_knows_whether_it_names_a_scope() {
        let global = WirePrivilege {
            code: WirePrivilegeCode::Read,
            namespace: None,
            set_name: None,
        };
        assert!(!global.is_scoped());

        let namespaced = WirePrivilege {
            namespace: Some("test".into()),
            ..global.clone()
        };
        assert!(namespaced.is_scoped());

        // A set with no namespace is still a scope: the server rejects it, and
        // reporting it as unscoped would hide the mistake.
        let set_only = WirePrivilege {
            set_name: Some("users".into()),
            ..global
        };
        assert!(set_only.is_scoped());
    }

    #[test]
    fn a_privilege_round_trips_at_every_scope() {
        for privilege in [
            WirePrivilege {
                code: WirePrivilegeCode::ReadWrite,
                namespace: None,
                set_name: None,
            },
            WirePrivilege {
                code: WirePrivilegeCode::ReadWrite,
                namespace: Some("test".into()),
                set_name: None,
            },
            WirePrivilege {
                code: WirePrivilegeCode::ReadWrite,
                namespace: Some("test".into()),
                set_name: Some("users".into()),
            },
        ] {
            let bytes = encode_body(&privilege).unwrap();
            assert_eq!(decode_body::<WirePrivilege>(&bytes).unwrap(), privilege);
        }
    }

    #[test]
    fn a_user_round_trips_with_the_servers_statistic_lists() {
        let user = WireUser {
            user: "alice".into(),
            roles: vec!["read-write".into(), "sys-admin".into()],
            read_info: vec![100, 5, 0, 2],
            write_info: Vec::new(),
            conns_in_use: 3,
        };
        let bytes = encode_body(&WireUsers {
            users: vec![user.clone()],
        })
        .unwrap();
        assert_eq!(decode_body::<WireUsers>(&bytes).unwrap().users[0], user);

        // The statistic lists are the server's, and a future release may append
        // to them — a longer list must still decode, which a struct of named
        // fields would not have allowed.
        let longer = WireUser {
            read_info: vec![1, 2, 3, 4, 5, 6],
            ..user
        };
        let bytes = encode_body(&longer).unwrap();
        assert_eq!(decode_body::<WireUser>(&bytes).unwrap(), longer);
    }

    #[test]
    fn a_role_round_trips_with_its_privileges_and_limits() {
        let role = WireRole {
            name: "auditor".into(),
            privileges: vec![WirePrivilege {
                code: WirePrivilegeCode::Read,
                namespace: Some("test".into()),
                set_name: None,
            }],
            allowlist: vec!["10.0.0.0/8".into()],
            read_quota: 1_000,
            write_quota: 0,
        };
        let bytes = encode_body(&WireRoles {
            roles: vec![role.clone()],
        })
        .unwrap();
        assert_eq!(decode_body::<WireRoles>(&bytes).unwrap().roles[0], role);
    }

    /// The three verbs that share one body really do share it, so a change to the
    /// shape cannot reach one of them and miss the others.
    #[test]
    fn creating_a_pki_user_and_granting_roles_share_one_shape() {
        let body = WireUserRolesBody {
            target: target(),
            user: "alice".into(),
            roles: vec!["read".into()],
        };
        let bytes = encode_body(&body).unwrap();
        assert_eq!(decode_body::<WireUserRolesBody>(&bytes).unwrap(), body);

        // A PKI user has no password, which is why creating one is this body and
        // not the one with a password field.
        assert!(!String::from_utf8_lossy(&bytes).contains("password"));
    }

    #[test]
    fn every_request_body_round_trips() {
        let create = WireUserCreateBody {
            target: target(),
            user: "alice".into(),
            password: "secret".into(),
            roles: vec!["read-write".into()],
        };
        let bytes = encode_body(&create).unwrap();
        assert_eq!(decode_body::<WireUserCreateBody>(&bytes).unwrap(), create);

        let password = WireUserPasswordBody {
            target: target(),
            user: "alice".into(),
            password: "newer".into(),
        };
        let bytes = encode_body(&password).unwrap();
        assert_eq!(
            decode_body::<WireUserPasswordBody>(&bytes).unwrap(),
            password
        );

        let named = WireUserNameBody {
            target: target(),
            user: "alice".into(),
        };
        let bytes = encode_body(&named).unwrap();
        assert_eq!(decode_body::<WireUserNameBody>(&bytes).unwrap(), named);

        // `None` is "every user", which is a different request from naming one.
        for user in [None, Some("alice".to_string())] {
            let query = WireUserQueryBody {
                target: target(),
                user,
            };
            let bytes = encode_body(&query).unwrap();
            assert_eq!(decode_body::<WireUserQueryBody>(&bytes).unwrap(), query);
        }

        let role = WireRoleCreateBody {
            target: target(),
            role: "auditor".into(),
            privileges: vec![WirePrivilege {
                code: WirePrivilegeCode::Read,
                namespace: Some("test".into()),
                set_name: Some("users".into()),
            }],
            allowlist: vec!["127.0.0.1".into()],
            read_quota: 10,
            write_quota: 20,
        };
        let bytes = encode_body(&role).unwrap();
        assert_eq!(decode_body::<WireRoleCreateBody>(&bytes).unwrap(), role);

        let privileges = WireRolePrivilegesBody {
            target: target(),
            role: "auditor".into(),
            privileges: role.privileges.clone(),
        };
        let bytes = encode_body(&privileges).unwrap();
        assert_eq!(
            decode_body::<WireRolePrivilegesBody>(&bytes).unwrap(),
            privileges
        );

        // An empty allowlist clears the restriction, so it has to survive as an
        // empty list rather than being indistinguishable from "unset".
        for allowlist in [Vec::new(), vec!["10.0.0.1".to_string()]] {
            let body = WireRoleAllowlistBody {
                target: target(),
                role: "auditor".into(),
                allowlist,
            };
            let bytes = encode_body(&body).unwrap();
            assert_eq!(
                decode_body::<WireRoleAllowlistBody>(&bytes).unwrap(),
                body
            );
        }

        // Zero lifts a quota, so it is a meaningful value and not an absence.
        let quotas = WireRoleQuotasBody {
            target: target(),
            role: "auditor".into(),
            read_quota: 0,
            write_quota: 0,
        };
        let bytes = encode_body(&quotas).unwrap();
        assert_eq!(decode_body::<WireRoleQuotasBody>(&bytes).unwrap(), quotas);

        let dropped = WireRoleNameBody {
            target: target(),
            role: "auditor".into(),
        };
        let bytes = encode_body(&dropped).unwrap();
        assert_eq!(decode_body::<WireRoleNameBody>(&bytes).unwrap(), dropped);

        for role in [None, Some("auditor".to_string())] {
            let query = WireRoleQueryBody {
                target: target(),
                role,
            };
            let bytes = encode_body(&query).unwrap();
            assert_eq!(decode_body::<WireRoleQueryBody>(&bytes).unwrap(), query);
        }
    }
}
