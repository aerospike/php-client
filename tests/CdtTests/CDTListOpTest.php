<?php

namespace Aerospike;

use PHPUnit\Framework\TestCase;

class CDTListOpTest extends TestCase{

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

    protected function setUp(): void
    {
        self::$set = self::randomString(random_int(5, 10));
        self::$key = new Key(self::$namespace, self::$set, self::randomString(random_int(5, 10)));
        self::$cdtBinName = self::randomString(random_int(5, 10));

        $list = array();
        $bwp = new BatchWritePolicy(); 
        $lp = new ListPolicy(ListOrderType::Unordered());
        $bp = new BatchPolicy();
        for ($i = 1; $i <= 10; $i++) {
            $list[] = $i;
            $ops = [ListOp::append($lp, self::$cdtBinName, [$i])];
            $bw = new BatchWrite($bwp, self::$key, $ops);
            self::$client->batch($bp, [$bw]);
        }
    }
    
    protected function randomString($length) {
        $randomBytes = random_bytes($length);
    
        $randomString = base64_encode($randomBytes);

        $randomString = preg_replace('/[^a-zA-Z0-9]/', '', $randomString);
        $randomString = substr($randomString, 0, $length);
        
        return $randomString;
    }

    protected function getSizeOfList($record, $binName) {
        $array = $record->bins[$binName];
        return count($array);
    }

    public function testShouldCreateValidCDTList(){
        $list = array();
        $bwp = new BatchWritePolicy(); 
        $lp = new ListPolicy(ListOrderType::Unordered());
        $bp = new BatchPolicy();
        $binName = "listBin";
        for ($i = 1; $i <= 100; $i++) {
            $list[] = $i;
            $ops = [ListOp::append($lp, $binName, [$i])];
            $bw = new BatchWrite($bwp, self::$key, $ops);
            self::$client->batch($bp, [$bw]);
            $rp = new ReadPolicy();
            $record = self::$client->get($rp, self::$key);
            $this->assertEquals(self::getSizeOfList($record, $binName), $i);
        }
    }

    public function testShouldGetListSize(){
        $lp = new ListPolicy(ListOrderType::Unordered());
        $bp = new BatchPolicy();

        // Get the size of the list
        $listOp = ListOp::size(self::$cdtBinName);
        $opsGetSize = [$listOp]; 
        $brp = new BatchReadPolicy();
        $br = BatchRead::ops($brp, self::$key, $opsGetSize);
        $recs = self::$client->batch($bp, [$br]);
        $this->assertEquals($recs[0]->record->bins[self::$cdtBinName], 10);
    }

    public function testShouldAppendElementToTail(){
        $bwp = new BatchWritePolicy(); 
        $lp = new ListPolicy(ListOrderType::Unordered());
        $bp = new BatchPolicy();
        
        $ops = [ListOp::append($lp, self::$cdtBinName, [11])];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $list = $record->bins[self::$cdtBinName];
        $this->assertEquals($list[10], 11);
    }

    public function testShouldAppendFewElementsToTail(){
        $bwp = new BatchWritePolicy(); 
        $lp = new ListPolicy(ListOrderType::Unordered());
        $bp = new BatchPolicy();
        
        $ops = [ListOp::append($lp, self::$cdtBinName, [11, 12, 13])];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $list = $record->bins[self::$cdtBinName];
        $this->assertEquals($list[10], 11);
        $this->assertEquals($list[11], 12);
        $this->assertEquals($list[12], 13);
    }

    public function testShouldPrependElement(){
        $bwp = new BatchWritePolicy(); 
        $lp = new ListPolicy(ListOrderType::Unordered());
        $bp = new BatchPolicy();
        
        $ops = [ListOp::insert($lp, self::$cdtBinName, 0, [-1])];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $list = $record->bins[self::$cdtBinName];
        $this->assertEquals($list[0], -1);
    }

    public function testShouldPopElementFromHead(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        
        $ops = [ListOp::pop(self::$cdtBinName, 0)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $list = $record->bins[self::$cdtBinName];
        $this->assertEquals($list[0], 2);
    }
    
    public function testShouldPopElementRange(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        
        $ops = [ListOp::popRange(self::$cdtBinName, 0, 3)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals(self::getSizeOfList($record, self::$cdtBinName), 7);
    }
        
    public function testShouldPopElementFromIndexToEnd(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        
        $ops = [ListOp::popRangeFrom(self::$cdtBinName, 5)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals(self::getSizeOfList($record, self::$cdtBinName), 5);
    }

    public function testShouldRemoveElement(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        
        $ops = [ListOp::removeValues(self::$cdtBinName, [8, 9, 10])];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals(self::getSizeOfList($record, self::$cdtBinName), 7);
    }

    public function testShouldRemoveByIndexRange(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        
        $ops = [ListOp::removeRange(self::$cdtBinName, 0, 3)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals(self::getSizeOfList($record, self::$cdtBinName), 7);
    }

    public function testShouldRemoveByValueByRelativeRankRange(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        
        $ops = [ListOp::removeByValueRelativeRankRange(self::$cdtBinName, 5, 2)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals(self::getSizeOfList($record, self::$cdtBinName), 6);
    }

    public function testShouldIncrementIndexByValue(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        $rp = new ReadPolicy();
        $record = self::$client->get($rp, self::$key);
        
        $this->assertEquals($record->bins[self::$cdtBinName][0], 1);
        $ops = [ListOp::increment(self::$cdtBinName, 0, 10)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals($record->bins[self::$cdtBinName][0], 11);
    }

    public function testShouldSortListByValue(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        $rp = new ReadPolicy();
        $lp = new ListPolicy(ListOrderType::Unordered());

        $record = self::$client->get($rp, self::$key);
        
        $this->assertEquals($record->bins[self::$cdtBinName][0], 1);
        $ops = [ListOp::sort(self::$cdtBinName, ListSortFlags::descending())];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals($record->bins[self::$cdtBinName][0], 10);
    }

    public function testShouldSetIndexToValue(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        $rp = new ReadPolicy();
        $lp = new ListPolicy(ListOrderType::Unordered());

        $record = self::$client->get($rp, self::$key);
        
        $this->assertEquals($record->bins[self::$cdtBinName][3], 4);
        $ops = [ListOp::set(self::$cdtBinName, 3, "newElement")];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals($record->bins[self::$cdtBinName][3], "newElement");
    }

    public function testShouldTrimList(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        $rp = new ReadPolicy();
        $lp = new ListPolicy(ListOrderType::Unordered());

        $record = self::$client->get($rp, self::$key);
        
        $this->assertEquals($record->bins[self::$cdtBinName][0], 1);
        $ops = [ListOp::trim(self::$cdtBinName, 0, 5)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals(self::getSizeOfList($record, self::$cdtBinName), 5);
    }

    public function testShouldClear(){
        $bwp = new BatchWritePolicy(); 
        $bp = new BatchPolicy();
        $rp = new ReadPolicy();
        $lp = new ListPolicy(ListOrderType::Unordered());

        $record = self::$client->get($rp, self::$key);
        
        $this->assertEquals($record->bins[self::$cdtBinName][0], 1);
        $ops = [ListOp::clear(self::$cdtBinName)];
        $bw = new BatchWrite($bwp, self::$key, $ops);
        self::$client->batch($bp, [$bw]);
        
        $record = self::$client->get($rp, self::$key);
        $this->assertEquals(self::getSizeOfList($record, self::$cdtBinName), 0);
    }

}   