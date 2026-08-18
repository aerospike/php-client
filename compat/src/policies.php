<?php

/**
 * The 1.x client's mutable policies.
 *
 * These are **builders**. The 1.x policy was a bag of public properties with
 * getters and setters; this client's is immutable, built by one named-argument
 * constructor. So each class here collects the settings and `build()` turns them
 * into the real thing.
 *
 * ```php
 * $p = new Aerospike\Compat\WritePolicy();
 * $p->setExpiration(3600);
 * $p->sendKey = true;              // the property form works too
 * $client->put($p->build(), $key, $bins);
 * ```
 *
 * {@see \Aerospike\Compat\Client} calls `build()` for you, so old code that passes
 * a policy straight to a verb keeps working — it is only the *real*
 * `Aerospike\Client` that will refuse one, deliberately: a silent conversion would
 * hide the single most useful fact during a migration.
 *
 * # Why the properties are real properties
 *
 * The extension cannot offer these safely — a property registered from Rust is
 * untyped to PHP, so `$policy->expiraton = 3600` (note the typo) would be accepted
 * and quietly do nothing. In plain PHP the same risk exists, so every class here
 * declares its properties explicitly and `__set` refuses an unknown name. That is
 * strictly better than the 1.x behaviour, which silently accepted anything.
 */

declare(strict_types=1);

namespace Aerospike\Compat;

use Aerospike\Expiration;
use Aerospike\Expression;
use Aerospike\ReadPolicy as NewReadPolicy;
use Aerospike\QueryPolicy as NewQueryPolicy;
use Aerospike\WritePolicy as NewWritePolicy;
use Aerospike\AdminPolicy as NewAdminPolicy;

/**
 * Settings every policy shares, and the guard that makes a typo an error.
 *
 * The 1.x classes each declared their own copy of these; one trait keeps them from
 * drifting.
 */
trait PolicyFields
{
    public ?int $total_timeout = null;
    public ?int $socket_timeout = null;
    public ?int $max_retries = null;
    public ?float $sleep_multiplier = null;
    public mixed $read_mode_ap = null;
    public mixed $read_mode_sc = null;
    public ?bool $use_compression = null;
    public mixed $filter_expression = null;

    /**
     * Accepted and inert, as it was in 1.x for most builds: the daemon owns the
     * connection pool, so a per-call decision about exhausting it has nothing to
     * act on. Kept so old code keeps parsing.
     */
    public ?bool $exit_fast_on_exhausted_connection_pool = null;

    /** Refuse an unknown property instead of silently accepting it. */
    public function __set(string $name, mixed $value): void
    {
        throw new \InvalidArgumentException(sprintf(
            '%s has no property $%s. The 1.x client would have accepted this and done nothing; '
            . 'this refuses so a typo is visible. Declared: %s',
            static::class,
            $name,
            implode(', ', array_map(fn($p) => '$' . $p, array_keys(get_object_vars($this))))
        ));
    }

    public function getTotalTimeout(): ?int
    {
        return $this->total_timeout;
    }

    public function setTotalTimeout(int $millis): static
    {
        $this->total_timeout = $millis;
        return $this;
    }

    public function getSocketTimeout(): ?int
    {
        return $this->socket_timeout;
    }

    public function setSocketTimeout(int $millis): static
    {
        $this->socket_timeout = $millis;
        return $this;
    }

    public function getMaxRetries(): ?int
    {
        return $this->max_retries;
    }

    public function setMaxRetries(int $retries): static
    {
        $this->max_retries = $retries;
        return $this;
    }

    public function getSleepMultiplier(): ?float
    {
        return $this->sleep_multiplier;
    }

    /**
     * Accepted and inert.
     *
     * 1.x multiplied the retry pause by this each attempt; this client's retry
     * pause is a fixed `sleepBetweenRetriesMs`, because an exponential client-side
     * backoff and a server-side timeout interact in ways that are hard to reason
     * about and were never what anyone tuned.
     */
    public function setSleepMultiplier(float $multiplier): static
    {
        $this->sleep_multiplier = $multiplier;
        return $this;
    }

    public function getReadModeAp(): mixed
    {
        return $this->read_mode_ap;
    }

    public function setReadModeAp(mixed $mode): static
    {
        $this->read_mode_ap = $mode;
        return $this;
    }

    public function getReadModeSc(): mixed
    {
        return $this->read_mode_sc;
    }

    public function setReadModeSc(mixed $mode): static
    {
        $this->read_mode_sc = $mode;
        return $this;
    }

    public function getUseCompression(): ?bool
    {
        return $this->use_compression;
    }

    public function setUseCompression(bool $compress): static
    {
        $this->use_compression = $compress;
        return $this;
    }

    public function getFilterExpression(): mixed
    {
        return $this->filter_expression;
    }

    public function setFilterExpression(mixed $filter): static
    {
        $this->filter_expression = $filter;
        return $this;
    }

    public function getExitFastOnExhaustedConnectionPool(): ?bool
    {
        return $this->exit_fast_on_exhausted_connection_pool;
    }

    public function setExitFastOnExhaustedConnectionPool(bool $exit): static
    {
        $this->exit_fast_on_exhausted_connection_pool = $exit;
        return $this;
    }

    /**
     * The filter, as this client takes it.
     *
     * 1.x put an `Expression` on `filter_expression`; this client has two
     * parameters — `filter` for Aerospike Expression Language *text* and
     * `filterExp` for a built or packed one. An `Expression` object goes to
     * `filterExp`, a string to `filter`.
     *
     * @return array{0: ?string, 1: ?Expression}
     */
    protected function filterParts(): array
    {
        if ($this->filter_expression === null) {
            return [null, null];
        }
        if ($this->filter_expression instanceof Expression) {
            return [null, $this->filter_expression];
        }
        if (is_string($this->filter_expression)) {
            return [$this->filter_expression, null];
        }
        throw new \InvalidArgumentException(
            'filter_expression must be an Aerospike\Expression or Aerospike Expression Language '
            . 'text, but is ' . get_debug_type($this->filter_expression)
        );
    }
}

/** 1.x {@see \Aerospike\ReadPolicy}, mutable. */
class ReadPolicy
{
    use PolicyFields;

    /** Accepted and inert on a read: a read stores nothing, so there is no key to send. */
    public ?bool $send_key = null;

    public function getSendKey(): ?bool
    {
        return $this->send_key;
    }

    public function setSendKey(bool $send): static
    {
        $this->send_key = $send;
        return $this;
    }

    /** The immutable policy this describes. */
    public function build(): NewReadPolicy
    {
        [$filter, $filterExp] = $this->filterParts();
        return new NewReadPolicy(
            totalTimeoutMs: $this->total_timeout,
            socketTimeoutMs: $this->socket_timeout,
            maxRetries: $this->max_retries,
            readModeAp: $this->read_mode_ap,
            readModeSc: $this->read_mode_sc,
            useCompression: $this->use_compression,
            filter: $filter,
            filterExp: $filterExp,
        );
    }
}

/** 1.x {@see \Aerospike\WritePolicy}, mutable. */
class WritePolicy
{
    use PolicyFields;

    public mixed $record_exists_action = null;
    public mixed $generation_policy = null;
    public ?int $generation = null;
    public mixed $expiration = null;
    public mixed $commit_level = null;
    public ?bool $durable_delete = null;
    public ?bool $respond_per_each_op = null;
    public ?bool $send_key = null;

    public function getRecordExistsAction(): mixed
    {
        return $this->record_exists_action;
    }

    public function setRecordExistsAction(mixed $action): static
    {
        $this->record_exists_action = $action;
        return $this;
    }

    public function getGenerationPolicy(): mixed
    {
        return $this->generation_policy;
    }

    public function setGenerationPolicy(mixed $policy): static
    {
        $this->generation_policy = $policy;
        return $this;
    }

    public function getGeneration(): ?int
    {
        return $this->generation;
    }

    public function setGeneration(int $generation): static
    {
        $this->generation = $generation;
        return $this;
    }

    public function getExpiration(): mixed
    {
        return $this->expiration;
    }

    /**
     * The record's time-to-live.
     *
     * Takes what 1.x took — a number of seconds — or this client's
     * `Aerospike\Expiration`, which names the three cases that are not durations.
     */
    public function setExpiration(mixed $expiration): static
    {
        $this->expiration = $expiration;
        return $this;
    }

    public function getCommitLevel(): mixed
    {
        return $this->commit_level;
    }

    public function setCommitLevel(mixed $level): static
    {
        $this->commit_level = $level;
        return $this;
    }

    public function getDurableDelete(): ?bool
    {
        return $this->durable_delete;
    }

    public function setDurableDelete(bool $durable): static
    {
        $this->durable_delete = $durable;
        return $this;
    }

    public function getRespondPerEachOp(): ?bool
    {
        return $this->respond_per_each_op;
    }

    public function setRespondPerEachOp(bool $respond): static
    {
        $this->respond_per_each_op = $respond;
        return $this;
    }

    public function getSendKey(): ?bool
    {
        return $this->send_key;
    }

    public function setSendKey(bool $send): static
    {
        $this->send_key = $send;
        return $this;
    }

    /** The immutable policy this describes. */
    public function build(): NewWritePolicy
    {
        [$filter, $filterExp] = $this->filterParts();
        return new NewWritePolicy(
            totalTimeoutMs: $this->total_timeout,
            socketTimeoutMs: $this->socket_timeout,
            maxRetries: $this->max_retries,
            readModeAp: $this->read_mode_ap,
            readModeSc: $this->read_mode_sc,
            useCompression: $this->use_compression,
            filter: $filter,
            filterExp: $filterExp,
            recordExistsAction: $this->record_exists_action,
            generationPolicy: $this->generation_policy,
            generation: $this->generation,
            expiration: self::expirationOf($this->expiration),
            commitLevel: $this->commit_level,
            durableDelete: $this->durable_delete,
            respondPerEachOp: $this->respond_per_each_op,
            sendKey: $this->send_key,
        );
    }

    /**
     * A 1.x expiration, as this client's.
     *
     * 1.x used negative numbers as sentinels — `-1` never expires, `-2` leaves the
     * TTL alone. This client names them, and refuses a negative number precisely
     * so that a mix-up is an error rather than a record with the wrong lifetime.
     * The mapping is done here, once, where the old convention is documented.
     */
    private static function expirationOf(mixed $expiration): ?Expiration
    {
        if ($expiration === null || $expiration instanceof Expiration) {
            return $expiration;
        }
        if (!is_int($expiration)) {
            throw new \InvalidArgumentException(
                'expiration must be an int of seconds or an Aerospike\Expiration, but is '
                . get_debug_type($expiration)
            );
        }
        return match (true) {
            $expiration === -1 => Expiration::never(),
            $expiration === -2 => Expiration::dontUpdate(),
            $expiration === 0 => Expiration::namespaceDefault(),
            $expiration > 0 => Expiration::seconds($expiration),
            default => throw new \InvalidArgumentException(sprintf(
                'expiration %d is not one the server understands. 1.x used -1 for "never" and '
                . '-2 for "do not update"; both are handled, and any other negative number was '
                . 'never meaningful',
                $expiration
            )),
        };
    }
}

/** 1.x {@see \Aerospike\QueryPolicy}, mutable. */
class QueryPolicy
{
    use PolicyFields;

    public ?int $max_records = null;
    public ?int $records_per_second = null;
    public ?bool $include_bin_data = null;

    public function getMaxRecords(): ?int
    {
        return $this->max_records;
    }

    public function setMaxRecords(int $max): static
    {
        $this->max_records = $max;
        return $this;
    }

    public function getRecordsPerSecond(): ?int
    {
        return $this->records_per_second;
    }

    public function setRecordsPerSecond(int $rate): static
    {
        $this->records_per_second = $rate;
        return $this;
    }

    public function getIncludeBinData(): ?bool
    {
        return $this->include_bin_data;
    }

    public function setIncludeBinData(bool $include): static
    {
        $this->include_bin_data = $include;
        return $this;
    }

    /** The immutable policy this describes. */
    public function build(): NewQueryPolicy
    {
        [$filter, $filterExp] = $this->filterParts();
        return new NewQueryPolicy(
            totalTimeoutMs: $this->total_timeout,
            socketTimeoutMs: $this->socket_timeout,
            maxRetries: $this->max_retries,
            readModeAp: $this->read_mode_ap,
            readModeSc: $this->read_mode_sc,
            useCompression: $this->use_compression,
            filter: $filter,
            filterExp: $filterExp,
            maxRecords: $this->max_records,
            recordsPerSecond: $this->records_per_second,
            includeBinData: $this->include_bin_data,
        );
    }
}

/**
 * 1.x `ScanPolicy`, mutable.
 *
 * A scan is a query with no filter in this client, so this builds a
 * `QueryPolicy`. `maxConcurrentNodes` and `recordQueueSize` are accepted and inert:
 * the daemon owns both — it holds one cursor and answers one page per round trip,
 * so there is no client-side queue to size and no per-call node concurrency.
 */
class ScanPolicy extends QueryPolicy
{
    public ?int $max_concurrent_nodes = null;
    public ?int $record_queue_size = null;

    public function getMaxConcurrentNodes(): ?int
    {
        return $this->max_concurrent_nodes;
    }

    public function setMaxConcurrentNodes(int $nodes): static
    {
        $this->max_concurrent_nodes = $nodes;
        return $this;
    }

    public function getRecordQueueSize(): ?int
    {
        return $this->record_queue_size;
    }

    public function setRecordQueueSize(int $size): static
    {
        $this->record_queue_size = $size;
        return $this;
    }
}

/**
 * 1.x `BatchPolicy`, mutable.
 *
 * Builds a `ReadPolicy`, which is what this client's `batch()` takes as its parent
 * policy — the per-row settings live on the rows. `allowInline`, `allowInlineSsd`
 * and `concurrency` are accepted and inert: the daemon decides those, from its own
 * configuration, because they are properties of the host rather than of a call.
 */
class BatchPolicy extends ReadPolicy
{
    public ?bool $allow_inline = null;
    public ?bool $allow_inline_ssd = null;
    public mixed $concurrency = null;

    public function getAllowInline(): ?bool
    {
        return $this->allow_inline;
    }

    public function setAllowInline(bool $allow): static
    {
        $this->allow_inline = $allow;
        return $this;
    }

    public function getAllowInlineSsd(): ?bool
    {
        return $this->allow_inline_ssd;
    }

    public function setAllowInlineSsd(bool $allow): static
    {
        $this->allow_inline_ssd = $allow;
        return $this;
    }

    public function getConcurrency(): mixed
    {
        return $this->concurrency;
    }

    public function setConcurrency(mixed $concurrency): static
    {
        $this->concurrency = $concurrency;
        return $this;
    }
}

/**
 * 1.x `InfoPolicy`, which carried a timeout and nothing else.
 *
 * This client calls the equivalent `AdminPolicy` — a UDF registration or an index
 * creation is one short info exchange, so retries and replica choice have nothing
 * to act on. 1.x passed a `WritePolicy` to some of those verbs and an `InfoPolicy`
 * to `truncate()`; both map here.
 */
class InfoPolicy
{
    public ?int $timeout = null;

    public function getTimeout(): ?int
    {
        return $this->timeout;
    }

    public function setTimeout(int $millis): static
    {
        $this->timeout = $millis;
        return $this;
    }

    public function build(): NewAdminPolicy
    {
        return new NewAdminPolicy(timeoutMs: $this->timeout);
    }
}

/** 1.x `AdminPolicy`, mutable. */
class AdminPolicy extends InfoPolicy
{
}

/**
 * Whatever policy a 1.x call passed, as the immutable one this client wants.
 *
 * Accepts a builder from this namespace, an already-immutable policy, or `null`.
 * Used by {@see Client} on every verb so old code needs no `->build()` calls.
 */
function built(mixed $policy): mixed
{
    if ($policy === null) {
        return null;
    }
    if (is_object($policy) && method_exists($policy, 'build')) {
        return $policy->build();
    }
    return $policy;
}
