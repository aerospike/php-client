<?php

namespace Aerospike;

class AerospikeHelper
{
    protected static $client;
    protected static $namespace = "test";
    protected static $socket = "/tmp/asld_grpc.sock";
    protected static $keyCount = 100;
    protected static $set = "queryTestSet";
    protected static $keys = [];
    protected static $indexName;
    protected static $indexName2;
    protected static $indexName3;

    public static function connect()
    {
        try {
            self::$client = Client::connect(self::$socket);
        } catch (AerospikeException $e) {
            throw $e;
        }
    }


    public static function setUp()
    {
        echo "\n Set up started ..";
        $wp = new WritePolicy();

        for ($i = 0; $i < self::$keyCount; $i++) {
            $key = new Key(self::$namespace, self::$set, self::randomString(random_int(1, 50) + $i));
            // echo "Key Digest: ". $keyDigest;
            $keyString = $key->digest;
            self::$keys[$keyString] = $key;

            $bins = [
                new Bin("AerospikeBin1", 46),
                new Bin("AerospikeBin2", "randomString12"),
                new Bin("AerospikeBin3", 987),
                new Bin("AerospikeBin4", "constVal"),
                new Bin("AerospikeBin5", -1),
                new Bin("AerospikeBin6", 1),
                new Bin("AerospikeBin7", null)
            ];
            $bins[7] = new Bin("AerospikeBin7", $i % 3);
            self::$client->put($wp, $key, $bins);
        }

        self::$indexName = self::$set . "AerospikeBin3";
        self::$client->createIndex($wp, self::$namespace, self::$set, "AerospikeBin3", self::$indexName, IndexType::Numeric());

        self::$indexName2 = self::$set . "AerospikeBin6";
        self::$client->createIndex($wp, self::$namespace, self::$set, "AerospikeBin6", self::$indexName2, IndexType::Numeric());

        self::$indexName3 = self::$set . "AerospikeBin7";
        self::$client->createIndex($wp, self::$namespace, self::$set, "AerospikeBin7", self::$indexName3, IndexType::Numeric());
        usleep(1000000);
        echo "\n Set up completed ..";
    }

    public static function tearDown()
    {
        echo "\n tear down started..";
        $wp = new WritePolicy();
        self::$indexName = self::$set . "AerospikeBin3";
        self::$client->dropIndex($wp, self::$namespace, self::$set, self::$indexName);
        self::$indexName = self::$set . "AerospikeBin6";
        self::$client->dropIndex($wp, self::$namespace, self::$set, self::$indexName);
        self::$indexName = self::$set . "AerospikeBin7";
        self::$client->dropIndex($wp, self::$namespace, self::$set, self::$indexName);

        $ip = new InfoPolicy();
        self::$client->truncate($ip, self::$namespace, self::$set);
        echo "\n tear down completed..";
    }

    public static function queryAllPartitionRecordsWithFilter()
    {
        echo "\n Query start up..";
        $pf = PartitionFilter::all();
        $qp = new QueryPolicy();
        $qp->setMaxRetries(10);
        $rangeFilter = Filter::Range('randomIndex', 1, 2);
        $statement = new Statement(self::$namespace, self::$set, $rangeFilter);
        $recordSet = self::$client->query($qp, $pf, $statement);

        $counter = 0;
        $keyCount1 = 0;

        while ($rec = $recordSet->next()) {
            $keyString = $rec->key->digest;
            if(array_key_exists($keyString, self::$keys)){
                $keyCount1++;
            }
            unset(self::$keys[$keyString]);
            $counter++;
        }

        echo "\n Total records: " . $counter . "\n";
        echo "\n Total keys: ". $keyCount1;
        echo "\n Query completed..";
    }

    public static function printAllKeys()
    {
        echo "\n Count of all keys - ".count(self::$keys);
    }

    protected static function randomString($length)
    {
        $randomBytes = random_bytes($length);
        $randomString = base64_encode($randomBytes);
        $randomString = preg_replace('/[^a-zA-Z0-9]/', '', $randomString);
        $randomString = substr($randomString, 0, $length);

        return $randomString;
    }
}

AerospikeHelper::connect();
// AerospikeHelper::setUp();
// AerospikeHelper::queryAllPartitionRecordsWithFilter();
// AerospikeHelper::printAllKeys();
AerospikeHelper::tearDown();
