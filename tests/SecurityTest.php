<?php

namespace Aerospike;

use PHPUnit\Framework\TestCase;

/* This test needs to have security enabled in aerospike.conf.
For more info please visit  - "https://docs.aerospike.com/server/operations/configure/security/access-control"
*/


final class SecurityTest extends TestCase
{

    protected static $socket = "/tmp/asld_grpc.sock";
    protected static $client;

    protected static $cp;
    protected static $key;

    protected static $namespace = "test";
    protected static $set = "test";

    public static function setUpBeforeClass(): void
    {
        try {
            self::$client = Client::connect(self::$socket);
            self::$key = new Key(self::$namespace, self::$set, 1);
        } catch (Exception $e) {
            throw $e;
        }
    }


    //Change this to TRUE when you enable security and want to test connections.
    protected static $authRequired = false;

    protected function isSecurityEnabled()
    {
        return self::$authRequired;
    }

    public function testAerospikeConnectionWithAuthEnabled()
    {
        if (!$this->isSecurityEnabled()) {
            $this->markTestSkipped("Enable Security in Aerospike.conf");
        }
        self::$client = Client::connect(self::$socket);

        // Assert that the Aerospike connection is successfully created
        $this->assertNotNull(self::$client->socket);
    }

    public function testCreateUser()
    {
        if (!$this->isSecurityEnabled()) {
            $this->markTestSkipped("Enable Security in Aerospike.conf");
        }
        $this->expectNotToPerformAssertions();
        $ap = new AdminPolicy();
        self::$client->createUser($ap, "user1", "password", ["read-write"]);
    }


    public function testDropUser()
    {
        if (!$this->isSecurityEnabled()) {
            $this->markTestSkipped("Enable Security in Aerospike.conf");
        }
        $this->expectNotToPerformAssertions();
        $ap = new AdminPolicy();
        self::$client->dropUser($ap, "user1");
    }

    public function testChangePassword()
    {
        if (!$this->isSecurityEnabled()) {
            $this->markTestSkipped("Enable Security in Aerospike.conf");
        }
        $this->expectNotToPerformAssertions();
        $ap = new AdminPolicy();
        self::$client->createUser($ap, "user1", "password", ["read-write"]);
        self::$client->changePassword($ap, "user1", "newPassword");
        self::$client->dropUser($ap, "user1");
    }

    public function testChangePasswordOfUnknownUser()
    {
        if (!$this->isSecurityEnabled()) {
            $this->markTestSkipped("Enable Security in Aerospike.conf");
        }

        $ap = new AdminPolicy();
        self::$client->createUser($ap, "user2", "password", ["read-write"]);
        try {
            $result = self::$client->changePassword($ap, "user1", "newPassword");
        } catch (AerospikeException $e) {
            $this->assertSame($e->message, "user name is invalid");
        }
        self::$client->dropUser($ap, "user2");
    }

    public function testQueryUsers()
    {
        if (!$this->isSecurityEnabled()) {
            $this->markTestSkipped("Enable Security in Aerospike.conf");
        }
        $this->expectNotToPerformAssertions();
        $ap = new AdminPolicy();
        self::$client->createUser($ap, "user1", "password", ["read-write"]);
        self::$client->changePassword($ap, "user1", "newPassword");
        self::$client->dropUser($ap, "user1");
    }
}
