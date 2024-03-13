<?php

namespace Aerospike;
use PhpBench\Attributes as Bench;


class AerospikeBenchmark{ 


    protected static $client;
    protected static $namespace = "test";
    protected static $set = "test";
    protected static $socket = "/tmp/asld_grpc.sock";

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$socket);
        } catch (Exception $e) {
            throw $e;
        }
    }

    /**
     * @Iterations(10000)
     * @Revs(100)
     */
    public function benchPutAndGet(): void
    {
        $binString = new Bin("stringBin", "StringData");
        $newKey = new Key(self::$namespace, self::$set, rand(1, 1000));

        $wp = new WritePolicy();
        self::$client->put($wp, $newKey, [$binString]);

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["stringBin"]);
    }
}

$bench = new AerospikeBenchmark();
$bench->setUpBeforeClass();
$bench->benchPutAndGet();