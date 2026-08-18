#!/usr/bin/env bash
#
# Generate `aerospike-php.stubs.php`, the PHP declarations an IDE and PHPStan read.
#
#     tools/gen-stubs.sh
#
# The stub file is *never loaded at runtime* — the extension itself provides these
# classes, so loading both would be a duplicate-declaration error. It exists only for
# static analysis. See ext/README.md.
#
# Three things here are not obvious, and each cost an afternoon:
#
# 1. **`--features stub-gen`.** `cargo php stubs` `dlopen`s the built library and
#    calls `ext_php_rs_describe_module` to enumerate the classes. On macOS the
#    library is linked with `-undefined dynamic_lookup`, so PHP's own symbols are
#    deliberately left unresolved for the PHP binary to supply — and cargo-php is not
#    PHP. The `stub-gen` feature defines those symbols so the library can be loaded
#    by something else. See `php_globals_for_tests` in `src/lib.rs`.
#
# 2. **A separate target directory.** The library built this way must never reach
#    PHP: its PHP globals are zeroed slabs and its engine functions abort. Building
#    into `target/stub-gen` keeps that artifact out of `target/debug`, where someone
#    might reasonably point `php -d extension=` at it.
#
# 3. **The post-processing step**, explained where it happens.
#
# cargo-php itself needs care on macOS. It links ext-php-rs into an *executable*, so
# it hits the same missing-symbol problem, and it must be built against the **same
# ext-php-rs version as this crate** — its `describe::abi::Vec` is passed across the
# library boundary and dropped on cargo-php's side, so a version skew is a wild
# pointer freed at exit rather than an error message:
#
#     RUSTFLAGS="-C link-arg=-Wl,-undefined,dynamic_lookup" cargo install cargo-php --force
#
# Do not use `--locked`: it pins an older ext-php-rs and reintroduces the skew.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ext="$(dirname "$here")"
cd "$ext"

out="aerospike-php.stubs.php"
target="target/stub-gen"

command -v cargo-php > /dev/null || {
    echo "cargo-php is not installed. See the note at the top of this script — the" >&2
    echo "plain 'cargo install cargo-php' does not build on macOS." >&2
    exit 1
}

echo "building with the stub-gen feature into $target"
cargo build --features stub-gen --target-dir "$target"

# Checked one at a time rather than with a brace glob: under `set -o pipefail` a
# non-matching glob makes `ls` fail and takes the whole script with it, silently.
lib=""
for candidate in "$target/debug/libaerospike_php.dylib" "$target/debug/libaerospike_php.so"; do
    [ -f "$candidate" ] && { lib="$candidate"; break; }
done
[ -n "$lib" ] || { echo "no built library found under $target/debug" >&2; exit 1; }

echo "describing $lib"
cargo php stubs "$(pwd)/$lib" -o "$out"

# `AerospikeException` registers `message` and `code` as its own properties, to
# shadow the ones `Exception` declares — that is what keeps `getMessage()` working
# from Rust (see the type's own docs). But `Exception`'s slots are **untyped**, and
# PHP forbids an override from adding a type: parsing the generated file fails with
# "Type of Aerospike\AerospikeException::$message must be omitted to match the
# parent definition". cargo-php cannot know that, because the parent is a class it
# never sees. So drop the two types, which is what the parent has.
echo "un-typing the two properties inherited from Exception"
STUB_FILE="$out" python3 - <<'PY'
import os
import re
import sys

path = os.environ["STUB_FILE"]
source = open(path).read()

# Scoped to the one class, so a `$message` property on anything else keeps its type.
start = source.index("class AerospikeException extends \\Exception {")
end = source.index("\n    }", start) + len("\n    }")
body = source[start:end]

patched, count = re.subn(r"public (?:int|string) \$(code|message);", r"public $\1;", body)
if count != 2:
    sys.exit(
        f"expected to un-type exactly 2 properties on AerospikeException, found {count}. "
        "If cargo-php's output changed, or the class stopped shadowing them, update "
        "this step rather than deleting it — a typed override of an untyped parent "
        "property is a parse error, so the stub file would not load into any analyser."
    )

open(path, "w").write(source[:start] + patched + source[end:])
PY

# Every verb takes its policy first and accepts `null` for it, which cargo-php emits
# as `?WritePolicy $policy = null` — a default *before* required parameters. PHP
# deprecates that and says why: such a parameter "is implicitly treated as a required
# parameter". It is right. You cannot omit the policy on `put($policy, $key, $bins)`;
# you can only pass `null`. So the default is removed wherever a later parameter has
# none, which makes the stub state what is actually true and drops ~80 deprecation
# notices that would otherwise greet anyone running the file through an analyser.
echo "removing defaults that precede a required parameter"
STUB_FILE="$out" python3 - <<'PY'
import os
import re

path = os.environ["STUB_FILE"]


def split_params(text):
    """Top-level comma split, so a default of `[]` or `"a,b"` stays in one piece."""
    parts, depth, quote, current = [], 0, None, ""
    for ch in text:
        if quote:
            if ch == quote:
                quote = None
        elif ch in "'\"":
            quote = ch
        elif ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        elif ch == "," and depth == 0:
            parts.append(current)
            current = ""
            continue
        current += ch
    if current.strip():
        parts.append(current)
    return parts


def fix(match):
    head, params, tail = match.group(1), match.group(2), match.group(3)
    if not params.strip():
        return match.group(0)
    parts = split_params(params)
    has_default = ["=" in p for p in parts]
    # Only those followed by a parameter with no default at all.
    for i, part in enumerate(parts):
        if has_default[i] and not all(has_default[i + 1:]):
            parts[i] = part[: part.index("=")].rstrip()
    return head + ", ".join(p.strip() for p in parts) + tail


source = open(path).read()
patched = re.sub(
    r"(function \w+\()([^\n]*?)(\)(?:: [^\n{]+)? \{\})",
    fix,
    source,
)
open(path, "w").write(patched)
PY

# cargo-php does not emit *static methods declared on an enum*, only the cases. This
# extension has 67 of them — the 1.x compatibility factories, `ReadModeAP::one()`,
# `CommitLevel::CommitAll()` and the rest — so without this step an analyser rejects
# every 1.x call site as undefined, which is precisely the code most in need of help.
#
# They are read back from the *real* extension by reflection rather than parsed out of
# the Rust, because reflection is the thing that cannot disagree with what PHP sees.
# That needs a normally-built library: the stub-gen one must never be loaded.
echo "building a loadable library for reflection"
cargo build > /dev/null 2>&1

loadable=""
for candidate in target/debug/libaerospike_php.dylib target/debug/libaerospike_php.so; do
    [ -f "$candidate" ] && { loadable="$candidate"; break; }
done
[ -n "$loadable" ] || { echo "no loadable library under target/debug" >&2; exit 1; }

echo "adding the enum static methods cargo-php omits"
php -d extension="$(pwd)/$loadable" tools/add-enum-statics.php "$out"

echo "checking it parses"
php -l "$out" > /dev/null

classes=$(grep -cE '^    (class|enum|interface) ' "$out")
echo "wrote $out — $classes classes, enums and interfaces, $(wc -l < "$out" | tr -d ' ') lines"
