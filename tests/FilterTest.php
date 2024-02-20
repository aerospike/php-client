<?php 

namespace Aerospike;
use PHPUnit\Framework\TestCase;


/*
This feature can be tested once support for Query and Scan are added 


final class FilterTest extends TestCase
{   

    protected static $namespace = "test";
    protected static $set = "test";
    protected static $host = "127.0.0.1:3000";

    public static function setUpBeforeClass(): void
    {
        $cp = new ClientPolicy();

        try {
            $client = Aerospike($cp, self::$host);
            $client->truncate("test", "test");
            $wp = new WritePolicy();
            
            for ($i = 1; $i <= 10; $i++) {
                $key = new Key(self::$namespace, self::$set, $i);
                $ibin = new Bin("ibin", $i);
                $client->put($wp, $key, [$ibin]);
            }

            $circleFormat = '{"type":"AeroCircle","coordinates":[[%f,%f], %f]}';
            $targetString = sprintf($circleFormat, -80.590000, 28.60000, 1000);
            $geoLoc = new Bin("geoLoc", Value::geoJson($targetString));
            $geoKey = new Key(self::$namespace, self::$set, "geoKey");
            $client->put($wp, $geoKey, [$geoLoc]);

        } catch (Exception $e) {
            throw $e;
        }
    }

    public function testRangeFilter(){
        $cp = new ClientPolicy();
        $qp = new QueryPolicy();
        $client = Aerospike($cp, self::$host);

        $client->createIndex("test", "test", "ibin", "test.test.ibin", IndexType::Numeric());
        $statement = new Statement("test", "test", ["ibin"]);
        $statement->filters = [Filter::range("ibin", 1, 10)];
        
        $recordset = $client->query($qp, $statement);
        $count = 0;
        while ($rec = $recordset->next()) {
            $count++;
        }
        $this->assertEquals(10, $count);
    }

    public function testRangeFilterNeg(){
        $cp = new ClientPolicy();
        $qp = new QueryPolicy();
        $client = Aerospike($cp, self::$host);

        $client->createIndex(self::$namespace, self::$set, "ibin", "test.test.ibin", IndexType::Numeric());
        $statement = new Statement(self::$namespace, self::$set, ["ibin"]);
        $statement->filters = [Filter::range("ibin", 11, 21)];
        
        $recordset = $client->query($qp, $statement);
        $count = 0;
        while ($rec = $recordset->next()) {
            $count++;
        }
        $this->assertEquals(0, $count);
    }

    public function testContainsFilter(){
        $cp = new ClientPolicy();
        $qp = new QueryPolicy();
        $client = Aerospike($cp, self::$host);

        $client->createIndex(self::$namespace, self::$set, "ibin", "test.test.ibin", IndexType::Numeric());
        $statement = new Statement(self::$namespace, self::$set, ["ibin"]);
        $statement->filters = [Filter::contains("ibin", 5)];
        
        $recordset = $client->query($qp, $statement);
        $count = 0;
        while ($rec = $recordset->next()) {
            $count++;
        }
        $this->assertEquals(1, $count);
    }

    public function testContainsFilterNeg(){
        $cp = new ClientPolicy();
        $qp = new QueryPolicy();
        $client = Aerospike($cp, self::$host);

        $client->createIndex(self::$namespace, self::$set, "ibin", "test.test.ibin", IndexType::Numeric());
        $statement = new Statement(self::$namespace, self::$set, ["ibin"]);
        $statement->filters = [Filter::contains("ibin", 15)];
        
        $recordset = $client->query($qp, $statement);
        $count = 0;
        while ($rec = $recordset->next()) {
            $count++;
        }
        $this->assertEquals(0, $count);
    }

    public function testContainsRangeFilter(){
        $cp = new ClientPolicy();
        $qp = new QueryPolicy();
        $client = Aerospike($cp, self::$host);

        $client->createIndex(self::$namespace, self::$set, "ibin", "test.test.ibin", IndexType::Numeric());
        $statement = new Statement(self::$namespace, self::$set, ["ibin"]);
        $statement->filters = [Filter::containsRange("ibin", 1, 5)];
        
        $recordset = $client->query($qp, $statement);
        $count = 0;
        while ($rec = $recordset->next()) {
            $count++;
        }
        $this->assertEquals(1, $count);
    }

    public function testContainsRangeFilterNeg(){
        $cp = new ClientPolicy();
        $qp = new QueryPolicy();
        $client = Aerospike($cp, self::$host);

        $client->createIndex(self::$namespace, self::$set, "ibin", "test.test.ibin", IndexType::Numeric());
        $statement = new Statement(self::$namespace, self::$set, ["ibin"]);
        $statement->filters = [Filter::containsRange("ibin", 11, 25)];
        
        $recordset = $client->query($qp, $statement);
        $count = 0;
        while ($rec = $recordset->next()) {
            $count++;
        }
        $this->assertEquals(0, $count);
    }

    public function testGeoRegionsContainingPointFilter(){
        $cp = new ClientPolicy();
        $qp = new QueryPolicy();
        $client = Aerospike($cp, self::$host);

        $client->createIndex(self::$namespace, self::$set, "geoLoc", "test.test.geobin", IndexType::Geo2DSphere());
        $pointString = '{"type":"Point","coordinates":[-80.590003, 28.60009]}';
        $statement = new Statement(self::$namespace, self::$set, ["geoLoc"]);
        $statement->filters = [Filter::regionsContainingPoint("geoLoc", $pointString)];
        
        $recordset = $client->query($qp, $statement);
        $count = 0;
        while ($rec = $recordset->next()) {
            $count++;
        }
        $this->assertEquals(1, $count);
    }

    //TODO: Needs fixing of geo values
    public function testGeoWithinRadiusFilter(){
        $cp = new ClientPolicy();
        $qp = new QueryPolicy();
        $client = Aerospike($cp, self::$host);

        $client->createIndex(self::$namespace, self::$set, "geoLoc", "test.test.geobin", IndexType::Geo2DSphere());
        $statement = new Statement(self::$namespace, self::$set, ["geoLoc"]);
        $statement->filters = [Filter::withinRadius("geoLoc", -80.590003,28.60009, 20)];
        
        $recordset = $client->query($qp, $statement);
        $count = 0;
        while ($rec = $recordset->next()) {
            $count++;
        }
        $this->assertEquals(1, $count);
    }
}
*/