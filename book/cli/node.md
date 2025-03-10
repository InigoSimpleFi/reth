# `reth node`

Start the node

```bash
$ reth node --help

Usage: reth node [OPTIONS]

Options:
      --datadir <DATA_DIR>
          The path to the data dir for all reth files and subdirectories.
          
          Defaults to the OS-specific data directory:
          
          - Linux: `$XDG_DATA_HOME/reth/` or `$HOME/.local/share/reth/`
          - Windows: `{FOLDERID_RoamingAppData}/reth/`
          - macOS: `$HOME/Library/Application Support/reth/`
          
          [default: default]

      --config <FILE>
          The path to the configuration file to use.

      --chain <CHAIN_OR_PATH>
          The chain this node is running.
          
          Possible values are either a built-in chain or the path to a chain specification file.
          
          Built-in chains:
          - mainnet
          - goerli
          - sepolia
          - holesky
          - dev
          
          [default: mainnet]

      --instance <INSTANCE>
          Add a new instance of a node.
          
          Configures the ports of the node to avoid conflicts with the defaults. This is useful for running multiple nodes on the same machine.
          
          Max number of instances is 200. It is chosen in a way so that it's not possible to have port numbers that conflict with each other.
          
          Changes to the following port numbers: - DISCOVERY_PORT: default + `instance` - 1 - AUTH_PORT: default + `instance` * 100 - 100 - HTTP_RPC_PORT: default - `instance` + 1 - WS_RPC_PORT: default + `instance` * 2 - 2
          
          [default: 1]

      --trusted-setup-file <PATH>
          Overrides the KZG trusted setup by reading from the supplied file

  -h, --help
          Print help (see a summary with '-h')

Metrics:
      --metrics <SOCKET>
          Enable Prometheus metrics.
          
          The metrics will be served at the given interface and port.

Networking:
  -d, --disable-discovery
          Disable the discovery service

      --disable-dns-discovery
          Disable the DNS discovery

      --disable-discv4-discovery
          Disable Discv4 discovery

      --discovery.port <DISCOVERY_PORT>
          The UDP port to use for P2P discovery/networking. default: 30303

      --trusted-peers <TRUSTED_PEERS>
          Target trusted peer enodes --trusted-peers enode://abcd@192.168.0.1:30303

      --trusted-only
          Connect only to trusted peers

      --bootnodes <BOOTNODES>
          Bootnodes to connect to initially.
          
          Will fall back to a network-specific default if not specified.

      --peers-file <FILE>
          The path to the known peers file. Connected peers are dumped to this file on nodes
          shutdown, and read on startup. Cannot be used with `--no-persist-peers`.

      --identity <IDENTITY>
          Custom node identity
          
          [default: reth/VERSION/PLATFORM]

      --p2p-secret-key <PATH>
          Secret key to use for this node.
          
          This will also deterministically set the peer ID. If not specified, it will be set in the data dir for the chain being used.

      --no-persist-peers
          Do not persist peers.

      --nat <NAT>
          NAT resolution method (any|none|upnp|publicip|extip:<IP>)
          
          [default: any]

      --port <PORT>
          Network listening port. default: 30303

      --max-outbound-peers <MAX_OUTBOUND_PEERS>
          Maximum number of outbound requests. default: 100

      --max-inbound-peers <MAX_INBOUND_PEERS>
          Maximum number of inbound requests. default: 30

RPC:
      --http
          Enable the HTTP-RPC server

      --http.addr <HTTP_ADDR>
          Http server address to listen on
          
          [default: 127.0.0.1]

      --http.port <HTTP_PORT>
          Http server port to listen on
          
          [default: 8545]

      --http.api <HTTP_API>
          Rpc Modules to be configured for the HTTP server
          
          [possible values: admin, debug, eth, net, trace, txpool, web3, rpc, reth, ots]

      --http.corsdomain <HTTP_CORSDOMAIN>
          Http Corsdomain to allow request from

      --ws
          Enable the WS-RPC server

      --ws.addr <WS_ADDR>
          Ws server address to listen on
          
          [default: 127.0.0.1]

      --ws.port <WS_PORT>
          Ws server port to listen on
          
          [default: 8546]

      --ws.origins <ws.origins>
          Origins from which to accept WebSocket requests

      --ws.api <WS_API>
          Rpc Modules to be configured for the WS server
          
          [possible values: admin, debug, eth, net, trace, txpool, web3, rpc, reth, ots]

      --ipcdisable
          Disable the IPC-RPC  server

      --ipcpath <IPCPATH>
          Filename for IPC socket/pipe within the datadir
          
          [default: /tmp/reth.ipc]

      --authrpc.addr <AUTH_ADDR>
          Auth server address to listen on
          
          [default: 127.0.0.1]

      --authrpc.port <AUTH_PORT>
          Auth server port to listen on
          
          [default: 8551]

      --authrpc.jwtsecret <PATH>
          Path to a JWT secret to use for authenticated RPC endpoints

      --rpc-max-request-size <RPC_MAX_REQUEST_SIZE>
          Set the maximum RPC request payload size for both HTTP and WS in megabytes
          
          [default: 15]

      --rpc-max-response-size <RPC_MAX_RESPONSE_SIZE>
          Set the maximum RPC response payload size for both HTTP and WS in megabytes
          
          [default: 115]
          [aliases: --rpc.returndata.limit]

      --rpc-max-subscriptions-per-connection <RPC_MAX_SUBSCRIPTIONS_PER_CONNECTION>
          Set the the maximum concurrent subscriptions per connection
          
          [default: 1024]

      --rpc-max-connections <COUNT>
          Maximum number of RPC server connections
          
          [default: 500]

      --rpc-max-tracing-requests <COUNT>
          Maximum number of concurrent tracing requests
          
          [default: 25]

      --rpc-max-logs-per-response <COUNT>
          Maximum number of logs that can be returned in a single response
          
          [default: 20000]

      --rpc-gas-cap <GAS_CAP>
          Maximum gas limit for `eth_call` and call tracing RPC methods
          
          [default: 50000000]

Gas Price Oracle:
      --gpo.blocks <BLOCKS>
          Number of recent blocks to check for gas price
          
          [default: 20]

      --gpo.ignoreprice <IGNORE_PRICE>
          Gas Price below which gpo will ignore transactions
          
          [default: 2]

      --gpo.maxprice <MAX_PRICE>
          Maximum transaction priority fee(or gasprice before London Fork) to be recommended by gpo
          
          [default: 500000000000]

      --gpo.percentile <PERCENTILE>
          The percentile of gas prices to use for the estimate
          
          [default: 60]

      --block-cache-len <BLOCK_CACHE_LEN>
          Maximum number of block cache entries
          
          [default: 5000]

      --receipt-cache-len <RECEIPT_CACHE_LEN>
          Maximum number of receipt cache entries
          
          [default: 2000]

      --env-cache-len <ENV_CACHE_LEN>
          Maximum number of env cache entries
          
          [default: 1000]

TxPool:
      --txpool.pending_max_count <PENDING_MAX_COUNT>
          Max number of transaction in the pending sub-pool
          
          [default: 10000]

      --txpool.pending_max_size <PENDING_MAX_SIZE>
          Max size of the pending sub-pool in megabytes
          
          [default: 20]

      --txpool.basefee_max_count <BASEFEE_MAX_COUNT>
          Max number of transaction in the basefee sub-pool
          
          [default: 10000]

      --txpool.basefee_max_size <BASEFEE_MAX_SIZE>
          Max size of the basefee sub-pool in megabytes
          
          [default: 20]

      --txpool.queued_max_count <QUEUED_MAX_COUNT>
          Max number of transaction in the queued sub-pool
          
          [default: 10000]

      --txpool.queued_max_size <QUEUED_MAX_SIZE>
          Max size of the queued sub-pool in megabytes
          
          [default: 20]

      --txpool.max_account_slots <MAX_ACCOUNT_SLOTS>
          Max number of executable transaction slots guaranteed per account
          
          [default: 16]

      --txpool.pricebump <PRICE_BUMP>
          Price bump (in %) for the transaction pool underpriced check
          
          [default: 10]

      --blobpool.pricebump <BLOB_TRANSACTION_PRICE_BUMP>
          Price bump percentage to replace an already existing blob transaction
          
          [default: 100]

Builder:
      --builder.extradata <EXTRADATA>
          Block extra data set by the payload builder
          
          [default: reth/VERSION/OS]

      --builder.gaslimit <GAS_LIMIT>
          Target gas ceiling for built blocks
          
          [default: 30000000]

      --builder.interval <SECONDS>
          The interval at which the job should build a new payload after the last (in seconds)
          
          [default: 1]

      --builder.deadline <SECONDS>
          The deadline for when the payload builder job should resolve
          
          [default: 12]

      --builder.max-tasks <MAX_PAYLOAD_TASKS>
          Maximum number of tasks to spawn for building a payload
          
          [default: 3]

Debug:
      --debug.continuous
          Prompt the downloader to download blocks one at a time.
          
          NOTE: This is for testing purposes only.

      --debug.terminate
          Flag indicating whether the node should be terminated after the pipeline sync

      --debug.tip <TIP>
          Set the chain tip manually for testing purposes.
          
          NOTE: This is a temporary flag

      --debug.max-block <MAX_BLOCK>
          Runs the sync only up to the specified block

      --debug.print-inspector
          Print opcode level traces directly to console during execution

      --debug.hook-block <HOOK_BLOCK>
          Hook on a specific block during execution

      --debug.hook-transaction <HOOK_TRANSACTION>
          Hook on a specific transaction during execution

      --debug.hook-all
          Hook on every transaction in a block

Database:
      --db.log-level <LOG_LEVEL>
          Database logging level. Levels higher than "notice" require a debug build

          Possible values:
          - fatal:   Enables logging for critical conditions, i.e. assertion failures
          - error:   Enables logging for error conditions
          - warn:    Enables logging for warning conditions
          - notice:  Enables logging for normal but significant condition
          - verbose: Enables logging for verbose informational
          - debug:   Enables logging for debug-level messages
          - trace:   Enables logging for trace debug-level messages
          - extra:   Enables logging for extra debug-level messages

Dev testnet:
      --dev
          Start the node in dev mode
          
          This mode uses a local proof-of-authority consensus engine with either fixed block times
          or automatically mined blocks.
          Disables network discovery and enables local http server.
          Prefunds 20 accounts derived by mnemonic "test test test test test test test test test test
          test junk" with 10 000 ETH each.

      --dev.block-max-transactions <BLOCK_MAX_TRANSACTIONS>
          How many transactions to mine per block

      --dev.block-time <BLOCK_TIME>
          Interval between blocks.
          
          Parses strings using [humantime::parse_duration]
          --dev.block_time 12s

Pruning:
      --full
          Run full node. Only the most recent 10064 block states are stored. This flag takes priority over pruning configuration in reth.toml

Logging:
      --log.directory <PATH>
          The path to put log files in
          
          [default: /reth/logs]

      --log.max-size <SIZE>
          The maximum size (in MB) of log files
          
          [default: 200]

      --log.max-files <COUNT>
          The maximum amount of log files that will be stored. If set to 0, background file logging is disabled
          
          [default: 5]

      --log.journald
          Log events to journald

      --log.filter <FILTER>
          The filter to use for logs written to the log file
          
          [default: error]

      --color <COLOR>
          Sets whether or not the formatter emits ANSI terminal escape codes for colors and other text formatting
          
          [default: always]

          Possible values:
          - always: Colors on
          - auto:   Colors on
          - never:  Colors off

Display:
  -v, --verbosity...
          Set the minimum log level.
          
          -v      Errors
          -vv     Warnings
          -vvv    Info
          -vvvv   Debug
          -vvvvv  Traces (warning: very verbose!)

  -q, --quiet
          Silence all log output
```
