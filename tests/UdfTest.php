<?php

namespace Aerospike;

use PHPUnit\Framework\TestCase;

class UdfTest extends TestCase{
    protected static $client;
    protected static $namespace = "test";
    protected static $socket = "/tmp/asld_grpc.sock";
    protected static $set;
    protected static $key;
    protected static $udfBody = 'function testFunc1(rec, div)
    local ret = map();                     -- Initialize the return value (a map)
  
    local x = rec["bin1"];                 -- Get the value from record bin named "bin1"
  
    rec["bin2"] = math.floor(x / div);     -- Set the value in record bin named "bin2"
  
    aerospike:update(rec);                 -- Update the main record
  
    ret["status"] = "OK";                   -- Populate the return status
    return ret;                             -- Return the Return value and/or status
end';

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$socket);
        } catch (AerospikeException $e) {
            throw $e;
        }
    }

    protected function randomString($length) {
        $randomBytes = random_bytes($length);
    
        $randomString = base64_encode($randomBytes);

        $randomString = preg_replace('/[^a-zA-Z0-9]/', '', $randomString);
        $randomString = substr($randomString, 0, $length);
        
        return $randomString;
    }

    public function testRunUdfOnASingleRecord(){
        $wp = new WritePolicy();
        self::$client->registerUdf($wp, self::$udfBody, "udf1.lua", UdfLanguage::lua());
        self::$key = new Key(self::$namespace, self::randomString(random_int(5, 10)), self::randomString(random_int(5, 10)));

        $bin1 = new Bin("bin1", 20);
        $bin2 = new Bin("bin2", 1);
        self::$client->put($wp, self::$key, [$bin1, $bin2]);

        $res = self::$client->udfExecute($wp, self::$key, "udf1", "testFunc1", [2]);
        $this->assertEquals($res["status"], "OK");
        usleep(300000);

        $rp = new ReadPolicy();
        $rec = self::$client->get($rp, self::$key);
        $this->assertEquals($rec->bins["bin2"], 10);
        $this->assertEquals($rec->bins["bin1"], 20);
    }

    public function testListAllUdf(){
        $rp = new ReadPolicy();
        $listAllUdf = self::$client->listUdf($rp);
        $this->assertGreaterThan(0, count($listAllUdf));
    }

    public function testDropUdf(){
        $this->expectNotToPerformAssertions();
        $wp = new WritePolicy();
        self::$client->registerUdf($wp, self::$udfBody, "udfToBeDropped.lua", UdfLanguage::lua());
        self::$client->dropUdf($wp, "udfToBeDropped.lua");
    }
}