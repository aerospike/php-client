<?php

use Aerospike\Client;
use Aerospike\WritePolicy;
use Aerospike\PartitionFilter;
use Aerospike\ScanPolicy;
use Aerospike\Bin;
use Aerospike\InfoPolicy;
use Aerospike\Key;

use PHPUnit\Framework\TestCase;

class ScanTest extends TestCase
{
    protected static $client;
    protected static $namespace = "test";
    protected static $socket = "/tmp/asld_grpc.sock";
    protected static $keyCount = 100;
    protected static $bins;
    protected static $set;
    protected static $keys = [];

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$socket);
        } catch (AerospikeException $e) {
            throw $e;
        }
        self::$bins = [
            new Bin("AerospikeBin1", 23),
            new Bin("AerospikeBin2", "randomString")
        ];
    }

    protected function setUp(): void
    {
        self::$keys = [];
        $wp = new WritePolicy();
        self::$set = self::randomString(random_int(5, 50));

        $ip = new InfoPolicy();
        self::$client->truncate($ip, self::$namespace, self::$set);

        for ($i = 0; $i < self::$keyCount; $i++) {
            $key = new Key(self::$namespace, self::$set, self::randomString(random_int(1, 50)+$i));
            $keyString = $key->digest;
            self::$keys[$keyString] = $key;
            self::$client->put($wp, $key, self::$bins);
        }
    }

    private function checkResults($recordset, $cancelCount): int {
        $counter = 0;
        $this->assertNotNull($recordset);
        while($rec = $recordset->next()) {
            $keyString = $rec->key->digest;

            $this->assertEquals($rec->bins['AerospikeBin1'], 23);
            $this->assertEquals($rec->bins['AerospikeBin2'], "randomString");
            unset(self::$keys[$keyString]);

            $counter++;

            //cancel scan stream abruptly
            if($cancelCount!= 0 && $counter == $cancelCount){
                $recordset->close();
            }
        }
        $this->assertGreaterThan(0, $counter);
        return $counter;
    }

    public function testScanAndPaginateAllPartitionsConcurrently(){
        $this->assertEquals(count(self::$keys), self::$keyCount);
        $pf = PartitionFilter::all();
        $sp = new ScanPolicy();
        $sp->maxRecords = 20;

        $times = 0;
        $received = 0;
        while ($received < self::$keyCount) {
            $times++;
            $recordset = self::$client->scan($sp, $pf, self::$namespace, self::$set);
            $this->assertNotNull($recordset);

            $recs = self::checkResults($recordset, 0);
            $this->assertLessThanOrEqual($recs, $sp->maxRecords);

            $received += $recs;
        }
        $this->assertLessThanOrEqual($recs, $sp->maxRecords);
        $this->assertEquals(count(self::$keys), 0);
    }

    public function testScanAllPartitionsOneByOne(){
        $pf = PartitionFilter::all();
        $sp = new ScanPolicy();
        $sp->maxRecords = 1;

        $times = 0;
        $received = 0;
        while ($received < self::$keyCount) {
            $times++;
            $recordset = self::$client->scan($sp, $pf, self::$namespace, self::$set);
            $this->assertNotNull($recordset);

            $recs = self::checkResults($recordset, 0);
            $this->assertLessThanOrEqual($recs, $sp->maxRecords);
            $received += $recs;
        }
    }

    public function testScanAllPartitions(){
        $pf = PartitionFilter::range(0, 4096);
        $sp = new ScanPolicy();
        $sp->maxRecords = 20;

        $times = 0;
        $received = 0;
        while ($received < self::$keyCount) {
            $times++;
            $recordset = self::$client->scan($sp, $pf, self::$namespace, self::$set);
            $this->assertNotNull($recordset);

            $recs = self::checkResults($recordset, 0);
            $received += $recs;
        }
    }

    public function testScanMustCancel(){
        $pf = PartitionFilter::range(0, 4096);
        $sp = new ScanPolicy();
        $sp->maxRecords = 20;

        $times = 0;
        $received = 0;
        while ($received < self::$keyCount) {
            $times++;
            $recordset = self::$client->scan($sp, $pf, self::$namespace, self::$set);
            $this->assertNotNull($recordset);

            $recs = self::checkResults($recordset, self::$keyCount/2);
            $received += $recs;
        }
    }

    function randomString($length) {
        $randomBytes = random_bytes($length);

        $randomString = base64_encode($randomBytes);

        $randomString = preg_replace('/[^a-zA-Z0-9]/', '', $randomString);
        $randomString = substr($randomString, 0, $length);

        return $randomString;
    }

}