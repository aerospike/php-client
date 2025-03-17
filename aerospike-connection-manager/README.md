# Aerospike Connection Manager Setup

This guide provides step-by-step instructions on setting up the Aerospike Connection Manager (ACM).  The ACM can be run directly on the command line (test mode) or as a daemon process.

### Prerequisites

**IMPORTANT**: If you have already installed the PHP Client and ACM via the provided installation scripts, the prerequisites should already be satisfied and there is no need to build anything.

- Go programming language installed on your system
- `make` utility installed

### Configuration Instructions
Aerospike's client policy allows for flexible control over read and write operations, including optimistic concurrency, time-to-live settings, and conditional writes based on record existence. The policy may be configured in the existing asld.toml file or you may create a custom toml file.  An example asld template toml file is provided below, for reference.

1. **Using the existing asld.toml file to configure the client policy:**
    - Change directory to php-client/aerospike-connection-manager
       ```shell
       cd php-client/aerospike-connection-manager
       ```
    - Edit the asld.toml file to change the client policy to your custom values

2. **Using a custom custom_client_policy.toml file as your client policy**:
    - Copy the template from `/php-client/aerospike-connection-manager/asld.toml.template` to your `custom_client_policy.toml`
    - Make the changes you desire.
    - Change directory to `php-client/aerospike-connection-manager` and copy the custom_client_policy.toml file into this directory
    - In the make under the run  section change asld.toml to the path of your custom config file.
        ```shell
        run: clean proto
        $(GOBUILD) -o $(BINARY_NAME) -v .
        ./$(BINARY_NAME) -config-file <path-to-your-.toml-file>
        ```

### Execution Instructions

1. **Change Directory**: Navigate into the `php-client/aerospike-connection-manager` directory:
   ```shell
   cd php-client/aerospike-connection-manager
   ```

2. **Run Makefile**: Execute the default target of the Makefile to build & runthe aerospike-connection-manager:
   ```shell
   make
   ```

3. **Verify Build**: Successful build output should resemble the following:
    ```shell
    rm -f asld
    rm -f memprofile.out profile.out
    rm -rf proto asld_kvs.pb.go asld_kvs_grpc.pb.go
    find . -name "*.coverprofile" -exec rm {} +
    protoc --go-grpc_out=. --go_out=. asld_kvs.proto --experimental_allow_proto3_optional
    go build -o asld -v .
    github.com/aerospike/php-client/asld
    ./asld
    2024/02/13 10:41:30 grpc ran on unix socket protocol /tmp/asld_grpc.sock
    ```

4. **Install Service**: Once the build is verified, linux users who want to install the ACM as a service can execute this command:
    ```shell
    sudo make daemonize
    ```
&nbsp;
### Example asld.toml file:
~~~toml
# -----------------------------------------------------
#
# Aerospike Local Daemon configuration file.
#
# -----------------------------------------------------

[cluster]
socket=/tmp/cluster.sock
host = "1.1.1.1:3001,2.2.2.2:3002,3.3.3.3"
user = "default-user"
password = "default-password"
auth = "EXTERNAL"

# ClusterName sets the expected cluster ID.  If not nil, server nodes must return this cluster ID in order to
# join the client's view of the cluster. Should only be set when connecting to servers that
# support the "cluster-name" info command. (v3.10+)
cluster-name = ""

# Initial host connection timeout duration. The timeout when opening a connection
# to the server host for the first time.
timeout = "30s"

# Connection idle timeout. Every time a connection is used, its idle
# deadline will be extended by this duration. When this deadline is reached,
# the connection will be closed and discarded from the connection pool.
# The value is limited to 24 hours (86400s).
#
# It's important to set this value to a few seconds less than the server's proto-fd-idle-ms
# (default 60000 milliseconds or 1 minute), so the client does not attempt to use a socket
# that has already been reaped by the server.
#
# Connection pools are now implemented by a LIFO stack. Connections at the tail of the
# stack will always be the least used. These connections are checked for IdleTimeout
# on every tend (usually 1 second).
#
# Default: 0 seconds
idle-timeout = "0"

# LoginTimeout specifies the timeout for login operation for external authentication such as LDAP.
login-timeout = "10s"

# ConnectionQueueCache specifies the size of the Connection Queue cache PER NODE.
# Note: One connection per node is reserved for tend operations and is not used for transactions.
connection-queue-size = 100

# MinConnectionsPerNode specifies the minimum number of synchronous connections allowed per server node.
# Preallocate min connections on client node creation.
# The client will periodically allocate new connections if count falls below min connections.
#
# Server proto-fd-idle-ms may also need to be increased substantially if min connections are defined.
# The proto-fd-idle-ms default directs the server to close connections that are idle for 60 seconds
# which can defeat the purpose of keeping connections in reserve for a future burst of activity.
#
# If server proto-fd-idle-ms is changed, client ClientPolicy.IdleTimeout should also be
# changed to be a few seconds less than proto-fd-idle-ms.
min-connections-per-node = 0

# MaxErrorRate defines the maximum number of errors allowed per node per ErrorRateWindow before
# the circuit-breaker algorithm returns MAX_ERROR_RATE on database commands to that node.
# If MaxErrorRate is zero, there is no error limit and
# the exception will never be thrown.
#
# The counted error types are any error that causes the connection to close (socket errors
# and client timeouts) and types.ResultCode.DEVICE_OVERLOAD.
max-error-rate = 100

# ErrorRateWindow defined the number of cluster tend iterations that defines the window for MaxErrorRate.
# One tend iteration is defined as TendInterval plus the time to tend all nodes.
# At the end of the window, the error count is reset to zero and backoff state is removed
# on all nodes.
error-rate-window = 1

# If set to true, will not create a new connection
# to the node if there are already `ConnectionQueueSize` active connections.
# Note: One connection per node is reserved for tend operations and is not used for transactions.
limit-connections-to-queue-size = true

# Number of connections allowed to established at the same time.
# This value does not limit the number of connections. It just
# puts a threshold on the number of parallel opening connections.
# By default, there are no limits.
opening-connection-threshold = 0

# Throw exception if host connection fails during addHost().
fail-if-not-connected = true

tend-interval = "1s"

# TendInterval determines interval for checking for cluster state changes.
# UseServicesAlternate determines if the client should use "services-alternate" instead of "services"
# in info request during cluster tending.
#"services-alternate" returns server configured external IP addresses that client
# uses to talk to nodes.  "services-alternate" can be used in place of providing a client "ipMap".
# This feature is recommended instead of using the client-side IpMap above.
#
# "services-alternate" is available with Aerospike Server versions >= 3.7.1.
use-services-alternate = false

# RackAware directs the client to update rack information on intervals.
# When this feature is enabled, the client will prefer to use nodes which reside
# on the same rack as the client for read transactions. The application should also set the RackId, and
# use the ReplicaPolicy.PREFER_RACK for reads.
# This feature is in particular useful if the cluster is in the cloud and the cloud provider
# is charging for network bandwidth out of the zone. Keep in mind that the node on the same rack
# may not be the Master, and as such the data may be stale. This setting is particularly usable
# for clusters that are read heavy.
rack-aware = false

# RackIds defines the list of acceptable racks in order of preference. Nodes in RackIds[0] are chosen first.
# If a node is not found in rackIds[0], then nodes in rackIds[1] are searched, and so on.
# If rackIds is set, ClientPolicy.RackId is ignored.
#
# ClientPolicy.RackAware, ReplicaPolicy.PREFER_RACK and server rack
# configuration must also be set to enable this functionality.
rack-ids = []

# IgnoreOtherSubnetAliases helps to ignore aliases that are outside main subnet
ignore-other-subnet-aliases = false

# SeedOnlyCluster enforces the client to use only the seed addresses.
# Peers nodes for the cluster are not discovered and seed nodes are
# retained despite connection failures.
seed-only-cluster = false

[cluster_tls]
port = 4333
host = "3.3.3.3"
tls-name = "tls-name"
tls-enable = true
tls-capath = "{{.RootCAPath}}"
tls-cafile = "{{.RootCAFile}}"
tls-certfile = "{{.CertFile}}"
tls-keyfile = "{{.KeyFile}}"

[cluster_instance]
host = "3.3.3.3:3003,4.4.4.4:3004"
user = "test-user"
password = "test-password"

[cluster_env]
host = "5.5.5.5:env-tls-name:1000"
password = "env:AEROSPIKE_TEST"

[cluster_envb64]
host = "6.6.6.6:env-tls-name:1000"
password = "env-b64:AEROSPIKE_TEST"

[cluster_b64]
host = "7.7.7.7:env-tls-name:1000"
password = "b64:dGVzdC1wYXNzd29yZAo="

[cluster_file]
host = "1.1.1.1"
password = "file:{{.PassFile}}"

[uda]
agent-port = 8001
store-file = "default1.store"

[uda_instance]
store-file = "test.store"

~~~
