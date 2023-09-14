#!/bin/bash

# Check PHP Version
required_php_version="8.1"

installed_php_version=$(php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;")
if [ "$installed_php_version" != "$required_php_version" ]; then
    echo "Error: PHP version $required_php_version is required, but you have PHP $installed_php_version installed."
    exit 1
fi

# Download PHP Client Code
github_repo="https://github.com/aerospike/php-client"
branch="CLIENT-2422-Code-completion"

git clone -b $branch $github_repo
cd php-client

# Run Make File
make

# Clean up: Optionally remove the cloned repository after building
rm -rf ../php-client  # Uncomment this line if you want to remove the cloned repository
