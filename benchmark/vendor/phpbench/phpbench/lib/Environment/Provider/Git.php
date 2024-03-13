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

namespace PhpBench\Environment\Provider;

use PhpBench\Environment\ProviderInterface;
use PhpBench\Environment\VcsInformation;
use Symfony\Component\Process\ExecutableFinder;
use Symfony\Component\Process\Process;

/**
 * Return information about the git environment.
 */
class Git implements ProviderInterface
{
    private $exeFinder;
    private $exeName;
    private $exePath;

    /**
     * @var string
     */
    private $cwd;

    public function __construct(string $cwd, ExecutableFinder $exeFinder = null, $exeName = 'git')
    {
        $this->exeFinder = $exeFinder ?: new ExecutableFinder();
        $this->exeName = $exeName;
        $this->cwd = $cwd;
    }

    /**
     * {@inheritdoc}
     */
    public function isApplicable(): bool
    {
        if (null === $this->getGitPath()) {
            return false;
        }

        $index = sprintf('%s/.git', $this->cwd);

        if (file_exists($index)) {
            return true;
        }

        return false;
    }

    /**
     * {@inheritdoc}
     */
    public function getInformation(): \PhpBench\Environment\Information
    {
        $process = $this->exec('symbolic-ref HEAD');

        if (0 !== $process->getExitCode() && stristr($process->getErrorOutput(), 'ref HEAD is not')) {
            $branchName = '(unnamed branch)';
        } elseif (0 === $process->getExitCode()) {
            preg_match('{^refs/heads/(.*)$}', $process->getOutput(), $matches);
            $branchName = $matches[1];
        } else {
            throw new \RuntimeException(sprintf(
                'Encountered error when determining git branch exit code: %s, stderr: "%s"',
                $process->getExitCode(),
                $process->getErrorOutput()
            ));
        }

        $commitshRef = sprintf(
            '%s/%s/%s',
            $this->cwd,
            '.git/refs/heads',
            $branchName
        );

        if (!file_exists($commitshRef)) {
            $version = null;
        } else {
            $version = trim(file_get_contents($commitshRef));
        }

        return new VcsInformation('git', $branchName, $version);
    }

    private function exec($cmd): Process
    {
        $cmd = sprintf('%s %s', escapeshellarg($this->getGitPath()), $cmd);
        $process = Process::fromShellCommandline($cmd, $this->cwd);
        $process->run();

        return $process;
    }

    private function getGitPath(): ?string
    {
        if (null !== $this->exePath) {
            return $this->exePath;
        }

        $this->exePath = $this->exeFinder->find($this->exeName, null);

        return $this->exePath;
    }
}
