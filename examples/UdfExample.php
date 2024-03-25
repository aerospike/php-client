<?php

namespace Aerospike;

$namespace = "test";
$set = "test";
$socket = "/tmp/asld_grpc.sock";

$client = Client::connect($socket);
echo "* Connected to the local daemon: $client->socket \n";


$udfBody = 'function testFunc1(rec, div)
    local ret = map();                     -- Initialize the return value (a map)
  
    local x = rec["bin1"];                 -- Get the value from record bin named "bin1"
  
    rec["bin2"] = math.floor(x / div);     -- Set the value in record bin named "bin2"
  
    aerospike:update(rec);                 -- Update the main record
  
    ret["status"] = "OK";                   -- Populate the return status
    return ret;                             -- Return the Return value and/or status
end';



$wp = new WritePolicy();

//register a udf
$client->registerUdf($wp, $udfBody, "udf1.lua", UdfLanguage::lua());

$ns = "test";
$set = "udf-set";
$key = new Key($ns, $set, "key1");

$bin1 = new Bin("bin1", 20);
$bin2 = new Bin("bin2", 1);
$client->put($wp, $key, [$bin1, $bin2]);

//execute a udf
$res = $client->udfExecute($wp, $key, "udf1", "testFunc1", [2]);
echo "\n Execute UDF result: ";
var_dump($res);

usleep(300000);

echo "\n Get record result: ";
$rp = new ReadPolicy();
$rec = $client->get($rp, $key);
var_dump($rec->bins);

//list all udf
$listUdf = $client->listUdf($rp);
echo "\n Count of udf: ",count($listUdf);
var_dump($listUdf);
