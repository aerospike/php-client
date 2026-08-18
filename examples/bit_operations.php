<?php

/**
 * Bitwise operations on blob bins.
 *
 * Port of the Rust client's `bit_operations` example, which ports the Java
 * client's OperateBit example.
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{Bin, BitOp, Blob, Key};

$client = example_client();
$ns = example_namespace();
$key = new Key($ns, 'bit_ops', 'demo');
$client->delete(null, $key);

// A PHP string cannot say whether it is text or bytes, so a bare string is written
// as text. `Blob` is how you write bytes — which is what a bitwise bin must be.
$bytes = new Blob(pack('C*', 0b0000_0001, 0b0100_0010, 0b0000_0011, 0b0000_0100, 0b0000_0101));
$client->put(null, $key, [new Bin('bits', $bytes)]);
out('stored 5 bytes', '00000001 01000010 00000011 00000100 00000101');

section('read');
$result = $client->operate(null, $key, [BitOp::get('bits', 9, 5)]);
out('get(offset 9, 5 bits)', bin2hex($result->bin('bits')->bytes()));

$result = $client->operate(null, $key, [BitOp::count('bits', 0, 40)]);
out('count of set bits across all 40', $result->bin('bits'));

section('modify');
$result = $client->operate(null, $key, [
    BitOp::or('bits', 0, 8, new Blob(pack('C', 0b1010_0000))),
    BitOp::get('bits', 0, 8),
]);
out('or 0xA0 into byte 0, then read it', bin2hex($result->bin('bits')->bytes()));

$result = $client->operate(null, $key, [
    BitOp::lshift('bits', 8, 8, 1),
    BitOp::get('bits', 8, 8),
]);
out('left-shift byte 1 by one, then read it', bin2hex($result->bin('bits')->bytes()));

section('integer view');
$result = $client->operate(null, $key, [BitOp::getInt('bits', 16, 8, false)]);
out('byte 2 read as a signed integer', $result->bin('bits'));

$client->delete(null, $key);
