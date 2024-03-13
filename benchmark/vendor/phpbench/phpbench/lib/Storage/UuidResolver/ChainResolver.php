<?php

/*
 * This file is part of the PHPBench package
 *
 * (c) Daniel Leech <daniel@dantleech.com>
 *
 * For the full copyright and license information, please view the LICENSE
 * file that was distributed with this source code.
 *
 */

namespace PhpBench\Storage\UuidResolver;

use PhpBench\Storage\UuidResolverInterface;

class ChainResolver implements UuidResolverInterface
{
    /**
     * @var array
     */
    private $resolvers = [];

    public function __construct(array $resolvers)
    {
        $this->resolvers = $resolvers;
    }

    public function resolve(string $reference): ?string
    {
        /** @var UuidResolverInterface $resolver */
        foreach ($this->resolvers as $resolver) {
            $ref = $resolver->resolve($reference);

            if (null === $ref) {
                continue;
            }

            return $ref;
        }

        return null;
    }
}
