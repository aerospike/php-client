<?php

namespace Aerospike;

use PHPUnit\Framework\TestCase;

class CDTMapOpTest extends TestCase{
    protected static $client;
    protected static $namespace = "test";
    protected static $socket = "/tmp/asld_grpc.sock";
    protected static $set;
    protected static $key;
    protected static $cdtBinName;

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$socket);
        } catch (AerospikeException $e) {
            throw $e;
        }
    }

    protected function setUp(): void
    {
        self::$set = self::randomString(random_int(5, 10));
        $ip = new InfoPolicy();
        self::$client->truncate($ip, self::$namespace, self::$set);

        self::$key = new Key(self::$namespace, self::$set, self::randomString(random_int(5, 10)));
        self::$cdtBinName = self::randomString(random_int(5, 10));
    }

    protected function randomString($length) {
        $randomBytes = random_bytes($length);
    
        $randomString = base64_encode($randomBytes);

        $randomString = preg_replace('/[^a-zA-Z0-9]/', '', $randomString);
        $randomString = substr($randomString, 0, $length);
        
        return $randomString;
    }

    public function testShouldCreateValidCDTMap(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        $mp = new MapPolicy(MapOrderType::unordered());

        $ops = [MapOp::put($mp, self::$cdtBinName, ["a" => 1, "b" => 2, "c" => 3, "d" => 4, "e" => 5, "f" => 6])];
        
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals($record->bins[self::$cdtBinName], ["a" => 1, "b" => 2, "c" => 3, "d" => 4, "e" => 5, "f" => 6]);
    }

    public function testShouldUnpackUnOrderedCDTMap(){
        
    }

    public function testShouldUnpackOrderedCDTMap(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        $mp = new MapPolicy(MapOrderType::KeyValueOrdered(), MapWriteFlags::UpdateOnly());
        $map = [
            "mk1" => ["v1.0", "v1.1"],
            "mk2" => ["v2.0", "v2.1"]
        ];
        $ops = [MapOp::put($mp, self::$cdtBinName, $map)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);

        $brp = new BatchReadPolicy();
        $ops = [MapOp::getByKeys($mp, self::$cdtBinName, ["mk1"], MapReturnType::value())];
        $br = BatchRead::ops($brp, self::$key, $ops);
        $recs = self::$client->batch($bp, [$br]);
        var_dump($recs);

        
    }

    public function testShouldReturnOrderedCDTMap(){
        
    }


}