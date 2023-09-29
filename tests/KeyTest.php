<?php 

use PHPUnit\Framework\TestCase;

final class KeyTest extends TestCase
{
    public function testIntegerKey()
    {
        $key = new Key("namespace", "set", 1);
        $digest = $key->getDigest();
        $this->assertEquals("82d7213b469812947c109a6d341e3b5b1dedec1f", $digest);
    }

    public function testIntegerMaxKey()
    {
        $key = new Key("namespace", "set", 123444);
        $digest = $key->getDigest();
        $this->assertEquals("6fa3f682162b45d3a96cd8a5adc86e20c68c3a9c", $digest);
    }

    public function testStringKey()
    {
        $key = new Key("namespace", "set", "");
        $digest = $key->getDigest();
        $this->assertEquals("2819b1ff6e346a43b4f5f6b77a88bc3eaac22a83", $digest);
    }

    public function testStringWithValKey()
    {
        $key = new Key("namespace", "set", "someValue");
        $digest = $key->getDigest();
        $this->assertEquals("fd70ae4e48c2cfa71b37cfcf05d0ae75f46796c7", $digest);
    }
}