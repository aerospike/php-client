<?php

namespace Aerospike;

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

    public function testMatrixKey()
    {
        $matrix = [
            -2147483648 => "d635a867b755f8f54cdc6275e6fb437df82a728c",
            2147483647 => "fa8c47b8b898af1bbcb20af0d729ca68359a2645",
            -32768 => "7f41e9dd1f3fe3694be0430e04c8bfc7d51ec2af",
            32767 => "309fc9c2619c4f65ff7f4cd82085c3ee7a31fc7c",
            -128 => "93191e549f8f3548d7e2cfc958ddc8c65bcbe4c6",
            127 => "a58f7d98bf60e10fe369c82030b1c9dee053def9",
            -1 => "22116d253745e29fc63fdf760b6e26f7e197e01d",
            0 => "93d943aae37b017ad7e011b0c1d2e2143c2fb37d",

            "" => "2819b1ff6e346a43b4f5f6b77a88bc3eaac22a83",
            str_repeat("s", 1) => "607cddba7cd111745ef0a3d783d57f0e83c8f311",
            str_repeat("a", 10) => "5979fb32a80da070ff356f7695455592272e36c2",
            str_repeat("m", 100) => "f00ad7dbcb4bd8122d9681bca49b8c2ffd4beeed",
            str_repeat("t", 1000) => "07ac412d4c33b8628ab147b8db244ce44ae527f8",
            str_repeat("-", 10000) => "b42e64afbfccb05912a609179228d9249ea1c1a0",
            str_repeat("+", 100000) => "0a3e888c20bb8958537ddd4ba835e4070bd51740",
        ];


        foreach ($matrix as $key_val => $digest) {
            $key = new Key("namespace", "set", $key_val);
            $this->assertEquals($digest, $key->getDigest());
        }

        $matrix = [
            Value::Blob(array_values(unpack('C*', ""))),
            Value::Blob(array_values(unpack('C*', str_repeat("s", 1)))),
            Value::Blob(array_values(unpack('C*', str_repeat("a", 10)))),
            Value::Blob(array_values(unpack('C*', str_repeat("m", 100)))),
            Value::Blob(array_values(unpack('C*', str_repeat("t", 1000)))),
            Value::Blob(array_values(unpack('C*', str_repeat("-", 10000)))),
            Value::Blob(array_values(unpack('C*', str_repeat("+", 100000)))),
        ];

        $hash = [
            "327e2877b8815c7aeede0d5a8620d4ef8df4a4b4",
            "ca2d96dc9a184d15a7fa2927565e844e9254e001",
            "d10982327b2b04c7360579f252e164a75f83cd99",
            "475786aa4ee664532a7d1ea69cb02e4695fcdeed",
            "5a32b507518a49bf47fdaa3deca53803f5b2e8c3",
            "ed65c63f7a1f8c6697eb3894b6409a95461fd982",
            "fe19770c371774ba1a1532438d4851b8a773a9e6",
        ];

        foreach ($matrix as $index => $key_val) {
            $key = new Key("namespace", "set", $key_val);
            $this->assertEquals($hash[$index], $key->getDigest());
        }
    }
}
