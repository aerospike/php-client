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

namespace PhpBench\Tests\Unit\Storage\UuidResolver;

use PhpBench\Registry\Registry;
use PhpBench\Storage\DriverInterface;
use PhpBench\Storage\HistoryEntry;
use PhpBench\Storage\HistoryIteratorInterface;
use PhpBench\Storage\UuidResolver\LatestResolver;
use PhpBench\Tests\TestCase;
use RuntimeException;

class LatestResolverTest extends TestCase
{
    private $resolver;
    private $storage;
    private $history;
    private $historyEntry;
    private $historyEntry1;

    protected function setUp(): void
    {
        $registry = $this->prophesize(Registry::class);
        $this->storage = $this->prophesize(DriverInterface::class);
        $registry->getService()->willReturn($this->storage->reveal());
        $this->history = $this->prophesize(HistoryIteratorInterface::class);
        $this->historyEntry = $this->prophesize(HistoryEntry::class);
        $this->historyEntry1 = $this->prophesize(HistoryEntry::class);

        $this->resolver = new LatestResolver(
            $registry->reveal()
        );
    }

    /**
     * It should resove the "latest" token.
     */
    public function testResolveLatest(): void
    {
        $this->storage->history()->willReturn($this->history->reveal());
        $this->history->current()->willReturn($this->historyEntry->reveal());
        $this->historyEntry->getRunId()->willReturn(1234);

        $ref = $this->resolver->resolve('latest');
        $this->assertEquals(1234, $ref);
    }

    public function testNullWhenNotBeginningWithLatest(): void
    {
        $this->storage->history()->willReturn($this->history->reveal());
        $this->history->current()->willReturn($this->historyEntry->reveal());
        $this->historyEntry->getRunId()->willReturn(1234);

        $ref = $this->resolver->resolve('foobar');
        $this->assertEquals(null, $ref);
    }

    public function testResolveCantResolve(): void
    {
        $this->expectException(RuntimeException::class);
        $this->resolver->resolve('latest-nutbar');
    }

    /**
     * It should return the nth history run using the minus operator.
     */
    public function testLatestMinusNth(): void
    {
        $this->storage->history()->willReturn($this->history->reveal());
        $this->history->current()->willReturn(
            $this->historyEntry->reveal(),
            $this->historyEntry->reveal(),
            $this->historyEntry1->reveal()
        );
        $this->historyEntry->getRunId()->willReturn(1234);
        $this->historyEntry1->getRunId()->willReturn(4321);

        $this->history->next()->shouldBeCalledTimes(3);

        $ref = $this->resolver->resolve('latest-2');
        $this->assertEquals(4321, $ref);
    }

    /**
     * It should throw an exception if no history is present.
     *
     */
    public function testNoHistory(): void
    {
        $this->expectException(\InvalidArgumentException::class);
        $this->expectExceptionMessage('No history present');
        $this->storage->history()->willReturn($this->history->reveal());
        $this->history->current()->willReturn(false);

        $this->resolver->resolve('latest');
    }
}
