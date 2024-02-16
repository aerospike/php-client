<?php 

namespace Aerospike;
use PHPUnit\Framework\TestCase;

final class BatchOpsTest extends TestCase
{
    protected static $client;

    protected static $namespace = "test";
    protected static $set = "test";
    protected static $socket = "/tmp/asld_grpc.sock";

    public static function generateRandomReport() {
        $shape = ["circle", "triangle", "square"];
        $summary = "Summary " . mt_rand(1000, 9999);
        $city = "City " . mt_rand(1, 100);
        $state = "State " . mt_rand(1, 50);
        $duration = mt_rand(1, 24) . " hours";
        return [
            "shape" => $shape,
            "summary" => $summary,
            "city" => $city,
            "state" => $state,
            "duration" => $duration
        ];
    }

    public static function setUpBeforeClass(): void
    {
        $numRecords = 10;
        try {
            self::$client = Client::connect(self::$socket);
        } catch (\Exception $e) {
            throw $e;
        }
        for ($i = 1; $i <= $numRecords; $i++) {
            $key = new Key(self::$namespace, self::$set, "record_$i");
        
            // Generate random data for Occurred, Reported, and Posted fields
            $occurred = mt_rand(1000000000, time());
            $reported = mt_rand(1000000000, time());
            $posted = mt_rand(1000000000, time());
        
            // Generate random data for the Report map
            $reportData = self::generateRandomReport();
        
            // Define bins
            $bins = [
                new Bin("Occurred", $occurred),
                new Bin("Reported", $reported),
                new Bin("Posted", $posted),
                new Bin("Report", $reportData)
            ];
        
            // Write the record
            $wp = new WritePolicy();
            self::$client->put($wp, $key, $bins);
        }
    }

    public function testBatchOpsRead(){
        $brp = new BatchReadPolicy();

        $bkey = new Key(self::$namespace, self::$set, 1);
        $batchRead = new BatchRead($brp, $bkey);
        
        $bp = new BatchPolicy();
        $recs = $client->batch($bp, [$batchRead]);
        var_dump($recs);
    }


}
