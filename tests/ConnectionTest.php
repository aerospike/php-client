<?php 

namespace Aerospike;
use PHPUnit\Framework\TestCase;

/* This test needs to have security enbaled in aerospike.conf.
For more info please visit  - "https://docs.aerospike.com/server/operations/configure/security/access-control"
*/


final class ConnectionTest extends TestCase
{

    protected static $host = "127.0.0.1:3000";
    
    //Change this to TRUE when you enable security and want to test connections.
    protected static $authRequired = FALSE;

    protected function isSecurityEnabled()
    {
        return self::$authRequired;
    }   

    public function testAerospikeConnectionSuccess()
    {  
        if(!$this->isSecurityEnabled()){
            $this->markTestSkipped("Enable Secuirty in Aerospike.conf");
        }
        $cp = new ClientPolicy();
        $cp->setUser("admin");
        $cp->setPassword("admin");
        $client = Aerospike($cp, self::$host);
        $connected = $client->isConnected();

        // Assert that the Aerospike connection is successfully created
        $this->assertEquals($connected, TRUE);
        $client->close();
    }

    public function testAerospikeConnectionFailure()
    {    
        if(!$this->isSecurityEnabled()){
            $this->markTestSkipped("Enable Secuirty in Aerospike.conf");
        }
        $cp = new ClientPolicy();
        $cp->setUser("admin");
        $cp->setPassword("wrongPassword");

        $this->expectException(\Exception::class);

        $client = Aerospike($cp, self::$host);
    }
}