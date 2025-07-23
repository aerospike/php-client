<?php

namespace Aerospike;

use PHPUnit\Framework\TestCase;

class CDTSingleTest extends TestCase
{

    protected static $client;
    protected static $namespace = "test";
    protected static $socket = "/tmp/asld_grpc.sock";
    protected static $set;
    protected static $key;
    protected static $cdtBinName;

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$socket);
        } catch (AerospikeException $e) {
            throw $e;
        }
    }

    // protected function randomString($length)
    // {
    //     $randomBytes = random_bytes($length);

    //     $randomString = base64_encode($randomBytes);

    //     $randomString = preg_replace('/[^a-zA-Z0-9]/', '', $randomString);
    //     $randomString = substr($randomString, 0, $length);

    //     return $randomString;
    // }

    protected function setUp(): void
    {
        self::$set = "set_name"; //self::randomString(random_int(5, 10));
        // $ip = new InfoPolicy();
        // self::$client->truncate($ip, self::$namespace, self::$set);

        self::$key = new Key(self::$namespace, self::$set, 255);
        self::$cdtBinName = "randomBinName"; //self::randomString(random_int(5, 10));
    }

    protected function assertBitModifyOperations($initial, $expected, ...$ops)
    {
        $wp = new WritePolicy();
        self::$client->delete($wp, self::$key);

        if ($initial !== null) {
            $bins = [new Bin(self::$cdtBinName, Value::blob($initial))];
            self::$client->put($wp, self::$key, $bins);
        }

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $bwp = new BatchWritePolicy();
        $bp = new BatchPolicy();

        foreach ($ops as $op) {
            $full_ops[] = $op;
        }
        $batchWrite = new BatchWrite($bwp, self::$key, $full_ops);
        self::$client->batch($bp, [$batchWrite]);
        $record = self::$client->get($rp, self::$key);

        $this->assertEquals($record->bins[self::$cdtBinName], Value::blob($expected));
    }

    public function testShouldSetBin()
    {
        $bit0 = [0x80];
        $defaultBitPolicy = new BitwisePolicy(BitwiseWriteFlags::Default());
        $updateBitPolicy = new BitwisePolicy(BitwiseWriteFlags::UpdateOnly());

        self::assertBitModifyOperations(
            [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
            [0x51, 0x02, 0x03, 0x04, 0x05, 0x06],
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 1, 1, $bit0),
            BitwiseOp::set($updateBitPolicy, self::$cdtBinName, 3, 1, $bit0),
            BitwiseOp::remove($updateBitPolicy, self::$cdtBinName, 6, 2)
        );
    }
}
