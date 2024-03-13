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

namespace PhpBench\Extensions\XDebug\Tests\System;

use PhpBench\Tests\System\SystemTestCase;
use Symfony\Component\Process\Process;

class XDebugTestCase extends SystemTestCase
{
    protected function setUp(): void
    {
        if (getenv('SKIP_XDEBUG_TESTS')) {
            $this->markTestSkipped('Tests disabled by CI.');
        }

        if (!extension_loaded('xdebug')) {
            $this->markTestSkipped('XDebug not enabled.');
        }

        parent::setUp();
    }

    public function phpbench($command, $workingDir = '.'): Process
    {
        $command .= ' --extension="PhpBench\\Extensions\\XDebug\\XDebugExtension"';

        return parent::phpbench($command, $workingDir);
    }
}
