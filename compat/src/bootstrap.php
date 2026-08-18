<?php

/**
 * Load the 1.x compatibility layer.
 *
 * ```php
 * require_once 'aerospike-php/compat/src/bootstrap.php';
 * ```
 *
 * Requires the extension to be loaded already — everything here builds on its
 * classes, and failing loudly now is better than a confusing error at the first
 * command.
 */

declare(strict_types=1);

if (!extension_loaded('aerospike-php')) {
    throw new \RuntimeException(
        'the Aerospike PHP compatibility layer needs the aerospike-php extension loaded first; '
        . 'it wraps that extension rather than replacing it'
    );
}

require_once __DIR__ . '/aliases.php';
require_once __DIR__ . '/enums.php';
require_once __DIR__ . '/result_code.php';
require_once __DIR__ . '/collections.php';
require_once __DIR__ . '/policies.php';
require_once __DIR__ . '/ops.php';
require_once __DIR__ . '/client.php';
