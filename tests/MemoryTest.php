<?php

namespace Aerospike;

use PHPUnit\Framework\TestCase;

final class MemoryTest extends TestCase
{
    protected static Client $client;

    protected static string $namespace = 'test';
    protected static string $set = 'test';
    protected static string $socket = '/tmp/asld_grpc.sock';

    public static function setUpBeforeClass(): void
    {
        self::$client = Client::connect(self::$socket);
    }

    public function testBlobOnPut(): void
    {
        $memoryAtStart = memory_get_usage();

        $this->doTest();

        $memoryUsage = memory_get_usage() - $memoryAtStart;

        // Memory difference must be less than 1KB
        $this->assertLessThan(
            1024,
            $memoryUsage,
            sprintf('Memory leak detected! Expected memory increase is less than 1024, but got: %s bytes', $memoryUsage)
        );
    }

    private function doTest(): void
    {
        $wp = new WritePolicy();
        $key = new Key(self::$namespace, self::$set, 1);

        for ($i = 0; $i < 10_000; $i++) {
            self::$client->put($wp, $key, [new Bin('binName', Value::blob('some value'))]);
        }
    }
}
