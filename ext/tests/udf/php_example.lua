-- Copyright 2015-2026 Aerospike, Inc.
--
-- Licensed under the Apache License, Version 2.0 (the "License"); you may not
-- use this file except in compliance with the License. You may obtain a copy of
-- the License at http://www.apache.org/licenses/LICENSE-2.0

-- The module `tests/smoke.php` registers, calls and removes.
--
-- Deliberately small: what the PHP tests are checking is the *plumbing* — that a
-- module registers on every node, that arguments and return values cross the
-- daemon intact, that a background query applies a function to a whole set, and
-- that the tasks reporting on all of that work. The Lua itself is not under test.

local function putBin(rec, name, value)
    if not aerospike:exists(rec) then aerospike:create(rec) end
    rec[name] = value
    aerospike:update(rec)
end

-- Write one bin, and return nothing. Tests the "a UDF that returns nil" case.
function writeBin(rec, name, value)
    putBin(rec, name, value)
end

-- Read one bin back, so a return value's type can be checked across the hop.
function readBin(rec, name)
    return rec[name]
end

-- Return the argument untouched. Every Aerospike value shape goes through this
-- on its way out and back, which is what pins the argument conversion.
function echo(rec, value)
    return value
end

-- Add to a bin, creating it at zero. Used by the background query, so that
-- applying it to a whole set has a visible, countable effect.
function incrementBin(rec, name, amount)
    if not aerospike:exists(rec) then
        aerospike:create(rec)
        rec[name] = 0
    end
    if rec[name] == nil then
        rec[name] = 0
    end
    rec[name] = rec[name] + amount
    aerospike:update(rec)
end

-- Fail on purpose, with the server's `code:message` convention. Tests that a
-- UDF's own error reaches PHP as a server failure rather than as a success.
function refuse(rec)
    error("1000:this function always refuses")
end
