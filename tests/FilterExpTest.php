<?php 

namespace Aerospike;
use PHPUnit\Framework\TestCase;

final class FilterExpTest extends TestCase
{   

    protected static $namespace = "test";
    protected static $set = "test";
    protected static $host = "127.0.0.1:3000";

    public static function setUpBeforeClass(): void
    {
        $cp = new ClientPolicy();

        try {
            $client = Aerospike($cp, self::$host);
            $client->truncate(self::$namespace, self::$set);
            $wp = new WritePolicy();
            
            for ($i = 0; $i < 100; $i++) {
                $key = new Key(self::$namespace, self::$set, $i);
                $iBin = new Bin("ibin", $i);
                $sbin = new Bin("sbin", strval($i));
                $client->put($wp, $key, [$iBin, $sbin]);
            }
        } catch (Exception $e) {
            throw $e;
        }
    }

    public function testEqFilter()
    {   
        echo "TEST EQ FILTER \n";
        $filter = FilterExpression::eq(FilterExpression::intBin("ibin"), FilterExpression::intVal(1));
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(1, $result);
    }

    public function testNeFilter()
    {   
        echo "TEST NE FILTER \n";
        $filter = FilterExpression::ne(FilterExpression::intBin("ibin"), FilterExpression::intVal(1));
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(99, $result);
    }

    public function testLtFilter()
    {   
        echo "TEST LT FILTER \n";
        $filter = FilterExpression::lt(FilterExpression::intBin("ibin"), FilterExpression::intVal(10));
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(10, $result);
    }

    public function testGtFilter()
    {   
        echo "TEST GT FILTER \n";
        $filter = FilterExpression::gt(FilterExpression::intBin("ibin"), FilterExpression::intVal(1));
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(98, $result);
    }

    public function testLeFilter()
    {   
        echo "TEST LE FILTER \n";
        $filter = FilterExpression::le(FilterExpression::intBin("ibin"), FilterExpression::intVal(10));
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(11, $result);
    }

    public function testGeFilter()
    {   
        echo "TEST GE FILTER \n";
        $filter = FilterExpression::le(FilterExpression::intBin("ibin"), FilterExpression::intVal(1));
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(99, $result);
    }



    public function testAndFilter()
    {   
        echo "TEST AND FILTER \n";
        $filter = FilterExpression::and([FilterExpression::eq(FilterExpression::intBin("ibin"), FilterExpression::intVal(1)), 
                                        FilterExpression::eq(FilterExpression::stringBin("sbin"), FilterExpression::stringVal("1"))]);
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(1, $result);
    }


    public function testOrFilter()
    {   
        echo "TEST OR FILTER \n";
        $filter = FilterExpression::or([FilterExpression::eq(FilterExpression::intBin("ibin"), FilterExpression::intVal(1)), 
                                        FilterExpression::eq(FilterExpression::intBin("ibin"), FilterExpression::intVal(3))]);
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(99, $result);
    }

    public function testNotFilter()
    {   
        echo "TEST NOT FILTER \n";
        $filter = FilterExpression::not(FilterExpression::eq(FilterExpression::intBin("ibin"), FilterExpression::intVal(1)));
        $cp = new ClientPolicy();
        $client = Aerospike($cp, self::$host);
        $recordset = $this->testFilter($client, $filter);
        $result = $this->countResult($recordset);
        $this->assertEquals(99, $result);
    }


    private function testFilter($client, $filter)
    {
        $qpolicy = new QueryPolicy();
        $qpolicy->filterExpression = $filter;

        $statement = new Statement(self::$namespace, self::$set);
        $recordset = $client->query($qpolicy, $statement);

        return $recordset;
    }

    private function countResult($recordset) 
    {
        $count = 0;
        while ($rec = $recordset->next()) {
            var_dump($rec);
            $count += 1;
        }
        return $count;
    }
}