<?php

/**
 * CDT path expressions: JSONPath-style selection over a nested document.
 *
 * Port of the Rust client's `path_expression` example. Needs Aerospike 8.1.1+;
 * skips cleanly on anything older.
 *
 * The goal is the classic JSONPath bookstore query:
 *
 *     $.book[?(@.price <= 10)].title
 */

declare(strict_types=1);

require_once __DIR__ . '/_bootstrap.php';

use Aerospike\{Bin, Ctx, Exp, ExpMap, ExpOp, ExpPath, ExpType, Key, LoopVarPart,
    MapReturn, OrderedMap};

$client = example_client();
$ns = example_namespace();

if (!server_at_least($client, '8.1.1')) {
    skip('CDT path expressions need server 8.1.1+, this is ' . server_version($client));
}

$key = new Key($ns, 'path_demo', 'bookstore');
$client->delete(null, $key);

section('store the document');
$books = [
    new OrderedMap(['title' => 'Sayings of the Century', 'price' => 8.95]),
    new OrderedMap(['title' => 'Sword of Honour', 'price' => 12.99]),
    new OrderedMap(['title' => 'Moby Dick', 'price' => 8.99]),
    new OrderedMap(['title' => 'The Lord of the Rings', 'price' => 22.99]),
];
$client->put(null, $key, [new Bin('store', new OrderedMap(['book' => $books]))]);
out('books stored', count($books));

section('select the titles of books cheaper than 10.00');
/*
 * The path is a list of contexts, read left to right:
 *
 *   store["book"]           a map key
 *     -> every element      all children, filtered on the element's price
 *       -> the "title" key  a map key
 *
 * A *filtered* context is what distinguishes a path expression from a plain CDT
 * context: the filter is an expression the server evaluates once per child, and
 * `Exp::loopVar()` is how that expression names the child it is being applied to.
 * Here the child is a book (a map), so the loop variable is read as a map and the
 * price pulled out of it.
 *
 * Note that a path expression produces a *value*, not a bin — so it is read back
 * with `ExpOp::read()`, which evaluates an expression server-side and returns the
 * result under a name of your choosing. The same expression can equally be a policy
 * filter, where the value decides whether the record is visible at all.
 */
$titlesUnderTen = ExpPath::selectValues(
    ExpType::ListType,
    Exp::mapBin('store'),
    [
        Ctx::mapKey('book'),
        Ctx::allChildrenWithFilter(Exp::le(
            ExpMap::getByKey(MapReturn::Value, ExpType::Double, Exp::stringVal('price'),
                Exp::loopVar(ExpType::MapType, LoopVarPart::Value)),
            Exp::floatVal(10.0),
        )),
        Ctx::mapKey('title'),
    ],
);

$result = $client->operate(null, $key, [ExpOp::read('titles', $titlesUnderTen)]);
out('titles under 10.00', $result->bin('titles'));

section('the same path as a policy filter');
// `Exp::eq(ExpList::size(...), ...)` turns the selection into a yes/no test, which is
// what a filter needs: the record is returned only when two books are under 10.00.
$filter = Exp::eq(Aerospike\ExpList::size($titlesUnderTen), Exp::intVal(2));
$found = $client->get(new Aerospike\ReadPolicy(filterExp: $filter), $key);
out('record visible through the filter', $found !== null);

$client->delete(null, $key);
