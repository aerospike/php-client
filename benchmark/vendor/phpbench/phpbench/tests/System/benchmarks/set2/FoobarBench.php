<?php

/*
 * This file is part of the PHPBench package
 *
 * (c) Daniel Leech <daniel@dantleech.com>
 *
 * For the full copyright and license information, please view the LICENSE
 * file that was distributed with this source code.
 *
 */

namespace PhpBench\Tests\System\benchmarks\foobar;

/**
 * This benchmark requires the bootstrap/foobar_bootstrap.php to be loaded in
 * order that Foobar be defined.
 */
class FoobarBench
{
    public function benchFoobar(): void
    {
        new \Test\Foobar();
    }
}
