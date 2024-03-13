<?php 

namespace Aerospike;
use PHPUnit\Framework\TestCase;

final class CDTListTest extends TestCase
{

    protected $client;
    protected $namespace;
    protected $set;
    protected $key;
    protected $wpolicy;
    protected $cdtBinName;
    protected $list;

    protected function setUp(): void
    {
        $this->client = new AerospikeClient(); // Assuming AerospikeClient is implemented similarly in PHP
        $this->namespace = 'test';
        $this->set = $this->randString(8);
        $this->wpolicy = new WritePolicy();
        $this->cdtBinName = $this->randString(10);
    }

    public function testCreateValidCDTList(): void
    {
        $key = new Key($this->namespace, $this->set, $this->randString(50));
        $cdtList = $this->client->operate($this->wpolicy, $key, new ListGetOp($this->cdtBinName, 0));
        $this->assertNull($cdtList);

        $list = [];
        for ($i = 1; $i <= 100; $i++) {
            $list[] = $i;

            $sz = $this->client->operate($this->wpolicy, $key, new ListAppendOp($this->cdtBinName, $i));
            $this->assertNotNull($sz);
            $this->assertEquals($i, $sz->getBins()[$this->cdtBinName]);

            $sz = $this->client->operate($this->wpolicy, $key, new ListSizeOp($this->cdtBinName));
            $this->assertNotNull($sz);
            $this->assertEquals($i, $sz->getBins()[$this->cdtBinName]);
        }

        $sz = $this->client->operate($this->wpolicy, $key, new ListGetRangeOp($this->cdtBinName, 0, 100));
        $this->assertNotNull($sz);
        $this->assertEquals($list, $sz->getBins()[$this->cdtBinName]);

        $sz = $this->client->operate($this->wpolicy, $key, new ListAppendOp($this->cdtBinName, ...$list));
        $this->assertNotNull($sz);
        $this->assertEquals(100 * 2, $sz->getBins()[$this->cdtBinName]);
    }

    protected function randString($length): string
    {
        $characters = '0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ';
        $charactersLength = strlen($characters);
        $randomString = '';
        for ($i = 0; $i < $length; $i++) {
            $randomString .= $characters[rand(0, $charactersLength - 1)];
        }
        return $randomString;
    }
}