<?php 

namespace Aerospike;
use PHPUnit\Framework\TestCase;

/* This test needs to have security enbaled in aerospike.conf.
For more info please visit  - "https://docs.aerospike.com/server/operations/configure/security/access-control"
*/


final class SecurityTest extends TestCase
{

    protected static $socket = "/tmp/asld_grpc.sock";
    protected static $client;

    //Change this to TRUE when you enable security and want to test connections.
    protected static $authRequired = FALSE;

    protected function isSecurityEnabled()
    {
        return self::$authRequired;
    }   

    public function testAerospikeConnectionWithAuthEnabled()
    {  
        if(!$this->isSecurityEnabled()){
            $this->markTestSkipped("Enable Secuirty in Aerospike.conf");
        }
        self::$client = Client::connect(self::$socket);

        // Assert that the Aerospike connection is successfully created
        $this->assertNotNull(self::$client->socket);
    }

    public function testCreateUser()
    {  
        if(!$this->isSecurityEnabled()){
            $this->markTestSkipped("Enable Secuirty in Aerospike.conf");
        }
        $this->expectNotToPerformAssertions();
        $ap = new AdminPolicy();
        $client->createUser($ap, "user1", "password", ["read-write"]);
    }


    public function testDropUser()
    {  
        if(!$this->isSecurityEnabled()){
            $this->markTestSkipped("Enable Secuirty in Aerospike.conf");
        }
        $this->expectNotToPerformAssertions();
        $ap = new AdminPolicy();
        $client->dropUser($ap, "user1");


    }

    public function testChangePassword()
    {  
        if(!$this->isSecurityEnabled()){
            $this->markTestSkipped("Enable Secuirty in Aerospike.conf");
        }
        $this->expectNotToPerformAssertions();
        $ap = new AdminPolicy();
        $client->createUser($ap, "user1", "password", ["read-write"]);

        $client->changePassword($ap, "user1", "newPassword");
    }

    public function testChangePasswordOfUnknownUser()
    {  
        if(!$this->isSecurityEnabled()){
            $this->markTestSkipped("Enable Secuirty in Aerospike.conf");
        }
        
        $ap = new AdminPolicy();
        $client->createUser($ap, "userDoesNotExist", "password", ["read-write"]);

        $client->changePassword($ap, "user1", "newPassword");
    }

    public function testQueryUsers()
    {  
        if(!$this->isSecurityEnabled()){
            $this->markTestSkipped("Enable Secuirty in Aerospike.conf");
        }
        $this->expectNotToPerformAssertions();
        $ap = new AdminPolicy();
        $client->createUser($ap, "user1", "password", ["read-write"]);

        $client->changePassword($ap, "user1", "newPassword");
    }
    
}