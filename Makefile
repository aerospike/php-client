# Aerospike for PHP — build, test and stub generation.
#
#     make help
#
# Everything here is run from `aerospike-php/`, which is a **self-contained cargo
# workspace** — see `Cargo.toml`. The wire contract and the daemon are its members; the
# extension is deliberately excluded, because it links the Zend API and a plain
# `cargo build` here must not need a PHP toolchain. So `ext` has its own target
# directory, and this file is what ties the two builds together.
#
# The one rule that catches everyone: **the extension and the daemon must be built
# the same way and be the same version.** Wakeup mode is a compile-time feature on
# both halves, and the shared-memory service name embeds the version, so a mismatched
# pair cannot meet at all. `make test-shm-poll` rebuilds both; do not rebuild one.

instance  ?= default
ns        ?= test
scns      ?= testsc
config    ?= aerospike-daemon.toml
workers   ?= 4

# ===== the cluster the tests run against =====================================
#
# Nothing here names a server. The tests that need one — the Rust integration
# tests and every script that starts a throwaway daemon — read these variables,
# so one cluster serves the whole suite and a different one is a variable away.
#
# Defaults come from `$(config)`, which is already pointed at your cluster for
# `make daemon`: the live suites talk to *that* daemon, so taking the seed from
# anywhere else would mean `make test` silently exercising two different servers.
# (Only the single-line `host = "…"` form is read; with the `hosts = […]` array,
# pass hosts= yourself.) Any of them can be overridden, on the command line or
# from the environment:
#
#     make test hosts=10.0.0.5:3000 user=tester password=hunter2
#     AEROSPIKE_HOSTS=10.0.0.5:3000 make test
#
cfg-str = $(shell sed -n 's/^[[:space:]]*$(1)[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' $(config) 2>/dev/null | head -1)
cfg-bool = $(shell sed -n 's/^[[:space:]]*$(1)[[:space:]]*=[[:space:]]*\([a-zA-Z0-9]*\).*/\1/p' $(config) 2>/dev/null | head -1)

hosts     ?= $(or $(AEROSPIKE_HOSTS),$(call cfg-str,host),127.0.0.1:3000)
alternate ?= $(or $(AEROSPIKE_USE_SERVICES_ALTERNATE),$(call cfg-bool,use-services-alternate))
user      ?= $(or $(AEROSPIKE_USER),$(call cfg-str,user))
password  ?= $(or $(AEROSPIKE_PASSWORD),$(call cfg-str,password))
auth      ?= $(or $(AEROSPIKE_AUTH_MODE),$(call cfg-str,auth))

# Exported rather than prefixed onto each recipe: make echoes its recipes, and a
# password does not belong in a build log or a CI transcript.
export AEROSPIKE_HOSTS := $(hosts)
export AEROSPIKE_USE_SERVICES_ALTERNATE := $(alternate)
export AEROSPIKE_USER := $(user)
export AEROSPIKE_PASSWORD := $(password)
export AEROSPIKE_AUTH_MODE := $(auth)

# The extension's filename is the platform's. Everything that loads it goes through
# $(EXT), so a new platform is one line here rather than a dozen call sites.
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
  EXT_FILE := libaerospike_php.dylib
else
  EXT_FILE := libaerospike_php.so
endif

EXT      := $(CURDIR)/ext/target/release/$(EXT_FILE)
DAEMON   := $(CURDIR)/target/release/aerospike-php-daemon
PHP      := php -d extension=$(EXT)
ENV      := AEROSPIKE_INSTANCE=$(instance) AEROSPIKE_NAMESPACE=$(ns) AEROSPIKE_SC_NAMESPACE=$(scns)

.PHONY: help all build build-daemon build-ext build-shm-poll test test-unit test-live \
        test-shm-poll test-all smoke concurrency compat examples stubs install-cargo-php \
        daemon daemon-check lint clean

help:
	@echo 'Aerospike for PHP'
	@echo
	@echo 'Build'
	@echo '  build              both halves, release (this is what you want)'
	@echo '  build-daemon       the daemon and the wire contract only'
	@echo '  build-ext          the PHP extension only'
	@echo '  build-shm-poll     both halves with the shm-poll wakeup mode'
	@echo
	@echo 'Test                 (the live suites need a running daemon: make daemon)'
	@echo '  test               unit tests, then the live suites'
	@echo '  test-unit          Rust unit and integration tests (needs a cluster)'
	@echo '  test-live          smoke, concurrency and the 1.x compatibility suite'
	@echo '  smoke              the extension against a live daemon'
	@echo '  concurrency        the process model, with real forked processes'
	@echo '  compat             the 1.x compatibility layer'
	@echo '  examples           every example in examples/, as a test'
	@echo '  test-shm-poll      rebuild both halves shm-poll and run everything'
	@echo '  test-all           test, then test-shm-poll, then restore the default build'
	@echo
	@echo 'Other'
	@echo '  stubs              regenerate ext/aerospike-php.stubs.php'
	@echo '  install-cargo-php  install cargo-php (needs a special build on macOS)'
	@echo '  daemon             run the daemon in the foreground'
	@echo '  daemon-check       validate the config without connecting to anything'
	@echo '  lint               clippy, all three crates'
	@echo '  clean              both target directories'
	@echo
	@echo 'Variables: hosts=$(hosts) instance=$(instance) ns=$(ns) scns=$(scns)'
	@echo '           config=$(config) workers=$(workers)'
	@echo '           alternate=$(alternate) user=$(user) auth=$(auth)'
	@echo '           (cluster defaults are read from $(config))'

all: build

# ===== build =================================================================

build: build-daemon build-ext

build-daemon:
	cargo build --release -p aerospike-php-daemon

build-ext:
	cd ext && cargo build --release

# Both halves, together and deliberately: the wakeup mode decides whether the daemon
# has an event port to notify at all, so a worker built one way cannot be woken by a
# daemon built the other. Mixing them is a hang, not an error message.
build-shm-poll:
	cargo build --release -p aerospike-php-daemon --features shm-poll
	cd ext && cargo build --release --features shm-poll

# ===== test ==================================================================

# Builds first, so this cannot report a pass for a stale extension.
#
# The individual live targets below deliberately do *not* depend on `build-ext`: they
# are reused by `test-shm-poll`, where a dependency on the default build would rebuild
# the extension without the feature part-way through and silently revert the wakeup
# mode being tested. They check that the extension exists and leave which build it is
# to the caller.
test: build test-unit test-live

# The daemon's integration tests drive the real serving loop over real shared memory
# with no PHP involved, which is what lets the transport be validated independently of
# the extension. They need a cluster; the ipc and ext tests do not.
test-unit:
	cargo test -p aerospike-php-ipc -p aerospike-php-daemon
	cd ext && cargo test

test-live: smoke concurrency compat examples

smoke: $(EXT)
	cd ext && $(ENV) $(PHP) tests/smoke.php

# Starts and stops its own daemon with a deliberately tiny `max-workers`, because that
# is iceoryx2's client limit and is fixed when the service is created — proving the
# boundary against the default of 64 would mean holding 65 PHP processes attached.
concurrency: $(EXT)
	cd ext && AEROSPIKE_NAMESPACE=$(ns) \
		PHP_EXTENSION=$(EXT) DAEMON=$(DAEMON) tests/concurrency.sh $(workers)

# The examples are run as tests, as the Rust client's are: one nobody executes rots
# silently as the API moves, and an example is the first thing a new user copies. An
# example that cannot run here (a transaction with no strong-consistency namespace)
# reports `skip` rather than passing quietly — hence `scns` being passed through.
examples: $(EXT)
	AEROSPIKE_NAMESPACE=$(ns) AEROSPIKE_SC_NAMESPACE=$(scns) PHP_EXTENSION=$(EXT) \
		$(PHP) examples/run-all.php

# Written throughout in the *old* idiom on purpose: a test written in the new one
# would pass with the compatibility layer unloaded, and prove nothing.
compat: $(EXT)
	cd compat && $(ENV) $(PHP) tests/compat.php

# The live suites here get their **own** daemon, built the same way, under its own
# instance name. Not a convenience: a worker and a daemon built with different wakeup
# modes do not report an error, the worker just never learns its reply is ready — so
# "restart the daemon first" is a step that must not be left to whoever runs this.
test-shm-poll: build-shm-poll
	cargo test -p aerospike-php-ipc -p aerospike-php-daemon --features shm-poll
	cd ext && cargo test --features shm-poll
	DAEMON=$(DAEMON) tools/with-daemon.sh shmpoll \
		env AEROSPIKE_NAMESPACE=$(ns) AEROSPIKE_SC_NAMESPACE=$(scns) $(PHP) ext/tests/smoke.php
	DAEMON=$(DAEMON) tools/with-daemon.sh shmpoll \
		env AEROSPIKE_NAMESPACE=$(ns) $(PHP) compat/tests/compat.php
	$(MAKE) concurrency

# Both wakeup modes, end to end. The default build is restored last, because leaving
# an shm-poll extension in place makes every later run pin a core while it waits.
test-all:
	$(MAKE) test
	$(MAKE) test-shm-poll
	$(MAKE) build
	@echo
	@echo 'both wakeup modes exercised; the default build is restored'

# ===== stubs =================================================================

# `ext/aerospike-php.stubs.php` is checked in, so nothing has to be generated to get
# editor completion. This regenerates it after the PHP-facing surface changes.
#
# It is never autoloaded: the extension provides these classes, so loading both is a
# duplicate-declaration error. It is input to a static analyser, not to a program.
stubs:
	cd ext && tools/gen-stubs.sh

# cargo-php needs two things on macOS that its own instructions do not mention.
#
# The link flag: it links ext-php-rs into an *executable*, so PHP's symbols are wanted
# at link time and there is no PHP to supply them. Without this it does not build.
#
# And **not `--locked`**, which pins an older ext-php-rs than this crate uses.
# `describe::abi::Vec` is passed across the library boundary and dropped on cargo-php's
# side, so a version skew is `pointer being freed was not allocated` at exit — a wild
# pointer, with nothing to suggest a version is involved.
install-cargo-php:
	RUSTFLAGS="-C link-arg=-Wl,-undefined,dynamic_lookup" cargo install cargo-php --force

# ===== running the daemon ====================================================

$(config):
	@echo 'no $(config) yet — copying the documented example'
	cp daemon/aerospike-daemon.toml.example $(config)
	@echo
	@echo ">>> Edit $(config): point 'host' at your cluster, then rerun."
	@false

daemon: $(config) build-daemon
	$(DAEMON) --config $(config)

# Unknown keys are a startup error rather than a warning, and the TLS certificate
# files are read while the configuration is validated — so this catches a wrong path,
# which is the only time anyone is looking at it.
daemon-check: $(config) build-daemon
	$(DAEMON) --config $(config) --check

# ===== housekeeping ==========================================================

# Clippy only. Formatting is deliberately not a target here: `cargo fmt` churn buries
# the diffs these changes are reviewed as.
lint:
	cargo clippy --all-targets
	cd ext && cargo clippy --all-targets

clean:
	cargo clean
	cd ext && cargo clean

# A missing extension is the most common reason a live target fails, and the error PHP
# gives for it names a path rather than a build step.
$(EXT):
	@echo 'the extension is not built at $(EXT)'
	@echo '  make build-ext'
	@false
