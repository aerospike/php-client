<?php

use Aerospike\Client;
use Aerospike\WritePolicy;
use Aerospike\PartitionFilter;
use Aerospike\QueryPolicy;
use Aerospike\Bin;
use Aerospike\InfoPolicy;
use Aerospike\Key;
use Aerospike\IndexType;
use Aerospike\Filter;
use Aerospike\Statement;
use Aerospike\AerospikeException;
use Aerospike\QueryDuration;

use PHPUnit\Framework\TestCase;

class QueryTest extends TestCase{

    protected static $client;
    protected static $namespace = "test";
    protected static $socket = "/tmp/asld_grpc.sock";
    protected static $keyCount = 1000;
    protected static $set = "queryTestSet";
    protected static $keys = [];
    protected static $indexName;
    protected static $indexName2;
    protected static $indexName3;
    protected static $bins;

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$socket);
        } catch (AerospikeException $e) {
            throw $e;
        }
        self::$bins = [
            new Bin("AerospikeBin1", 46),
            new Bin("AerospikeBin2", "randomString12"),
            new Bin("AerospikeBin3", 987),
            new Bin("AerospikeBin4", "constVal"),
            new Bin("AerospikeBin5", -1),
            new Bin("AerospikeBin6", 1),
            new Bin("AerospikeBin7", null)
        ];
    }

    protected function setUp(): void
    {
        self::$keys = [];
        $wp = new WritePolicy();
        
        for ($i = 0; $i < self::$keyCount; $i++) {
            $key = new Key(self::$namespace, self::$set, self::randomString(random_int(1, 50)+$i));
            $keyString = $key->digest;
            self::$keys[$keyString] = $key;

            self::$bins[7] = new Bin("AerospikeBin7", $i%3);
            self::$client->put($wp, $key, self::$bins);
        }
        self::$indexName = self::$set . "AerospikeBin3";
        self::$client->createIndex($wp, self::$namespace, self::$set, "AerospikeBin3", self::$indexName, IndexType::Numeric());

        self::$indexName2 = self::$set . "AerospikeBin6";
        self::$client->createIndex($wp, self::$namespace, self::$set, "AerospikeBin6", self::$indexName2, IndexType::Numeric());

        self::$indexName3 = self::$set . "AerospikeBin7";
        self::$client->createIndex($wp, self::$namespace, self::$set, "AerospikeBin7", self::$indexName3, IndexType::Numeric());

        //wait for setup to complete...
        usleep(1000000);
    }

    protected function tearDown(): void
    {
        $wp = new WritePolicy();
        self::$indexName = self::$set . "AerospikeBin3";
        self::$client->dropIndex($wp, self::$namespace, self::$set, self::$indexName);

        self::$indexName = self::$set . "AerospikeBin6";
        self::$client->dropIndex($wp, self::$namespace, self::$set, self::$indexName);

        self::$indexName = self::$set . "AerospikeBin7";
        self::$client->dropIndex($wp, self::$namespace, self::$set, self::$indexName);

        $ip = new InfoPolicy();
        self::$client->truncate($ip, self::$namespace, self::$set);
    }

    protected function checkResults($recordSet, $cancelCount){
        $counter = 0;
        while($rec = $recordSet->next()) {  
            $keyString = $rec->key->digest;
            
            $this->assertEquals($rec->bins['AerospikeBin1'], 46);
            $this->assertEquals($rec->bins['AerospikeBin2'], "randomString12");
            unset(self::$keys[$keyString]);

            $counter++;

            //cancel query stream abruptly
            if($cancelCount!= 0 && $counter == $cancelCount){
                $recordSet->close();
            }
        }
        $this->assertGreaterThan(0, $counter);
    }

    function randomString($length) {
        $randomBytes = random_bytes($length);
    
        $randomString = base64_encode($randomBytes);

        $randomString = preg_replace('/[^a-zA-Z0-9]/', '', $randomString);
        $randomString = substr($randomString, 0, $length);
    
        return $randomString;
    }
    

    public function testQueryToGetAllPartitionRecordsWithFilter(){
        $counter = 0;
        $this->assertEquals(count(self::$keys), self::$keyCount);
        $pf =  PartitionFilter::all();
        $qp = new QueryPolicy();
        $qp->setMaxRetries(20);
        $rangeFilter = Filter::Range('AerospikeBin7', 1 ,2);
        $statement = new Statement(self::$namespace, self::$set, $rangeFilter);

        $recordSet = self::$client->query($qp, $pf, $statement);
        $counter = 0;
        while($rec = $recordSet->next()) {  
            $keyString = $rec->key->digest;
            
            $this->assertEquals($rec->bins['AerospikeBin1'], 46);
            $this->assertEquals($rec->bins['AerospikeBin2'], "randomString12");
            unset(self::$keys[$keyString]);

            $counter++;
        }
        $this->assertEquals(count(self::$keys), 334);
        $this->assertEquals($counter, self::$keyCount - 334);

    }

    public function testQueryIndexNotFound(){
        $this->expectException(AerospikeException::class);
        $pf =  PartitionFilter::all();
        $qp = new QueryPolicy();
        $rangeFilter = Filter::Range(self::randomString(5), 1 ,2);
        $statement = new Statement(self::$namespace, self::$set, $rangeFilter);

        $recordSet = self::$client->query($qp, $pf, $statement);
        while($rec = $recordSet->next()) {  
            $this->assertNull($rec);
        }
    }

    public function testQueryNonIndexedField(){
        $this->expectException(AerospikeException::class);
        $pf =  PartitionFilter::all();
        $qp = new QueryPolicy();
        $rangeFilter = Filter::Range("AerospikeBin2", 1 ,2);
        $statement = new Statement(self::$namespace, self::$set, $rangeFilter);

        $recordSet = self::$client->query($qp, $pf, $statement);
        while($rec = $recordSet->next()) {  
            $this->assertNull($rec);
        }
    }

    public function testQueryAndGetAllRecords(){
        $pf =  PartitionFilter::all();
        $qp = new QueryPolicy();
        $statement = new Statement(self::$namespace, self::$set, null);

        $recordSet = self::$client->query($qp, $pf, $statement);
        self::checkResults($recordSet, 0);
        $this->assertEquals(count(self::$keys), 0);
    }

    public function testQueryMustCancelAbruptly(){
        $pf =  PartitionFilter::all();
        $qp = new QueryPolicy();
        $statement = new Statement(self::$namespace, self::$set, null);

        $recordSet = self::$client->query($qp, $pf, $statement);
        self::checkResults($recordSet, self::$keyCount/2);
        $this->assertGreaterThanOrEqual(count(self::$keys), self::$keyCount/2);
    }

    public function testQueryToGetRecordsWithEqualityFilter(){
        $counter = 0;
        $this->assertEquals(count(self::$keys), self::$keyCount);
        $pf =  PartitionFilter::all();
        $qp = new QueryPolicy();
        $qp->setMaxRetries(20);
        $eqFilter = Filter::Equal('AerospikeBin3', 987);
        $statement = new Statement(self::$namespace, self::$set, $eqFilter);

        $recordSet = self::$client->query($qp, $pf, $statement);
        $counter = 0;
        while($rec = $recordSet->next()) {  
            $this->assertNotNull($rec->bins['AerospikeBin3']);
            $counter++;
        }
        $this->assertGreaterThan(0, $counter);

    }

    public function testQueryWithExpectedDurationSet(){
        $counter = 0;
        $this->assertEquals(count(self::$keys), self::$keyCount);
        $pf =  PartitionFilter::all();
        $qp = new QueryPolicy();
        $qp->setMaxRetries(20);
        $qp->setExpectedDuration(QueryDuration::Short());
        $eqFilter = Filter::Equal('AerospikeBin3', 987);
        $statement = new Statement(self::$namespace, self::$set, $eqFilter);

        $recordSet = self::$client->query($qp, $pf, $statement);
        $counter = 0;
        while($rec = $recordSet->next()) {  
            $this->assertNotNull($rec->bins['AerospikeBin3']);
            $counter++;
        }
        $this->assertGreaterThan(0, $counter);

    }

}