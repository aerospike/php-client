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
     * @BeforeMethods("makeDataForGetBenchString1")
     */
    public function benchGetString1(): void {
        $set = "Benchmark_Get_String1";
        $this->doGet($set);
    }

    public function makeDataForGetBenchString1() {
        $set = "Benchmark_Get_String1";
        $value = str_repeat("s", 1);
        $wp = new WritePolicy();
        for ($k = 0; $k < 1000; $k++) {
            $key = new Key(self::$namespace, $set, $k);
            self::$client->put($wp, $key, [new Bin("b", $value)]);
        }
        gc_collect_cycles();
    }

    /**
     * @Iterations(5)
     * @BeforeMethods("makeDataForGetBenchString10")
     */
    public function benchGetString10(): void {
        $set = "Benchmark_Get_String10";
        $this->doGet($set);
    }
    public function makeDataForGetBenchString10() {
        $set = "Benchmark_Get_String1";
        $value = str_repeat("s", 10);
        $wp = new WritePolicy();
        for ($k = 0; $k < 1000; $k++) {
            $key = new Key(self::$namespace, $set, $k);
            self::$client->put($wp, $key, [new Bin("b", $value)]);
        }
        gc_collect_cycles();
    }

    /**
     * @Iterations(5)
     * @BeforeMethods("makeDataForGetBenchString100")
     */
    public function benchGetString100(): void {
        $set = "Benchmark_Get_String100";
        $this->doGet($set);
    }
    public function makeDataForGetBenchString100() {
        $set = "Benchmark_Get_String1";
        $value = str_repeat("s", 100);
        $wp = new WritePolicy();
        for ($k = 0; $k < 1000; $k++) {
            $key = new Key(self::$namespace, $set, $k);
            self::$client->put($wp, $key, [new Bin("b", $value)]);
        }
        gc_collect_cycles();
    }

    /**
     * @Iterations(5)
     * @BeforeMethods("makeDataForGetBenchString1000")
     */
    public function benchGetString1000(): void {
        $set = "Benchmark_Get_String1000";
        $this->doGet($set);
    }
    public function makeDataForGetBenchString1000() {
        $set = "Benchmark_Get_String1";
        $value = str_repeat("s", 1000);
        $wp = new WritePolicy();
        for ($k = 0; $k < 1000; $k++) {
            $key = new Key(self::$namespace, $set, $k);
            self::$client->put($wp, $key, [new Bin("b", $value)]);
        }
        gc_collect_cycles();
    }

    /**
     * @Iterations(5)
     * @BeforeMethods("makeDataForGetBenchString10000")
     */
    public function benchGetString10000(): void {
        $set = "Benchmark_Get_String10000";
        $this->doGet($set);
    }
    public function makeDataForGetBenchString10000() {
        $set = "Benchmark_Get_String1";
        $value = str_repeat("s", 10000);
        $wp = new WritePolicy();
        for ($k = 0; $k < 1000; $k++) {
            $key = new Key(self::$namespace, $set, $k);
            self::$client->put($wp, $key, [new Bin("b", $value)]);
        }
        gc_collect_cycles();
    }

    /**
     * @Iterations(5)
     * @BeforeMethods("makeDataForGetBenchInt32")
     */
    public function benchGetInteger32(): void {
        $set = "Benchmark_Get_Integer32";
        $this->doGet($set);
    }
    public function makeDataForGetBenchInt32() {
        $set = "Benchmark_Get_Integer32";
        $value = 2147483647; 
        $wp = new WritePolicy();
        for ($k = 0; $k < 1000; $k++) {
            $key = new Key(self::$namespace, $set, $k);
            self::$client->put($wp, $key, [new Bin("b", $value)]);
        }
        gc_collect_cycles();
    }

    /**
     * @Iterations(5)
     * @BeforeMethods("makeDataForGetBenchInt64")
     */
    public function benchGetInteger64(): void {
        $set = "Benchmark_Get_Integer64";
        $this->doGet($set);
    }
    public function makeDataForGetBenchInt64() {
        $set = "Benchmark_Get_Integer64";
        $value = 9223372036854775807; 
        $wp = new WritePolicy();
        for ($k = 0; $k < 1000; $k++) {
            $key = new Key(self::$namespace, $set, $k);
            self::$client->put($wp, $key, [new Bin("b", $value)]);
        }
        gc_collect_cycles();
    }

    public function doGet($set) {
        $rp = new ReadPolicy();
        $key = new Key(self::$namespace, $set, rand(0,100));
        $record = self::$client->get($rp, $key);
    }
}

$bench = new AerospikeBenchmark();
$bench->setUpBeforeClass();
