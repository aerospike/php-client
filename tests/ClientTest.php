<?php

namespace Aerospike;

use PHPUnit\Framework\TestCase;

final class ClientTest extends TestCase
{
    protected static $cp;
    protected static $client;
    protected static $key;

    protected static $namespace = "test";
    protected static $set = "test";
    protected static $socket = "/tmp/asld_grpc.sock";

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$socket);
            self::$key = new Key(self::$namespace, self::$set, 1);
        } catch (Exception $e) {
            throw $e;
        }
    }


    public function testPutGetValues()
    {
        $values = [
            Value::int(-1),
            Value::uint(1),
            Value::float(3.14),
            Value::bool(true),
            Value::string("hello world!"),
            Value::list([1, "hello world", true, 3.14]),
            Value::map(array(1 => true, 2 => false, "hello" => "world", "nil" => Value::nil())),
            Value::blob([1, 3, 5, 7, 9, 24, 255]),
            Value::geoJson("{ \"type\": \"AeroCircle\", \"coordinates\": [[0.0, 0.0], 3000.0 ] }"),
        ];

        foreach ($values as $value) {

            // Prepare a bin with an integer value
            $bin = new Bin("binName", $value);
            // var_dump($value);

            // Create a new key for the test
            $newKey = new Key(self::$namespace, self::$set, 0);

            // // Write the bin to the record
            $wp = new WritePolicy();
            self::$client->put($wp, $newKey, [$bin]);

            // // Read the bin back from the record
            $rp = new ReadPolicy();
            $record = self::$client->get($rp, $newKey, ["binName"]);
            $binGet = $record->getBins();

            // // Assert that the value associated with "binName" is an integer
            $this->assertEquals($binGet["binName"], $value);
        }
    }

    public function testPutGetString()
    {
        $binString = new Bin("stringBin", "StringData");
        $newKey = new Key(self::$namespace, self::$set, 2);
        $wp = new WritePolicy();
        self::$client->put($wp, $newKey, [$binString]);

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["stringBin"]);
        $binGet = $record->getBins();

        $this->assertIsString($binGet["stringBin"]);
    }

    public function testPutGetInteger()
    {
        // Prepare a bin with an integer value
        $binInteger = new Bin("integerBin", 42);

        // Create a new key for the test
        $newKey = new Key(self::$namespace, self::$set, 3);

        // Write the bin to the record
        $wp = new WritePolicy();
        self::$client->put($wp, $newKey, [$binInteger]);

        // Read the bin back from the record
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["integerBin"]);
        $binGet = $record->getBins();

        // Assert that the value associated with "integerBin" is an integer
        $this->assertIsInt($binGet["integerBin"]);
    }

    public function testPutGetLists()
    {
        // Prepare a list (array) bin
        $listData = [1, 2, 3, 4, 5];
        $binList = new Bin("listBin", $listData);

        // Create a new key for the test
        $newKey = new Key(self::$namespace, self::$set, 4);

        // Write the list bin to the record
        $wp = new WritePolicy();
        self::$client->put($wp, $newKey, [$binList]);

        // Read the list bin back from the record
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["listBin"]);
        $binGet = $record->getBins();

        // Assert that the value associated with "listBin" is an array
        $this->assertIsArray($binGet["listBin"]);

        $this->assertEquals($listData, $binGet["listBin"]);
    }


    public function testPutGetMaps()
    {
        // Prepare a map (associative array) bin
        $mapData = [
            "key1" => "value1",
            "key2" => "value2",
            "key3" => "value3",
        ];
        $binMap = new Bin("mapBin", $mapData);

        // Create a new key for the test
        $newKey = new Key(self::$namespace, self::$set, 5);

        // Write the map bin to the record
        $wp = new WritePolicy();
        self::$client->put($wp, $newKey, [$binMap]);

        // Read the map bin back from the record
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["mapBin"]);
        $binGet = $record->getBins();

        // Assert that the value associated with "mapBin" is an array (map)
        $this->assertIsArray($binGet["mapBin"]);

        // Optionally, you can assert that the content of the map matches
        $this->assertEquals($mapData, $binGet["mapBin"]);
    }

    public function testAddIntegerBinsToExistingRecord()
    {
        // Prepare a record with an existing integer bin
        $existingKey = new Key(self::$namespace, self::$set, 6);
        $existingBin = new Bin("newIntegerBin", 5);

        // Write the existing bin to the record
        $wp = new WritePolicy();
        self::$client->put($wp, $existingKey, [$existingBin]);

        // Prepare the bins to add (integer values)
        $binToAdd = new Bin("newIntegerBin", 10);
        self::$client->add($wp, $existingKey, [$binToAdd]);

        // Read the updated record
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $existingKey, ["newIntegerBin"]);
        $bins = $record->getBins();

        // Assert that the values have been added correctly
        $this->assertEquals(5 + 10, $bins["newIntegerBin"]);
    }

    public function testAddFloatToExistingRecord()
    {
        // Prepare a record with an existing integer bin
        $existingKey = new Key(self::$namespace, self::$set, 6);
        $existingBin = new Bin("newFloatBin", 5.5);

        // Write the existing bin to the record
        $wp = new WritePolicy();
        self::$client->put($wp, $existingKey, [$existingBin]);

        // Prepare the bins to add (integer values)
        $binToAdd = new Bin("newFloatBin", 10.1);
        self::$client->add($wp, $existingKey, [$binToAdd]);

        // Read the updated record
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $existingKey, ["newFloatBin"]);
        $bins = $record->getBins();

        // Assert that the values have been added correctly
        $this->assertEquals(5.5 + 10.1, $bins["newFloatBin"]);
    }

    public function testPrependValue()
    {
        $newKey = new Key(self::$namespace, self::$set, 2);
        $wp = new WritePolicy();
        $prependVal = new Bin("stringBin", "newData_");
        self::$client->prepend($wp, $newKey, [$prependVal]);

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["stringBin"]);
        $binGet = $record->getBins();

        $this->assertEquals("newData_StringData", $binGet["stringBin"]);
    }

    public function testAppendValue()
    {
        $newKey = new Key(self::$namespace, self::$set, 2);
        $wp = new WritePolicy();
        $prependVal = new Bin("stringBin", "_oldData");
        self::$client->append($wp, $newKey, [$prependVal]);

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["stringBin"]);
        $binGet = $record->getBins();

        $this->assertEquals("newData_StringData_oldData", $binGet["stringBin"]);
    }

    public function testDeleteKeyAndExists()
    {
        $newKey = new Key(self::$namespace, self::$set, "key_e");
        $wp = new WritePolicy();
        self::$client->put($wp, $newKey, [new Bin("bini", 1)]);
        $rp = new ReadPolicy();
        $exists = self::$client->exists($rp, $newKey);
        $this->assertTrue($exists);

        self::$client->delete($wp, $newKey);
        $exists = self::$client->exists($rp, $newKey);
        $this->assertFalse($exists);
    }

    public function testTouchKey()
    {
        $newKey = new Key(self::$namespace, self::$set, "new_key");
        $wp = new WritePolicy();
        self::$client->put($wp, $newKey, [new Bin("bini", 1)]);
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey);
        $this->assertEquals($record->getGeneration(), 1);

        $wp = new WritePolicy();
        self::$client->touch($wp, $newKey);
        $record = self::$client->get($rp, $newKey);
        $this->assertEquals($record->getGeneration(), 2);
        self::$client->delete($wp, $newKey);
    }

    public function testTruncate()
    {

        $wp = new WritePolicy();
        for ($i = 1; $i <= 10; $i++) {
            $bin1 = new Bin("bin1", $i);
            self::$client->put($wp, self::$key, [$bin1]);
        }

        // Perform the truncate operation
        $ip = new InfoPolicy();
        self::$client->truncate($ip, self::$namespace, self::$set);
        //wait for truncate to finish
        sleep(5);

        $rp = new ReadPolicy();
        for ($i = 1; $i <= 10; $i++) {
            $exists = self::$client->exists($rp, self::$key);
            $this->assertEquals($exists, false);
        }
    }

    public function testAppendException()
    {
        $stringKey = new Key(self::$namespace, self::$set, "string_key");
        $wp = new WritePolicy();
        self::$client->put($wp, $stringKey, [new Bin("sbin", "string_value")]);
        try {
            $appendExcpVal = new Bin("sbin", 23);
            self::$client->append($wp, $stringKey, [$appendExcpVal]);
            $this->fail("Expected exception AerospikeException not thrown");
        } catch (AerospikeException $e) {
            $this->assertSame($e->code, ResultCode::BIN_TYPE_ERROR);
        }
    }

    public function testReadTouchTTlPercent()
    {
        $stringKey = new Key(self::$namespace, self::$set, "new_key");
        $wp = new WritePolicy();
        $wp->setExpiration(Expiration::Seconds(10));
        self::$client->put($wp, $stringKey, [new Bin("record", "expires_in_10")]);
        $rp = new ReadPolicy();
        $rp->setReadTouchTtlPercent(80);
        $record = self::$client->get($rp, $stringKey);
        $this->assertEquals($record->getTtl(), 1);
    }
}
