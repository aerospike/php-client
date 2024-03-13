<?php

namespace Aerospike;
use PhpBench\Attributes as Bench;

class AerospikeBenchmark { 

    protected static $client;
    protected static $namespace = "test";
    protected static $set = "test";
    protected static $socket = "/tmp/asld_grpc.sock";

    public static function setUpBeforeClass(): void {
        try {
            self::$client = Client::connect(self::$socket);
        } catch (\Exception $e) {
            throw $e;
        }
    }

    /**
     * @Iterations(5)
     * @Revs(1)
     */
    public function benchGet(): void {
        $this->benchGetString1();
        $this->benchGetString10();
        $this->benchGetString100();
        $this->benchGetString1000();
        $this->benchGetString10000();
        $this->benchGetInteger32();
        $this->benchGetInteger64();
    }

    public function benchGetString1(): void {
        $set = "Benchmark_Get_String1";
        $value = str_repeat("s", 1);
        $this->makeDataForGetBench($set, "b", $value);
        gc_collect_cycles();
        $this->doGet($set);
    }

    public function benchGetString10(): void {
        $set = "Benchmark_Get_String10";
        $value = str_repeat("s", 10);
        $this->makeDataForGetBench($set, "b", $value);
        gc_collect_cycles();
        $this->doGet($set);
    }

    public function benchGetString100(): void {
        $set = "Benchmark_Get_String100";
        $value = str_repeat("s", 100);
        $this->makeDataForGetBench($set, "b", $value);
        gc_collect_cycles();
        $this->doGet($set);
    }

    public function benchGetString1000(): void {
        $set = "Benchmark_Get_String1000";
        $value = str_repeat("s", 1000);
        $this->makeDataForGetBench($set, "b", $value);
        gc_collect_cycles();
        $this->doGet($set);
    }

    public function benchGetString10000(): void {
        $set = "Benchmark_Get_String10000";
        $value = str_repeat("s", 10000);
        $this->makeDataForGetBench($set, "b", $value);
        gc_collect_cycles();
        $this->doGet($set);
    }

    public function benchGetInteger32(): void {
        $set = "Benchmark_Get_Integer32";
        $value = 2147483647; // Maximum value for a 32-bit signed integer
        $this->makeDataForGetBench($set, "b", $value);
        gc_collect_cycles();
        $this->doGet($set);
    }

    public function benchGetInteger64(): void {
        $set = "Benchmark_Get_Integer64";
        $value = 9223372036854775807; // Maximum value for a 64-bit signed integer
        $this->makeDataForGetBench($set, "b", $value);
        gc_collect_cycles();
        $this->doGet($set);
    }

    public static function makeDataForGetBench($set, $binName, $binValue) {
        $wp = new WritePolicy();
        for ($k = 0; $k < 1000; $k++) {
            $key = new Key(self::$namespace, $set, $k);
            self::$client->put($wp, $key, [new Bin($binName, $binValue)]);
        }
    }

    public function doGet($set) {
        $rp = new ReadPolicy();
        for ($i = 0; $i < 100; $i++) {
            $key = new Key(self::$namespace, $set, 0);
            $record = self::$client->get($rp, $key);
        }
    }
}

$bench = new AerospikeBenchmark();
$bench->setUpBeforeClass();
$bench->benchGet();