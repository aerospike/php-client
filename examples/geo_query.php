<?php

/**
 * Geospatial queries against a geo2dsphere index.
 *
 * Port of the Rust client's `geo_query` example: points within a region, points
 * within a radius, and stored regions containing a point.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{AerospikeException, Bin, Filter, GeoJson, IndexType, Key, Statement};

$client = example_client();
$ns = example_namespace();
$set = 'geo_demo';
$bin = 'loc';
$index = 'geo_demo_loc_idx';

section('set up: a geo2dsphere index and a few points');
try {
    $client->dropIndex(null, $ns, $set, $index)->waitTillComplete(10_000);
} catch (AerospikeException) {
}
$client->createIndexOnBin(null, $ns, $set, $bin, $index, IndexType::Geo2DSphere)
       ->waitTillComplete(20_000);

$points = [
    'sf-office' => [-122.40, 37.79],
    'oakland'   => [-122.27, 37.80],
    'san-jose'  => [-121.89, 37.34],
    'la'        => [-118.24, 34.05],   // far outside the Bay Area
];
foreach ($points as $name => [$lng, $lat]) {
    $point = json_encode(['type' => 'Point', 'coordinates' => [$lng, $lat]]);
    $client->put(null, new Key($ns, $set, $name), [new Bin($bin, new GeoJson($point))]);
}
out('points stored', array_keys($points));

/** How many records a filter matches. */
function matching(Aerospike\Client $client, string $ns, string $set, Filter $filter): int
{
    $count = 0;
    foreach ($client->query(null, null, new Statement($ns, $set, null, $filter)) as $ignored) {
        $count++;
    }
    return $count;
}

$bayArea = json_encode([
    'type' => 'Polygon',
    'coordinates' => [[[-123.0, 37.0], [-121.5, 37.0], [-121.5, 38.2], [-123.0, 38.2], [-123.0, 37.0]]],
]);

section('points within a region');
out('inside the Bay Area polygon', matching($client, $ns, $set, Filter::geoWithinRegion($bin, $bayArea)));

section('points within a radius');
out('within 50km of San Jose',
    matching($client, $ns, $set, Filter::geoWithinRadius($bin, -121.89, 37.33, 50_000.0)));

section('stored regions containing a point');
// The same index answers the inverse question, when the stored value is a polygon
// rather than a point.
$client->put(null, new Key($ns, $set, 'bay-area-region'), [new Bin($bin, new GeoJson($bayArea))]);
$sfOffice = json_encode(['type' => 'Point', 'coordinates' => [-122.40, 37.79]]);
out('stored regions containing the SF office',
    matching($client, $ns, $set, Filter::geoContains($bin, $sfOffice)));

section('cleanup');
$client->dropIndex(null, $ns, $set, $index)->waitTillComplete(10_000);
$client->truncate(null, $ns, $set);
out('index dropped, set truncated');
