<?php

/* This feature will be added in the next release */

namespace Aerospike;

function createClient() {
    $cp = new ClientPolicy();
    $client = Aerospike($cp, "127.0.0.1:3000");
    return $client;
}

////////////////////////////////////////////////////////////////////////////////
//
// Filter -> Geo2DSphere
//
////////////////////////////////////////////////////////////////////////////////


function insertData($client) {
    echo "Inserting test data ... \n";
    $circleFormat = '{"type":"AeroCircle","coordinates":[[%f,%f], %f]}';
    $target_string = sprintf($circleFormat, -80.590000, 28.60000, 1000);
    $geoLoc = new Bin("bottom_targets", Value::geoJson($target_string));
    $key = new Key("test", "testGeo", "geoKey");
    $wp = new WritePolicy();
    $client->put($wp, $key, [$geoLoc]);

}


function getBins($client) {
    echo "Getting Test data ... \n";
    $key = new Key("test", "testGeo", "geoKey");
    $rp = new ReadPolicy();
    $record = $client->get($rp, $key);
    var_dump($record);
}

function geoFilter($client) {
    echo "Query point data ... \n";
    $lng = -80.590003;
    $lat = 28.60009;

    $point_string = '{"type":"Point","coordinates":[-80.590003, 28.60009]}';
    $statement = new Statement("test", "deliverySet");
    $statement->filters = [Filter::regionsContainingPoint("area", $point_string)];
    $qp = new QueryPolicy();
    $recordset = $client->query($qp, $statement);

    if ($recordset === false) {
        echo "Error querying the database: " . $client->error() . "\n";
        exit(1);
    }

    echo "Record set... \n";
    while ($rec = $recordset->next()) {
        var_dump($rec->bins);
    }
}

$client = createClient();
// insertData($client);
// getBins($client);
// geoFilter($client);

$client->close();