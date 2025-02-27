#!/bin/bash -m

set +e
set -x

apt update

#install git, if needed
which -s git
if [[ $? != 0 ]] ; then
  printf 'git was not installed.  Installing git...\n'
  apt -y install git
  wait
else
  printf 'git was already installed!\n'
fi

#install PHP 8 if needed
if ! php -v | grep -q 'PHP 8'; then
  printf 'PHP 8 was not installed.\n'
  if apt-cache search php8.4 | grep -q 'php8.4'; then
    printf 'Installing PHP 8.4...\n'
    apt -y install php8.4
    wait
  elif apt-cache search php8.3 | grep -q 'php8.3'; then
    printf 'Installing PHP 8.3...\n'
    apt -y install php8.3
    wait
  fi
else
  printf 'PHP 8 was already installed!\n'
fi


#install php-dev, if needed
which php-config
if [[ $? != 0 ]] ; then
  printf 'php-dev was not installed.\n'
  printf 'Installing php-dev...\n'
  apt -y install php-dev
  wait
else
  printf 'php-dev was already installed!\n'
fi


#install PHPUnit, if needed
which -s phpunit
if [[ $? != 0 ]] ; then
  printf 'phpunit was not installed.  Installing phpunit...\n'
  apt -y install phpunit
  wait
else
  printf 'phpunit was already installed!\n'
fi


#install curl, if needed
which -s curl
if [[ $? != 0 ]] ; then
  printf 'curl was not installed.  Installing curl...\n'
  apt -y install curl
  wait
else
  printf 'curl was already installed!\n'
fi


#install latest rustup via curl, if needed
which -s rustup
if [[ $? != 0 ]] ; then
  printf "rustup was not installed.  Installing rustup...\n"
  curl https://sh.rustup.rs -sSf | sh -s -- -y
  . "$HOME/.cargo/env"
  wait
else
  printf "rustup was already installed!\n"
fi


#install latest go, if needed
which -s go
if [[ $? != 0 ]] ; then
  printf "go was not installed.  Installing go...\n"
  apt-get -y install golang-go
  wait
else
  printf "go was already installed!\n"
fi


#install build-essential meta package, if needed
which -s make
if [[ $? != 0 ]] ; then
  printf "build-essential was not installed.  Installing build-essential...\n"
  apt-get -y install build-essential
  wait
else
  printf "build-essential was already installed!\n"
fi


#install build-essential meta package, if needed
which -s clang
if [[ $? != 0 ]] ; then
  printf "clang was not installed.  Installing clang...\n"
  apt-get -y install clang
  wait
else
  printf "clang was already installed!\n"
fi


# cd to Aerospike Conenction Manager folder:
cd ../aerospike-connection-manager


#install protobuf, if needed
which -s protoc
if [[ $? != 0 ]] ; then
  printf "protobuf was not installed.  Installing protobuf...\n"
  apt -y install protobuf-compiler
  wait
else
  printf "protobuf was already installed!\n"
fi


go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
wait
go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest
wait
go get google.golang.org/grpc
wait

#fix up the protobuf symlinkage, if needed
which protoc-gen-go
if [[ $? != 0 ]] ; then
  printf "protobufs not symlinked. Symlinking protobufs...\n"
  ln -s /root/go/bin/protoc-gen-go /usr/bin/
  ln -s /root/go/bin/protoc-gen-go-grpc /usr/bin/
  wait
else
  printf "protobuf was already installed!\n"
fi


#Build & install PHP client
cd ..
make

#build and run the ACM
cd ./aerospike-connection-manager
make

#note: you should see "ResultCode: INVALID_NODE_ERROR" which just means you need to
# configure your Aerspike Server in asdl.toml.  Once configured run the ACM again with
# make run
# when ready to deploy as a service run this:
# sudo make demonize
