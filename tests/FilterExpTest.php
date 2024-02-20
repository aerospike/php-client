<?php 

namespace Aerospike;
use PHPUnit\Framework\TestCase;

final class FilterExpTest extends TestCase
{   

    protected static $client;
    protected static $namespace = "test";
    protected static $set = "test";
    protected static $host = "/tmp/asld_grpc.sock";

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$host);
            $ip = new InfoPolicy();
            self::$client->truncate($ip, self::$namespace, self::$set);
        } catch (Exception $e) {
            throw $e;
        }
    }

    public function testEqFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 1);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::eq(Expression::intBin("bin1"), Expression::intVal(1));
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(3, count($recs->bins));
    }

    public function testNeFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 2);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("name", "aerospike")]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::ne(Expression::stringBin("name"), Expression::stringVal("aerospike_nosql_db"));
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(2, count($recs->bins));
    }

    public function testLtFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 3);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::lt(Expression::intBin("bin1"), Expression::intVal(1));
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(3, count($recs->bins));
    }

    public function testGtFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 4);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::gt(Expression::intBin("bin1"), Expression::intVal(1));
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(3, count($recs->bins));
    }

    public function testLeFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 3);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::le(Expression::intBin("bin1"), Expression::intVal(1));
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(3, count($recs->bins));
    }

    public function testGeFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 4);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::gt(Expression::intBin("bin1"), Expression::intVal(1));
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(3, count($recs->bins));
    }



    public function testAndFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 4);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::and([Expression::eq(Expression::intBin("bin1"), Expression::intVal(1)), 
        Expression::eq(Expression::intBin("bin2"), Expression::intVal(2))]);
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(3, count($recs->bins));
    }


    public function testOrFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 4);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::or([Expression::eq(Expression::intBin("bin1"), Expression::intVal(1)), 
        Expression::eq(Expression::intBin("bin3"), Expression::intVal(9))]);
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(3, count($recs->bins));
    }

    public function testNotFilter()
    {   
        $key = new Key(self::$namespace, self::$set, 4);
        $wp = new WritePolicy();
        self::$client->put($wp, $key, [new Bin("bin1", 1), new Bin("bin2", 2)]);

        $batchWritePolicy = new BatchWritePolicy();
        $exp = Expression::not(Expression::eq(Expression::intBin("bin1"), Expression::intVal(1)));
        $ops = [Operation::put(new Bin("bin3", 3))];
        $batchWrite = new BatchWrite($batchWritePolicy, $key, $ops);

        $batchPolicy = new BatchPolicy();
        self::$client->batch($batchPolicy, [$batchWrite]);

        $rp = new ReadPolicy();
        $recs = self::$client->get($rp, $key);

        $this->assertEquals(3, count($recs->bins));
    }

}