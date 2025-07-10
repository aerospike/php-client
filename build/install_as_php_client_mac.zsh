#!/bin/zsh -m
# Aerospike PHP8 install and build script for MacOS (darwin)

set +e

SCRIPT_PATH="${0:A:h}"
PROJ_FOLDER="php-client"


#install XCode command-line developer tools, if needed:
xcode-select -p 1>/dev/null
if [ $? -ne 0 ]; then
  printf 'xcode tools were not found.  Attempting to install them.  Please look for the confirmation dialogue...\n'
  xcode-select --install

  printf 'xcode-select install has been requested.  Please look for the confirmation dialogue...\n'
  until $(xcode-select --print-path &> /dev/null); do
    printf 'waiting for xcode-slect to finish installation...\n'
    sleep 10;
  done
else
  printf 'XCode tools are already installed!\n'
fi


#determine if the script is being run via direct download or from within the repo
if [[ $SCRIPT_PATH == *$PROJ_FOLDER* ]]; then
  echo "script is in the repo - no need to clone"
else
  echo "script is NOT in the repo - cloning..."
  #clone repo & cd into project folder:
  if ! git clone https://github.com/aerospike/php-client.git "$PROJ_FOLDER" 2>/dev/null && [ -d "$PROJ_FOLDER" ] ; then
    printf 'Git clone failed. Target folder exists. Assuming clone was already completed & continuing...\n'
  fi
  cd $SCRIPT_PATH/$PROJ_FOLDER
fi


if [[ $PWD != *$PROJ_FOLDER* && -d $PROJ_FOLDER ]]; then
  cd $PROJ_FOLDER
else
  if [[ $PWD == *build ]]; then
    echo "running in build directory!"
    cd ..
  fi
fi

pwd
#NOTE: we should now be in the project root, regardless of where the script is or where it was run from

# Install Homebrew if not already installed
which -s brew
if [[ $? != 0 ]] ; then
    mkdir ~/homebrew
    curl -L https://github.com/Homebrew/brew/tarball/master | tar xz --strip 1 -C ~/homebrew
    wait
    echo 'export PATH=$PATH:~/homebrew/bin' >> ~/.zshrc
    source ~/.zshrc
fi
brew update
wait


#install PHP 8 via Homebrew & fix up env (zsh shown), if needed
if ! php -v | grep -q 'PHP 8'; then
  printf 'PHP 8 was not installed.\n '
  if ! which curl | grep -q 'homebrew'; then
    printf 'Installing brew curl...\n'
      brew install curl
      wait
      echo 'export PATH=~/homebrew/opt/curl/bin:$PATH' >> ~/.zshrc
      source ~/.zshrc
  fi
  printf 'Installing PHP 8.4...\n'
  brew install php@8.4
  wait
  echo 'export PATH=~/homebrew/opt/php@8.4/bin:$PATH' >> ~/.zshrc
  echo 'export PATH=~/homebrew/opt/php@8.4/sbin:$PATH' >> ~/.zshrc
else
  printf 'PHP 8 was already installed!\n'
fi


#install PHPUnit via Homebew, if needed
which -s phpunit
if [[ $? != 0 ]] ; then
  printf 'phpunit was not installed.  Installing phpunit...\n'
  brew install phpunit
  wait
else
  printf 'phpunit was already installed!\n'
fi


#install latest rustup via Homebrew, if needed
which -s rustup
if [[ $? != 0 ]] ; then
  printf 'rustup was not installed.  Installing rustup...\n'
  brew install rustup
  wait
  echo 'export PATH=~/homebrew/opt/rustup/bin/:$PATH' >> ~/.zshrc
  source ~/.zshrc
else
  printf 'rustup was already installed!\n'
fi


#install Rust and the Rust Toolchain via rustup - standard installation is recommended (Option 1), if needed
which -s rustc
if [[ $? != 0 ]] ; then
  printf 'rust was not installed.  Installing rust...\n'
  rustup-init -y
  wait
  #update env:
  . "$HOME/.cargo/env"
else
  printf 'rust was already installed!\n'
fi


#repair broken cargo / toolchain, if needed
cargo -v > /dev/null
if [[ $? != 0 ]] ; then
  printf 'cargo was not installed successfully. fixing... \n'
  rustup install stable
  wait
  rustup default stable
  wait
fi


#NOTE:  For ARM-based macs, the following must be done for Cargo to be able to build successfully:
mkdir -p ~/.cargo
touch ~/.cargo/config.toml
if ! cat ~/.cargo/config.toml | grep -q 'aarch64'; then
  printf '[target.aarch64-apple-darwin]
  rustflags = [
    "-C", "link-arg=-undefined",
    "-C", "link-arg=dynamic_lookup",
  ]\n' >> ~/.cargo/config.toml
fi


#Install go and fix up env (zsh example shown), if needed:
which -s go
if [[ $? != 0 ]] ; then
  printf 'go was not installed.  Installing go...\n'
  brew install go
  wait
  echo 'export PATH="$PATH:$(go env GOPATH)/bin"' >> ~/.zshrc
  source ~/.zshrc
else
  printf 'go was already installed!\n'
fi


# Install protocol buffer compiler & plugins & latest grpc package
cd aerospike-connection-manager
brew install protobuf
wait
go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
wait
go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest
wait
go get -u google.golang.org/grpc
wait


#Build & install PHP client
cd ..
make
#build and run the ACM
cd aerospike-connection-manager
make

echo "Installation complete!"

# TODO:
# configure your Aerospike Server in php-client/aerospike-connection-manager/asld.toml
# Once configured, run the ACM again with:
# cd php-client/aerospike-connection-manager
# make run
