<?php

namespace Aerospike;

use PHPUnit\Framework\TestCase;

class CDTBitwiseOpTest extends TestCase{

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

    protected function randomString($length) {
        $randomBytes = random_bytes($length);
    
        $randomString = base64_encode($randomBytes);

        $randomString = preg_replace('/[^a-zA-Z0-9]/', '', $randomString);
        $randomString = substr($randomString, 0, $length);
        
        return $randomString; 
    }

    protected function setUp(): void
    {
        self::$set = self::randomString(random_int(5, 10));
        $ip = new InfoPolicy();
        self::$client->truncate($ip, self::$namespace, self::$set);

        self::$key = new Key(self::$namespace, self::$set, self::randomString(random_int(5, 10)));
        self::$cdtBinName = self::randomString(random_int(5, 10));
    }

    protected function testBitModifyRegion($bin_sz, $offset, $set_sz, $expected, $isInsert, ...$ops){
        $dp = new WritePolicy();
        self::$client->delete($dp, $key);
        $initial = array_fill(0, $bin_sz, 0xFF);
        
        $wp = new WritePolicy();
        self::$client->put($wp, self::$key, [new Bin(self::$cdtBinName, $initial)]);

        $int_sz = 64;

        if($set_sz < $int_sz) {
            $int_sz = $set_sz;
        }

        $bin_bit_sz = $bin_sz * 8;

        if($isInsert){
            $bin_bit_sz += $set_sz;
        }

        foreach ($ops as $op) {
            $full_ops[] = $op;
        }
    
        $full_ops[] = BitwiseOp::lscan(self::$cdtBinName, $offset, $set_sz, true);
        $full_ops[] = BitwiseOp::rscan(self::$cdtBinName, $offset, $set_sz, true);
        $full_ops[] = BitwiseOp::getInt(self::$cdtBinName, $offset, $int_sz, false);
        $full_ops[] = BitwiseOp::count(self::$cdtBinName, $offset, $set_sz);
        $full_ops[] = BitwiseOp::lscan(self::$cdtBinName, 0, $bin_bit_sz, false);
        $full_ops[] = BitwiseOp::rscan(self::$cdtBinName, 0, $bin_bit_sz, false);
        $full_ops[] = BitwiseOp::get(self::$cdtBinName, $offset, $set_sz);

        $bwp = new BatchWritePolicy();
        $bp = new BatchPolicy();
        $batchWrite = new BatchWrite($bwp, self::$key, $full_ops);
    }

    protected function assertBitModifyOperations($initial, $expected, ...$ops){
        $wp = new WritePolicy();
        self::$client->delete($wp, self::$key);

        if ($initial !== null) {
            $bins = [new Bin(self::$cdtBinName, $initial)];
            self::$client->put($wp, self::$key, $bins);
        }

        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        echo "Record Before: ";
        var_dump($record->bins[self::$cdtBinName]);
        echo "\n";

        $bwp = new BatchWritePolicy();
        $bp = new BatchPolicy();

        foreach ($ops as $op) {
            $full_ops[] = $op;
        }
        $batchWrite = new BatchWrite($bwp, self::$key, $full_ops);
        self::$client->batch($bp, [$batchWrite]);
        $record = self::$client->get($rp, self::$key);
        echo "Record After: ";
        var_dump($record->bins[self::$cdtBinName]);
        echo "\n";

        $this->assertEquals($record->bins[self::$cdtBinName], $expected);
    }

    public function testShouldSetBin(){
        $bit0 = [0x80];
        $defaultBitPolicy = new BitwisePolicy(BitwiseWriteFlags::default());
        $updateBitPolicy = new BitwisePolicy(BitwiseWriteFlags::updateOnly());

        self::assertBitModifyOperations(
            [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
            [0x51, 0x02, 0x03, 0x04, 0x05, 0x06],
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 1, 1, $bit0),
            BitwiseOp::set($updateBitPolicy, self::$cdtBinName, 3, 1, $bit0),
            BitwiseOp::remove($updateBitPolicy, self::$cdtBinName, 6, 2)
        );
    }

    public function testShouldSetBinsBits(){
        $bit0 = [0x80];
        $bits1 = [0x11, 0x22, 0x33];
        $defaultBitPolicy = new BitwisePolicy(BitwiseWriteFlags::default());

        self::assertBitModifyOperations(
            [0x01, 0x12, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x41],
            [0x41, 0x13, 0x11, 0x22, 0x33, 0x11, 0x22, 0x33, 0x08, 0x08, 0x91, 0x1B, 0x01, 0x12, 0x23,
            0x11, 0x22, 0x11, 0xc1],
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 1, 1, $bit0),
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 15, 1, $bit0),
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 16, 24, $bits1),
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 40, 22, $bits1),
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 73, 21, $bits1),
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 100, 20, $bits1),
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 120, 17, $bits1),
            BitwiseOp::set($defaultBitPolicy, self::$cdtBinName, 144, 1, $bit0)
        );
    }

    public function testShouldLShiftBits(){
        $bit0 = [0x80];
        $bits1 = [0x11, 0x22, 0x33];
        $defaultBitPolicy = new BitwisePolicy(BitwiseWriteFlags::default());

        self::assertBitModifyOperations(
            [0x01, 0x01, 0x00, 0x80,
            0xFF, 0x01, 0x01,
            0x18, 0x01],
            [0x02, 0x40, 0x01, 0x00,
            0xF8, 0x08, 0x01,
            0x28, 0x01],
            BitwiseOp::lshift($defaultBitPolicy, self::$cdtBinName, 0, 8, 1),
            BitwiseOp::lshift($defaultBitPolicy, self::$cdtBinName, 9, 7, 6),
            BitwiseOp::lshift($defaultBitPolicy, self::$cdtBinName, 23, 2, 1),
            BitwiseOp::lshift($defaultBitPolicy, self::$cdtBinName, 37, 18, 3),
            BitwiseOp::lshift($defaultBitPolicy, self::$cdtBinName, 58, 2, 1),
            BitwiseOp::lshift($defaultBitPolicy, self::$cdtBinName, 64, 4, 7)
        );
    }

    public function testShouldRShiftBits(){
        $putMode = new BitwisePolicy(BitwiseWriteFlags::default());
    
        self::assertBitModifyOperations(
            [0x80, 0x40, 0x01, 0x00,
            0xFF, 0x01, 0x01,
            0x18, 0x80],
            [0x40, 0x01, 0x00, 0x80,
            0xF8, 0xE0, 0x21,
            0x14, 0x80],
            BitwiseOp::rshift($putMode, self::$cdtBinName, 0, 8, 1),
            BitwiseOp::rshift($putMode, self::$cdtBinName, 9, 7, 6),
            BitwiseOp::rshift($putMode, self::$cdtBinName, 23, 2, 1),
            BitwiseOp::rshift($putMode, self::$cdtBinName, 37, 18, 3),
            BitwiseOp::rshift($putMode, self::$cdtBinName, 60, 2, 1),
            BitwiseOp::rshift($putMode, self::$cdtBinName, 68, 4, 7)
        );
    }

    public function testShouldORBits(){
        $bits1 = [0x11, 0x22, 0x33];
        $putMode = new BitwisePolicy(BitwiseWriteFlags::default());
    
        self::assertBitModifyOperations(
            [0x80, 0x40, 0x01, 0x00, 0x00,
            0x01, 0x02, 0x03],
            [0x90, 0x48, 0x01, 0x20, 0x11,
            0x11, 0x22, 0x33],
            BitwiseOp::or($putMode, self::$cdtBinName, 0, 5, $bits1),
            BitwiseOp::or($putMode, self::$cdtBinName, 9, 7, $bits1),
            BitwiseOp::or($putMode, self::$cdtBinName, 23, 6, $bits1),
            BitwiseOp::or($putMode, self::$cdtBinName, 32, 8, $bits1),
            BitwiseOp::or($putMode, self::$cdtBinName, 40, 24, $bits1)
        );
    }

    public function testShouldXORBits(){
        $bits1 = [0x11, 0x22, 0x33];
        $putMode = new BitwisePolicy(BitwiseWriteFlags::default());
    
        self::assertBitModifyOperations(
            [0x80, 0x40, 0x01, 0x00, 0x00,
            0x01, 0x02, 0x03],
            [0x90, 0x48, 0x01, 0x20, 0x11, 0x10, 0x20,
            0x30],
            BitwiseOp::xor($putMode, self::$cdtBinName, 0, 5, $bits1),
            BitwiseOp::xor($putMode, self::$cdtBinName, 9, 7, $bits1),
            BitwiseOp::xor($putMode, self::$cdtBinName, 23, 6, $bits1),
            BitwiseOp::xor($putMode, self::$cdtBinName, 32, 8, $bits1),
            BitwiseOp::xor($putMode, self::$cdtBinName, 40, 24, $bits1)
        );
    }

    public function testShouldANDBits(){
        $bits1 = [0x11, 0x22, 0x33];
        $putMode = new BitwisePolicy(BitwiseWriteFlags::default());
    
        self::assertBitModifyOperations(
            [0x80, 0x40, 0x01, 0x00, 0x00,
            0x01, 0x02, 0x03],
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03],
            BitwiseOp::and($putMode, self::$cdtBinName, 0, 5, $bits1),
            BitwiseOp::and($putMode, self::$cdtBinName, 9, 7, $bits1),
            BitwiseOp::and($putMode, self::$cdtBinName, 23, 6, $bits1),
            BitwiseOp::and($putMode, self::$cdtBinName, 32, 8, $bits1),
            BitwiseOp::and($putMode, self::$cdtBinName, 40, 24, $bits1)
        );
    }

    public function testShouldNOTBits(){
        $putMode = new BitwisePolicy(BitwiseWriteFlags::default());
    
        self::assertBitModifyOperations(
            [0x80, 0x40, 0x01, 0x00, 0x00, 0x01, 0x02, 0x03],
            [0x78, 0x3F, 0x00, 0xF8, 0xFF, 0xFE, 0xFD, 0xFC],
            BitwiseOp::not($putMode, self::$cdtBinName, 0, 5),
            BitwiseOp::not($putMode, self::$cdtBinName, 9, 7),
            BitwiseOp::not($putMode, self::$cdtBinName, 23, 6),
            BitwiseOp::not($putMode, self::$cdtBinName, 32, 8),
            BitwiseOp::not($putMode, self::$cdtBinName, 40, 24)
        );
    }

}