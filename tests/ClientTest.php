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


    public static function setUpBeforeClass(): void
    {
        self::$cp = new ClientPolicy();

        try {
            self::$client = Aerospike(self::$cp, "127.0.0.1:3000");
            self::$key = new Key("test", "test", 1);
        } catch (Exception $e) {
            throw $e;
        }
    }

    public function testAerospikeConnection()
    {
        $this->assertTrue(self::$client->isConnected());
    }

    public function testBinNameTooLong(){
        $binString = new Bin("thisIsTooLongForTheBinName", "StringData");
        
        $this->expectExceptionMessage("bin name too long");
    }

    public function testPutGetString(){
        $binString = new Bin("stringBin", "StringData");
        $newKey = new Key("test", "test", 2);
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
        $newKey = new Key("test", "test", 3);

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
        $newKey = new Key("test", "test", 4);
    
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
        $newKey = new Key("test", "test", 5);
    
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
        $existingKey = new Key("test", "test", 6);
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
        $existingKey = new Key("test", "test", 6);
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

    public function testPrependValue(){
        $newKey = new Key("test", "test", 2);
        $wp = new WritePolicy();
        $prependVal = new Bin("stringBin", "newData_");
        self::$client->prepend($wp, $newKey, [$prependVal]);

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["stringBin"]);
        $binGet = $record->getBins();

        $this->assertEquals("newData_StringData", $binGet["stringBin"]);
    }

    public function testAppendValue(){
        $newKey = new Key("test", "test", 2);
        $wp = new WritePolicy();
        $prependVal = new Bin("stringBin", "_oldData");
        self::$client->append($wp, $newKey, [$prependVal]);

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey, ["stringBin"]);
        $binGet = $record->getBins();

        $this->assertEquals("newData_StringData_oldData", $binGet["stringBin"]);
    }


    public function testDeleteKeyAndExists(){
        $newKey = new Key("test", "test", 2);
        $wp = new WritePolicy();
        $exists = self::$client->exists($wp, $newKey);
        $this->assertTrue($exists);

        self::$client->delete($wp, $newKey);
        $exists = self::$client->exists($wp, $newKey);
        $this->assertFalse($exists);
    }

    public function testTouchKey(){
        $newKey = new Key("test", "test", 3);
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, $newKey);
        $this->assertEquals($record->getGeneration(), 1);

        $wp = new WritePolicy();
        self::$client->touch($wp, $newKey);
        $record = self::$client->get($rp, $newKey);
        $this->assertEquals($record->getGeneration(), 2);
    }

    public function testTruncate()
    {
        try {
            $wp = new WritePolicy();
            for ($i = 1; $i <= 10; $i++) {
                $bin1 = new Bin("bin1", $i);
                self::$client->put($wp, self::$key, [$bin1]);
            }

            // Perform the truncate operation
            self::$client->truncate(self::$namespace, self::$set);
            //wait for truncate to finish
            usleep(200000);

            $sp = new ScanPolicy();
            $count = 0;
            $recordset = self::$client->scan($sp, "test", "test", ["bin1"]);
            while ($rec = $recordset->next()) {
                $count += 1;
            }
            //wait for scan to finish
            usleep(200000);
            $this->assertEquals($count, 0);
        } catch (Exception $e) {
            $this->fail("An exception was thrown during truncate: " . $e->getMessage());
        }
    }


}