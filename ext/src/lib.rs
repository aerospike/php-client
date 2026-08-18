// Copyright 2015-2026 Aerospike, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at http://www.apache.org/licenses/LICENSE-2.0

#![cfg_attr(windows, feature(abi_vectorcall))]
// In a debug build the `#[php_module]` macro expands to an extra entry point —
// `ext_php_rs_describe_module`, behind `cfg(debug_assertions)` — that it does
// not document, and there is no way to annotate a generated item. The lint is
// therefore applied to release builds, which is how the extension actually
// ships and how the PHP test suite builds it.
#![cfg_attr(not(debug_assertions), warn(missing_docs))]

//! The Aerospike PHP extension.
//!
//! This extension contains no database client. It is a thin PHP surface over
//! the shared-memory contract in `aerospike-php-ipc`; the real client, its
//! cluster state and its connections all live in `aerospike-php-daemon`, so
//! nothing here has to survive PHP's request lifecycle or its process model.
//!
//! [`Client`] exposes the single-record verbs — `ping`, `put`, `get`, `delete`,
//! `touch`, `exists`, `add`, `append` and `prepend` — each taking a policy, a
//! [`Key`] and whatever the verb operates on, **in the order
//! `aerospike-core`'s methods take them**. [`AerospikeException`] carries every
//! failure.
//!
//! # Everything the Rust client models as a type is a class here
//!
//! There are no associative arrays in the API surface. A [`Key`] is a class, so
//! is a [`Bin`], a [`Bins`] selection, a [`Record`], a [`ReadPolicy`], a
//! [`WritePolicy`] and an [`Expiration`]; the six policy enumerations and the
//! failure [`Status`] are real PHP enums. That is not symmetry for its own
//! sake — it is what lets the engine reject a wrong argument at the call
//! instead of the daemon reporting it a round trip later.
//!
//! The modules are private — nothing links this crate as a Rust library — so
//! they are named here rather than linked:
//!
//! - `transport` holds the process model: why nothing is opened in `MINIT`, how
//!   a forked worker recovers, and the send/wait loop.
//! - `value` holds the PHP ⇄ `WireValue` mapping, including the array rule.
//! - `types` holds the wrapper classes for the values PHP has no literal for:
//!   [`Blob`], [`GeoJson`], [`Hll`], [`Infinity`] and [`Wildcard`].
//! - `maps` holds [`OrderedMap`] and [`SortedMap`], the two map kinds a PHP
//!   array cannot express, with iteration and array access.
//! - `record` holds what a command names and returns: [`Key`], [`Bin`],
//!   [`Bins`], [`Record`], [`DaemonInfo`].
//! - `policy` holds [`ReadPolicy`], [`WritePolicy`], [`QueryPolicy`],
//!   [`AdminPolicy`], [`TxnVerifyPolicy`], [`TxnRollPolicy`] and [`Expiration`],
//!   and why they are immutable.
//! - `query` holds scans and queries: [`Statement`], [`Filter`],
//!   [`PartitionFilter`] and the [`RecordSet`] a traversal is read through one
//!   page at a time.
//! - `admin` holds what the cluster-management commands return: [`Task`],
//!   [`UdfModule`] and [`Node`] — and why a task polls from this process rather
//!   than blocking in the daemon.
//! - `txn` holds [`Transaction`], and why an unfinished one aborts itself when it
//!   is destroyed.
//! - `security` holds [`User`], [`Role`] and [`Privilege`] — and which privilege
//!   codes may be confined to a namespace.
//! - `enums` holds the PHP enums, and the correction that lets an enum case be
//!   returned to PHP without corrupting its reference count.
//! - `arg` holds the wrapper that makes an optional typed argument actually
//!   type-checked, which PHP does not do for an extension.
//! - `batch` holds the batch rows and their per-row results.
//! - `settings` holds the `php.ini` entries.
//!
//! # Version lock-step
//!
//! This extension only talks to a daemon of **exactly its own version**; the
//! shared-memory service name has the version in it, so a mismatched pair never
//! meets. The assertion below is what keeps this crate's version — which cannot
//! be inherited from the cargo workspace, because a PHP extension must not be
//! part of it — equal to the contract's.

mod admin;
mod arg;
mod batch;
mod client;
mod enums;
mod error;
mod exp;
mod maps;
mod ops;
mod policy;
mod query;
mod record;
mod security;
mod settings;
mod transport;
mod txn;
mod types;
mod value;

use ext_php_rs::prelude::*;
use ext_php_rs::zend::IniEntryDef;

pub use admin::{Node, Task, UdfModule};
pub use batch::{BatchDelete, BatchRead, BatchResult, BatchRow, BatchUdf, BatchWrite};
pub use client::Client;
pub use enums::{
    AbortStatus, BinWriteMode, BitOverflow, BitResize, CollectionIndex, CommitLevel,
    CommitStatus, ExpType, GenerationPolicy, IndexType, ListOrder, ListReturn, MapOrder,
    MapReturn,
    MapWriteMode, PrivilegeCode, ReadModeAp, ReadModeSc, RecordExistsAction, Replica, Status,
    LoopVarPart, StringNumericType, StringRegexFlag, TaskStatus, TxnState, UdfLanguage,
};
pub use error::AerospikeException;
pub use exp::{Exp, ExpBit, ExpHll, ExpList, ExpMap, ExpPath, ExpStr, ModifyFlag, RegexFlag, SelectFlag};
pub use maps::{OrderedMap, SortedMap};
pub use ops::{
    BinPolicy, BitOp, Ctx, ExpOp, Expression, HllOp, ListOp, ListPolicy, MapOp, MapPolicy, Op,
    Operation,
};
pub use policy::{
    AdminPolicy, Expiration, QueryPolicy, ReadPolicy, TxnRollPolicy, TxnVerifyPolicy, WritePolicy,
};
pub use query::{Filter, PartitionFilter, RecordSet, Statement};
pub use record::{Bin, Bins, DaemonInfo, Key, Record};
pub use security::{Privilege, Role, User};
pub use txn::Transaction;
pub use types::{Blob, GeoJson, Hll, Infinity, Wildcard};

const _: () = assert!(
    aerospike_php_ipc::is_version(env!("CARGO_PKG_VERSION")),
    "this extension's version must equal aerospike-php-ipc's: the daemon and the extension are \
     matched by version, so an extension whose own version differs from the contract it speaks \
     would report a version that pairs with nothing"
);

/// `MINIT`.
///
/// Under PHP-FPM this runs in the **master** process, before the worker pool
/// is forked, so it must not open anything that cannot cross `fork()`.
/// Registering ini entries is safe; the shared-memory ports are deliberately
/// left until a worker first needs them — see the `transport` module for the
/// full reasoning.
pub fn startup(_ty: i32, module_number: i32) -> i32 {
    IniEntryDef::register(settings::ini_entries(), module_number);
    0
}

/// `MSHUTDOWN`.
///
/// Releases this process's ports. Safe in the master too, which never
/// attached and therefore has nothing to release.
///
/// # Safety
/// Called by PHP with the module type and number; there is nothing here for a
/// caller to get wrong, and it must not unwind across the FFI boundary, which
/// [`transport::teardown`] guarantees.
unsafe extern "C" fn shutdown(_ty: i32, _module_number: i32) -> i32 {
    transport::teardown();
    0
}

/// The PHP globals a build with no PHP around it has to define for itself.
///
/// `ext_php_rs` compiles a small C shim whose object file references PHP's own
/// global variables. In the extension that is exactly right — it is loaded into
/// a PHP process, which defines them, and on macOS the link deliberately leaves
/// them undefined (`-undefined dynamic_lookup` in `.cargo/config.toml`) for PHP to
/// resolve. But macOS binds data symbols when the image loads, so **anything that
/// loads this code outside a PHP process fails immediately** with `symbol not found
/// in flat namespace '_executor_globals'`.
///
/// Two things do that, which is why this is gated on a feature as well as on
/// `test`:
///
/// - `cargo test`, whose harness is a plain binary. It aborts before the first
///   test without these.
/// - `cargo php stubs`, which `dlopen`s the built `cdylib` and calls
///   `ext_php_rs_describe_module` to enumerate the classes. It is a plain binary
///   too, so it needs the same slabs — hence `--features stub-gen`. Without them
///   `dlopen` fails and cargo-php 0.1.11 reports **nothing at all**: it exits 0
///   having written no file, which looks like an extension with no classes in it
///   rather than a load error.
///
/// Nothing reads these. Only the address is bound, and any code path that
/// genuinely needed a PHP runtime is a path that has to be tested from PHP —
/// which is what `tests/smoke.php` is for.
#[cfg(any(test, feature = "stub-gen"))]
mod php_globals_for_tests {
    /// Sized generously and never read; only the address is bound.
    const SLAB: usize = 8192;

    /// One slab per global the extension's linked code refers to. Add another
    /// when a new `dyld: symbol not found in flat namespace` appears — that is
    /// the whole maintenance cost, and it fails loudly rather than subtly.
    macro_rules! php_global {
        ($($name:ident),* $(,)?) => {
            $(
                #[no_mangle]
                static mut $name: [u8; SLAB] = [0; SLAB];
            )*
        };
    }

    /// PHP's allocator, on the system one.
    ///
    /// These three cannot be aborting stubs like the rest: `ext_php_rs_describe_module`
    /// builds the class description on the heap, so it really does call `_emalloc`.
    /// The engine's allocator is per-request and arena-backed; `malloc` is not the
    /// same thing, but for a process that describes itself and exits it is
    /// indistinguishable.
    ///
    /// These forward straight to `malloc`/`free` rather than to Rust's
    /// `alloc`/`dealloc`, and that is the whole trick: `dealloc` needs the layout
    /// back, so wrapping it means storing the size in a header and returning a
    /// *shifted* pointer — which dies the moment anything releases the block with
    /// the system `free()` instead of `_efree`, as `pointer being freed was not
    /// allocated`. `malloc` and `free` are interchangeable with themselves, so
    /// every mix of paths works.
    mod allocator {
        extern "C" {
            fn malloc(size: usize) -> *mut u8;
            fn free(ptr: *mut u8);
        }

        #[no_mangle]
        unsafe extern "C" fn _emalloc(size: usize) -> *mut u8 {
            malloc(size)
        }

        /// PHP's "allocate or die" variant. The same thing here: a null return is
        /// already fatal for the caller.
        #[no_mangle]
        unsafe extern "C" fn __zend_malloc(size: usize) -> *mut u8 {
            malloc(size)
        }

        #[no_mangle]
        unsafe extern "C" fn _efree(ptr: *mut u8) {
            free(ptr);
        }
    }

    /// One aborting stub per PHP *function* the shim calls.
    ///
    /// A data slab would satisfy the loader just as well — the flat namespace only
    /// binds a name to an address — but calling one would execute zeroes and die as
    /// an illegal instruction with nothing to read. These say which symbol was
    /// reached instead, so if `describe_module` ever grows a call into the engine
    /// the failure names it.
    macro_rules! php_fn {
        ($($name:ident),* $(,)?) => {
            $(
                #[no_mangle]
                extern "C" fn $name() -> ! {
                    // Not `panic!`: unwinding out of an `extern "C"` frame is
                    // undefined, and there is no caller able to handle it anyway.
                    eprintln!(
                        "aerospike-php was built with the `stub-gen` feature, which replaces \
                         PHP's own {} with a stub, and something called it. This build is only \
                         for `cargo php stubs` — never load it into PHP.",
                        stringify!($name),
                    );
                    std::process::abort()
                }
            )*
        };
    }

    php_fn!(
        _zend_bailout,
        _zend_new_array,
        gc_possible_root,
        instanceof_function_slow,
        object_properties_init,
        zend_array_count,
        zend_array_destroy,
        zend_declare_class_constant,
        zend_declare_property,
        zend_do_implement_interface,
        zend_enum_add_case,
        zend_enum_get_case,
        zend_hash_find,
        zend_hash_get_current_data_ex,
        zend_hash_get_current_key_type_ex,
        zend_hash_get_current_key_zval_ex,
        zend_hash_index_find,
        zend_hash_index_update,
        zend_hash_move_forward_ex,
        zend_hash_next_index_insert,
        zend_hash_str_find,
        zend_hash_str_update,
        zend_hash_update,
        zend_is_callable,
        zend_is_iterable,
        zend_is_true,
        zend_object_std_dtor,
        zend_object_std_init,
        zend_objects_clone_members,
        zend_objects_store_del,
        zend_register_ini_entries,
        zend_register_internal_class_ex,
        zend_register_internal_enum,
        zend_register_internal_interface,
        zend_std_get_properties,
        zend_std_has_property,
        zend_std_read_property,
        zend_std_write_property,
        zend_string_init_interned,
        zend_throw_error,
        zend_throw_exception_ex,
        zend_throw_exception_object,
        zend_wrong_parameters_count_error,
        zval_ptr_dtor,
    );

    php_global!(
        executor_globals,
        std_object_handlers,
        // Two real data symbols, not functions: a `zend_string *` and the engine's
        // table of 256 one-character interned strings.
        zend_empty_string,
        zend_one_char_string,
        // Every class entry `ext_php_rs::zend::ce` can hand out. They come as a
        // group: the module is one compilation unit, so referring to any one of
        // them makes the linker want all of them.
        zend_ce_aggregate,
        zend_ce_argument_count_error,
        zend_ce_arithmetic_error,
        zend_ce_arrayaccess,
        zend_ce_compile_error,
        zend_ce_countable,
        zend_ce_division_by_zero_error,
        zend_ce_error_exception,
        zend_ce_exception,
        zend_ce_iterator,
        zend_ce_parse_error,
        zend_ce_serializable,
        zend_ce_stringable,
        zend_ce_throwable,
        zend_ce_traversable,
        zend_ce_type_error,
        zend_ce_unhandled_match_error,
        zend_ce_value_error,
    );
}

/// Registers the extension with PHP.
#[php_module]
#[php(startup = "startup")]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .shutdown_function(shutdown)
        // Enums first: a class that names one in a property or a return type
        // needs its class entry to exist already.
        .enumeration::<Replica>()
        .enumeration::<ReadModeAp>()
        .enumeration::<ReadModeSc>()
        .enumeration::<RecordExistsAction>()
        .enumeration::<GenerationPolicy>()
        .enumeration::<CommitLevel>()
        .enumeration::<Status>()
        .enumeration::<ListOrder>()
        .enumeration::<ListReturn>()
        .enumeration::<MapOrder>()
        .enumeration::<MapReturn>()
        .enumeration::<MapWriteMode>()
        .enumeration::<BinWriteMode>()
        .enumeration::<BitResize>()
        .enumeration::<BitOverflow>()
        .enumeration::<CollectionIndex>()
        .enumeration::<TaskStatus>()
        .enumeration::<IndexType>()
        .enumeration::<UdfLanguage>()
        .enumeration::<TxnState>()
        .enumeration::<CommitStatus>()
        .enumeration::<AbortStatus>()
        .enumeration::<PrivilegeCode>()
        .enumeration::<ExpType>()
        .enumeration::<StringNumericType>()
        .enumeration::<LoopVarPart>()
        .class::<AerospikeException>()
        .class::<Client>()
        .class::<Key>()
        .class::<Bin>()
        .class::<Bins>()
        .class::<Record>()
        .class::<DaemonInfo>()
        .class::<ReadPolicy>()
        .class::<WritePolicy>()
        .class::<QueryPolicy>()
        .class::<AdminPolicy>()
        .class::<TxnVerifyPolicy>()
        .class::<TxnRollPolicy>()
        .class::<Expiration>()
        .class::<Filter>()
        .class::<Statement>()
        .class::<PartitionFilter>()
        .class::<RecordSet>()
        .class::<Task>()
        .class::<UdfModule>()
        .class::<Node>()
        .class::<Transaction>()
        .class::<Privilege>()
        .class::<User>()
        .class::<Role>()
        .class::<Operation>()
        .class::<Op>()
        .class::<ListOp>()
        .class::<ListPolicy>()
        .class::<MapOp>()
        .class::<MapPolicy>()
        .class::<BitOp>()
        .class::<HllOp>()
        .class::<BinPolicy>()
        .class::<ExpOp>()
        .class::<Expression>()
        .class::<Exp>()
        .class::<RegexFlag>()
        .class::<ExpList>()
        .class::<ExpMap>()
        .class::<ExpBit>()
        .class::<ExpHll>()
        .class::<ExpStr>()
        .class::<StringRegexFlag>()
        .class::<ExpPath>()
        .class::<SelectFlag>()
        .class::<ModifyFlag>()
        .class::<BatchRow>()
        .class::<BatchRead>()
        .class::<BatchWrite>()
        .class::<BatchDelete>()
        .class::<BatchUdf>()
        .class::<BatchResult>()
        .class::<Ctx>()
        .class::<OrderedMap>()
        .class::<SortedMap>()
        .class::<Blob>()
        .class::<GeoJson>()
        .class::<Hll>()
        .class::<Infinity>()
        .class::<Wildcard>()
}
