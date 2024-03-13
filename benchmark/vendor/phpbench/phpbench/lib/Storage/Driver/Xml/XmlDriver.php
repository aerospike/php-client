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

namespace PhpBench\Storage\Driver\Xml;

use PhpBench\Dom\Document;
use PhpBench\Model\SuiteCollection;
use PhpBench\Serializer\XmlDecoder;
use PhpBench\Serializer\XmlEncoder;
use PhpBench\Storage\DriverInterface;
use Symfony\Component\Filesystem\Filesystem;

/**
 * XML storage driver.
 *
 * The collections are sharded by year, month and day in order that we can
 * effectively sort them without hydrating all of the results.
 */
class XmlDriver implements DriverInterface
{
    public const UUID_LENGTH = 40;

    private $path;
    private $xmlEncoder;
    private $xmlDecoder;
    private $filesystem;

    public function __construct($path, XmlEncoder $xmlEncoder, XmlDecoder $xmlDecoder, Filesystem $filesystem = null)
    {
        $this->path = $path;
        $this->xmlEncoder = $xmlEncoder;
        $this->xmlDecoder = $xmlDecoder;
        $this->filesystem = $filesystem ?: new Filesystem();
    }

    public function store(SuiteCollection $collection): ?string
    {
        foreach ($collection->getSuites() as $suite) {
            $path = $this->getPath($suite->getUuid());

            if (false === $this->filesystem->exists(dirname($path))) {
                $this->filesystem->mkdir(dirname($path));
            }

            $collection = new SuiteCollection([$suite]);
            $dom = $this->xmlEncoder->encode($collection);
            $dom->save($path);
        }

        return null;
    }

    /**
     * {@inheritdoc}
     */
    public function fetch(string $runId): SuiteCollection
    {
        if (!$this->has($runId)) {
            throw new \InvalidArgumentException(sprintf(
                'Cannot find run with reference "%s"',
                $runId
            ));
        }

        $path = $this->getPath($runId);

        $dom = new Document();
        $dom->load($path);
        $collection = $this->xmlDecoder->decode($dom);

        return $collection;
    }

    /**
     * {@inheritdoc}
     */
    public function has($runId): bool
    {
        $path = $this->getPath($runId);

        if (null === $path) {
            return false;
        }

        return $this->filesystem->exists($path);
    }

    /**
     * {@inheritdoc}
     */
    public function history(): \PhpBench\Storage\HistoryIteratorInterface
    {
        return new HistoryIterator($this->xmlDecoder, $this->path);
    }

    private function getPath(string $uuid): ?string
    {
        if (strlen($uuid) !== self::UUID_LENGTH) {
            return null;
        }

        try {
            $date = new \DateTime((string) hexdec(substr($uuid, 0, 7)));
        } catch (\Exception $e) {
            return null;
        }

        return sprintf(
            '%s/%s/%s/%s/%s.xml',
            $this->path,
            dechex((int) $date->format('Y')),
            dechex((int) $date->format('m')),
            dechex((int) $date->format('d')),
            $uuid
        );
    }
}
