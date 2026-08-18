<?php

/**
 * The 1.x client's `Client`, over this one.
 *
 * Three things it does:
 *
 * 1. **`connect()`.** 1.x took a Unix socket path, because the connection manager
 *    listened on one. This client names a *daemon instance* configured in the
 *    daemon's own file. A socket path is turned into an instance name by taking its
 *    basename without extension, which is the convention that makes
 *    `/tmp/asd-default.sock` mean `default`; pass an instance name directly and it
 *    is used as-is.
 * 2. **Unwraps policies.** Every verb accepts a {@see \Aerospike\Compat\WritePolicy}
 *    and friends, calling `build()` for you, so old code needs no edit.
 * 3. **The renamed verbs.** `createIndex`, `dropUdf` and `udfExecute` are what 1.x
 *    called `createIndexOnBin`, `removeUdf` and `executeUdf`.
 *
 * Everything else is forwarded untouched by `__call`, so a verb this client has and
 * 1.x did not — `operate()`, `beginTransaction()`, `info()` — is reachable here too.
 */

declare(strict_types=1);

namespace Aerospike\Compat;

use Aerospike\BatchRecord;
use Aerospike\Bins;
use Aerospike\Client as NewClient;
use Aerospike\Key;
use Aerospike\PartitionFilter;
use Aerospike\Record;
use Aerospike\Statement;
use Aerospike\UdfMeta;

class Client
{
    private function __construct(private NewClient $inner, private string $instance)
    {
    }

    /**
     * Connect, by daemon instance name or by 1.x socket path.
     *
     * ```php
     * $client = Aerospike\Compat\Client::connect('default');
     * $client = Aerospike\Compat\Client::connect('/tmp/asd-default.sock');  // 1.x
     * ```
     *
     * Note that nothing is opened here. This client attaches to the daemon lazily,
     * on the first command, because under PHP-FPM the process that loads the
     * extension is not the one that runs the request — see the extension's notes on
     * the process model. So a wrong instance name is reported by the first verb, not
     * by this call, which is a difference from 1.x worth knowing when reading a
     * stack trace.
     */
    public static function connect(string $socketOrInstance): self
    {
        $instance = self::instanceOf($socketOrInstance);
        return new self(new NewClient($instance), $instance);
    }

    /** The instance name this talks to. 1.x returned the socket path. */
    public function socket(): string
    {
        return $this->instance;
    }

    /** The wrapped client, for code that has moved on. */
    public function inner(): NewClient
    {
        return $this->inner;
    }

    // ===== verbs whose policy needs unwrapping ==============================

    public function put(mixed $policy, Key $key, array $bins): void
    {
        $this->inner->put(built($policy), $key, $bins);
    }

    public function get(mixed $policy, Key $key, ?array $bins = null): ?Record
    {
        return $this->inner->get(built($policy), $key, self::binsOf($bins));
    }

    public function getHeader(mixed $policy, Key $key): ?Record
    {
        return $this->inner->getHeader(built($policy), $key);
    }

    public function add(mixed $policy, Key $key, array $bins): void
    {
        $this->inner->add(built($policy), $key, $bins);
    }

    public function append(mixed $policy, Key $key, array $bins): void
    {
        $this->inner->append(built($policy), $key, $bins);
    }

    public function prepend(mixed $policy, Key $key, array $bins): void
    {
        $this->inner->prepend(built($policy), $key, $bins);
    }

    public function delete(mixed $policy, Key $key): bool
    {
        return $this->inner->delete(built($policy), $key);
    }

    public function touch(mixed $policy, Key $key): void
    {
        $this->inner->touch(built($policy), $key);
    }

    public function exists(mixed $policy, Key $key): bool
    {
        return $this->inner->exists(built($policy), $key);
    }

    public function operate(mixed $policy, Key $key, array $ops): ?Record
    {
        return $this->inner->operate(built($policy), $key, $ops);
    }

    /**
     * A batch, as the 1.x array of {@see \Aerospike\BatchRecord}.
     *
     * A reply row does not carry its key, so each result is paired with the key from
     * the command at the same index — the rows come back in the order they were sent.
     * A command that exposes no key leaves it `null`.
     *
     * Nothing is lost by the wrapping: a `BatchRecord` forwards `isOk()`,
     * `resultCode()`, `isInDoubt()` and `message()` to the `BatchResult` inside it,
     * which `inner()` returns.
     */
    public function batch(mixed $policy, array $commands): array
    {
        $results = $this->inner->batch(built($policy), $commands);
        $rows = [];
        foreach ($results as $index => $result) {
            $rows[] = new BatchRecord(self::keyOf($commands[$index] ?? null), $result);
        }
        return $rows;
    }

    /**
     * Truncate a set, or a whole namespace.
     *
     * 1.x took an `InfoPolicy`; this client takes an `AdminPolicy`, and
     * {@see InfoPolicy} builds one.
     *
     * A `$beforeNanos` in the **future** truncates nothing: the server refuses a
     * cutoff ahead of its own clock. That was true of 1.x as well, and it is the
     * most common surprise here, so it is worth repeating.
     */
    public function truncate(mixed $policy, string $namespace, string $setName, ?int $beforeNanos = null): void
    {
        $this->inner->truncate(built($policy), $namespace, $setName, $beforeNanos);
    }

    /** Every record in a set, as a {@see Recordset}. */
    public function scan(mixed $policy, mixed $partitionFilter, string $namespace, string $setName, ?array $bins = null): Recordset
    {
        return new Recordset(
            $this->inner->scan(built($policy), self::partitionsOf($partitionFilter), $namespace, $setName, $bins)
        );
    }

    /** A secondary-index query, as a {@see Recordset}. */
    public function query(mixed $policy, mixed $partitionFilter, Statement $statement): Recordset
    {
        return new Recordset(
            $this->inner->query(built($policy), self::partitionsOf($partitionFilter), $statement)
        );
    }

    // ===== verbs this client renamed =======================================

    /** 1.x `createIndex`, which this client calls `createIndexOnBin`. */
    public function createIndex(
        mixed $policy,
        string $namespace,
        string $setName,
        string $binName,
        string $indexName,
        mixed $indexType,
        mixed $collectionType = null,
        ?array $ctx = null,
    ): mixed {
        return $this->inner->createIndexOnBin(
            built($policy),
            $namespace,
            $setName,
            $binName,
            $indexName,
            $indexType,
            $collectionType,
            $ctx,
        );
    }

    public function dropIndex(mixed $policy, string $namespace, string $setName, string $indexName): mixed
    {
        return $this->inner->dropIndex(built($policy), $namespace, $setName, $indexName);
    }

    public function registerUdf(mixed $policy, string $body, string $packageName, mixed $language = null): mixed
    {
        return $this->inner->registerUdf(built($policy), $body, $packageName, $language);
    }

    /** 1.x `dropUdf`, which this client calls `removeUdf`. */
    public function dropUdf(mixed $policy, string $packageName): mixed
    {
        return $this->inner->removeUdf(built($policy), $packageName);
    }

    /** The registered modules, as the 1.x array of {@see \Aerospike\UdfMeta}. */
    public function listUdf(mixed $policy): array
    {
        return array_map(
            fn($module) => new UdfMeta($module),
            $this->inner->listUdf(built($policy))
        );
    }

    /** 1.x `udfExecute`, which this client calls `executeUdf`. */
    public function udfExecute(mixed $policy, Key $key, string $packageName, string $functionName, array $args): mixed
    {
        return $this->inner->executeUdf(built($policy), $key, $packageName, $functionName, $args);
    }

    // ===== security ========================================================

    public function createUser(mixed $policy, string $user, string $password, array $roles): void
    {
        $this->inner->createUser(built($policy), $user, $password, $roles);
    }

    public function dropUser(mixed $policy, string $user): void
    {
        $this->inner->dropUser(built($policy), $user);
    }

    public function changePassword(mixed $policy, string $user, string $password): void
    {
        $this->inner->changePassword(built($policy), $user, $password);
    }

    public function grantRoles(mixed $policy, string $user, array $roles): void
    {
        $this->inner->grantRoles(built($policy), $user, $roles);
    }

    public function revokeRoles(mixed $policy, string $user, array $roles): void
    {
        $this->inner->revokeRoles(built($policy), $user, $roles);
    }

    public function queryUsers(mixed $policy, ?string $user = null): array
    {
        return $this->inner->queryUsers(built($policy), $user);
    }

    public function queryRoles(mixed $policy, ?string $roleName = null): array
    {
        return $this->inner->queryRoles(built($policy), $roleName);
    }

    public function createRole(
        mixed $policy,
        string $roleName,
        array $privileges,
        array $allowlist = [],
        int $readQuota = 0,
        int $writeQuota = 0,
    ): void {
        $this->inner->createRole($policy = built($policy), $roleName, $privileges);
        if ($allowlist !== []) {
            $this->inner->setAllowlist($policy, $roleName, $allowlist);
        }
        if ($readQuota !== 0 || $writeQuota !== 0) {
            $this->inner->setQuotas($policy, $roleName, $readQuota, $writeQuota);
        }
    }

    public function dropRole(mixed $policy, string $roleName): void
    {
        $this->inner->dropRole(built($policy), $roleName);
    }

    public function grantPrivileges(mixed $policy, string $roleName, array $privileges): void
    {
        $this->inner->grantPrivileges(built($policy), $roleName, $privileges);
    }

    public function revokePrivileges(mixed $policy, string $roleName, array $privileges): void
    {
        $this->inner->revokePrivileges(built($policy), $roleName, $privileges);
    }

    public function setAllowlist(mixed $policy, string $roleName, array $allowlist): void
    {
        $this->inner->setAllowlist(built($policy), $roleName, $allowlist);
    }

    public function setQuotas(mixed $policy, string $roleName, int $readQuota, int $writeQuota): void
    {
        $this->inner->setQuotas(built($policy), $roleName, $readQuota, $writeQuota);
    }

    /**
     * Anything else this client offers.
     *
     * Forwarded untouched, so `beginTransaction()`, `info()`, `nodes()`, `ping()`
     * and the rest are reachable without unwrapping this. A policy argument is *not*
     * unwrapped here — those verbs were not in 1.x, so nothing is passing a 1.x
     * policy to them.
     */
    public function __call(string $name, array $arguments): mixed
    {
        return $this->inner->$name(...$arguments);
    }

    /**
     * A daemon instance name from what 1.x passed.
     *
     * A path becomes its basename without extension; anything else is already an
     * instance name. `/tmp/asd-default.sock` and `default` both mean `default`,
     * which is the mapping that makes the common 1.x configuration work untouched.
     */
    private static function instanceOf(string $socketOrInstance): string
    {
        if (!str_contains($socketOrInstance, '/') && !str_contains($socketOrInstance, '.')) {
            return $socketOrInstance;
        }
        $base = pathinfo($socketOrInstance, PATHINFO_FILENAME);
        // `asd-default` → `default`, the connection manager's own naming.
        if (str_starts_with($base, 'asd-')) {
            $base = substr($base, 4);
        }
        return $base === '' ? 'default' : $base;
    }

    /**
     * The key a batch command was for, when it has one.
     *
     * Every command class exposes `key()`, but defensively: a future one that does not
     * yields a `null` key rather than a thrown error, because the row's *result* is
     * what the caller asked for and the key is the part this layer is adding back.
     */
    private static function keyOf(mixed $command): ?Key
    {
        if (is_object($command) && method_exists($command, 'key')) {
            $key = $command->key();
            return $key instanceof Key ? $key : null;
        }
        return null;
    }

    /** 1.x passed bin names as a plain array; this client has a `Bins` selection. */
    private static function binsOf(?array $bins): ?Bins
    {
        return $bins === null ? null : Bins::some($bins);
    }

    /** 1.x passed `mixed` for the partition filter, including `null`. */
    private static function partitionsOf(mixed $partitions): ?PartitionFilter
    {
        return $partitions instanceof PartitionFilter ? $partitions : null;
    }
}
