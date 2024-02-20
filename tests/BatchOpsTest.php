<?php 

namespace Aerospike;
use PHPUnit\Framework\TestCase;

final class BatchOpsTest extends TestCase
{
    protected static $client;

    protected static $namespace = "test";
    protected static $set = "test";
    protected static $socket = "/tmp/asld_grpc.sock";

    public static function generateRandomReport() {
        $shape = ["circle", "triangle", "square"];
        $summary = "Summary " . mt_rand(1000, 9999);
        $city = "City " . mt_rand(1, 100);
        $state = "State " . mt_rand(1, 50);
        $duration = mt_rand(1, 24) . " hours";
        return [
            "shape" => $shape,
            "summary" => $summary,
            "city" => $city,
            "state" => $state,
            "duration" => $duration
        ];
    }

    public static function setUpBeforeClass(): void
    {
        $numRecords = 10;
        try {
            self::$client = Client::connect(self::$socket);
        } catch (AerospikeException $e) {
            throw $e;
        }
        for ($i = 1; $i <= $numRecords; $i++) {
            $key = new Key(self::$namespace, self::$set, "record_$i");
        
            // Generate random data for Occurred, Reported, and Posted fields
            $occurred = mt_rand(1000000000, time());
            $reported = mt_rand(1000000000, time());
            $posted = mt_rand(1000000000, time());
        
            // Generate random data for the Report map
            $reportData = self::generateRandomReport();
        
            // Define bins
            $bins = [
                new Bin("Occurred", $occurred),
                new Bin("Reported", $reported),
                new Bin("Posted", $posted),
                new Bin("Report", $reportData)
            ];
        
            // Write the record
            $wp = new WritePolicy();
            self::$client->put($wp, $key, $bins);
        }
    }

    public function testBatchOpsRead(){
        $brp = new BatchReadPolicy();

        $brkey = new Key(self::$namespace, self::$set, 1);
        $batchRead = new BatchRead($brp, $brkey, []);
        
        $bp = new BatchPolicy();
        $recs = self::$client->batch($bp, [$batchRead]);
        $this->assertIsArray($recs);
    }

    public function testBatchOpsWrite(){

        $stringKey = new Key(self::$namespace, self::$set, "string_key");
        $bwp = new BatchWritePolicy();
        $ops = [Operation::put(new Bin("ibin", 10)), Operation::put(new Bin("sbin", "string_val"))];
        $bw = new BatchWrite($bwp, $stringKey, $ops);
        
        $bp = new BatchPolicy();
        $recs = self::$client->batch($bp, [$bw]);
        $this->assertIsArray($recs);
    }

    public function testBatchOpsDelete(){
        $bdp = new BatchDeletePolicy();

        $bdkey = new Key(self::$namespace, self::$set, "string_key");
        $batchDelete = new BatchDelete($bdp, $bdkey);
        
        $bp = new BatchPolicy();
        self::$client->batch($bp, [$batchDelete]);

        $rp  = new ReadPolicy();
        $this->assertFalse(self::$client->exists($rp, $bdkey));
    }

    public function testInvalidBatchCmd(){
        $this->expectException(AerospikeException::class);
        $batchOp = [];
        $bp = new BatchPolicy();
        self::$client->batch($bp, [$batchOp]);
    }

    public function testBatchReadWrite(){
        $brp = new BatchReadPolicy();
        $bwp = new BatchWritePolicy();
        $batchKey = new Key(self::$namespace, self::$set, "batch_key");
        $ops = [Operation::put(new Bin("ibin", 10)), Operation::put(new Bin("sbin", "string_val"))];
        $batchRead = new BatchRead($brp, $batchKey, []);
        $batchWrite = new BatchWrite($bwp, $batchKey, $ops);

        $bp = new BatchPolicy();
        $batchRecords = self::$client->batch($bp, [$batchWrite, $batchRead]);
        $this->assertIsArray($batchRecords);
    }

    public function testBatchWriteMultipleOpsAppend(){
        $bwp = new BatchWritePolicy();
        $batchKey = new Key(self::$namespace, self::$set, "batch_key");
        $wp = new WritePolicy();
        self::$client->put($wp, $batchKey, [new Bin("sbinapp", "db")]);
        $ops = [Operation::append(new Bin("sbinapp", "aerospike_"))];
        $batchWrite = new BatchWrite($bwp, $batchKey, $ops);

        $bp = new BatchPolicy();
        self::$client->batch($bp, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $batchKey);
        $bins = $recs->getBins();

        $this->assertEquals("dbaerospike_", $bins["sbinapp"]);
    }

    public function testBatchWriteMultipleOpsPrepend(){
        $bwp = new BatchWritePolicy();
        $batchKey = new Key(self::$namespace, self::$set, "batch_key");
        $wp = new WritePolicy();
        self::$client->put($wp, $batchKey, [new Bin("sbinpre", "db")]);
        $ops = [Operation::prepend(new Bin("sbinpre", "aerospike_"))];
        $batchWrite = new BatchWrite($bwp, $batchKey, $ops);

        $bp = new BatchPolicy();
        self::$client->batch($bp, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $batchKey);
        $bins = $recs->getBins();

        $this->assertEquals("aerospike_db", $bins["sbinpre"]);
    }

    public function testBatchWriteMultipleOpsAdd(){
        $bwp = new BatchWritePolicy();
        $batchKey = new Key(self::$namespace, self::$set, "batch_key");
        $wp = new WritePolicy();
        self::$client->put($wp, $batchKey, [new Bin("ibin", 1)]);
        $ops = [Operation::add(new Bin("ibin", 20))];
        $batchWrite = new BatchWrite($bwp, $batchKey, $ops);

        $bp = new BatchPolicy();
        self::$client->batch($bp, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $batchKey);
        $bins = $recs->getBins();

        $this->assertEquals(21, $bins["ibin"]);
    }

}
