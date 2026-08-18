<?php

/**
 * Add the enum static methods `cargo php stubs` leaves out.
 *
 *     php -d extension=<lib> tools/add-enum-statics.php aerospike-php.stubs.php
 *
 * cargo-php emits an enum's *cases* but not static methods declared on it. This
 * extension has 67 of those — the 1.x compatibility factories, `ReadModeAP::one()`,
 * `CommitLevel::CommitAll()`, `IndexType::String()` and the rest — so without this
 * step an analyser rejects every 1.x call site as undefined, which is exactly the
 * code most in need of the help.
 *
 * They are read back from the **real extension by reflection** rather than parsed out
 * of the Rust, because reflection is the one source that cannot disagree with what
 * PHP itself sees. So this must run with a normally-built library loaded — never the
 * `stub-gen` one, whose engine functions abort.
 *
 * Run by `tools/gen-stubs.sh`; there is no reason to call it directly.
 */

declare(strict_types=1);

$path = $argv[1] ?? null;
if ($path === null || !is_file($path)) {
    fwrite(STDERR, "usage: add-enum-statics.php <stub-file>\n");
    exit(1);
}

$source = file_get_contents($path);
$added = 0;
$touched = 0;

foreach (get_declared_classes() as $class) {
    if (!str_starts_with($class, 'Aerospike\\') || !enum_exists($class)) {
        continue;
    }
    $short = substr($class, strlen('Aerospike\\'));

    // The enum's own block, so insertions land inside the right braces. Built by
    // concatenation rather than interpolation: `"$short[^{]"` would be read as an
    // array subscript on $short, which is a parse error rather than a bad match.
    $pattern = '/^    enum ' . preg_quote($short, '/') . '[^{]*\{.*?^    \}/ms';
    if (!preg_match($pattern, $source, $match, PREG_OFFSET_CAPTURE)) {
        continue;
    }
    [$block, $offset] = $match[0];

    $declarations = '';
    foreach ((new ReflectionClass($class))->getMethods(ReflectionMethod::IS_STATIC) as $method) {
        // PHP's own enum statics. Declaring them would shadow the real ones.
        if (in_array($method->name, ['cases', 'from', 'tryFrom'], true)) {
            continue;
        }
        if (preg_match('/function ' . preg_quote($method->name, '/') . '\(/', $block)) {
            continue;
        }

        $returnType = $method->getReturnType();
        $returns = '';
        if ($returnType instanceof ReflectionNamedType) {
            $name = $returnType->getName();
            $returns = ': ' . ($returnType->isBuiltin() ? $name : '\\' . $name);
        }

        $declarations .= "\n        /** The 1.x spelling of a case of this enum. */\n"
            . "        public static function {$method->name}(){$returns} {}\n";
        $added++;
    }

    if ($declarations === '') {
        continue;
    }
    $touched++;

    $close = strrpos($block, "\n    }");
    $patched = substr($block, 0, $close) . $declarations . substr($block, $close);
    $source = substr($source, 0, $offset) . $patched . substr($source, $offset + strlen($block));
}

file_put_contents($path, $source);
fwrite(STDERR, "  added $added static methods across $touched enums\n");
