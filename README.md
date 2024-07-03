
# SRCC - Simple Rust Consensus Client

This project implements a simple blockchain network using Rust and the Tokio asynchronous runtime. The blockchain nodes communicate over TCP to create a p2p network and broadcast new blocks to a shared blockchain which is used to chronologically store gossip data.

## Features

- Asynchronous TCP server and client using Tokio.
- Peer-to-peer communication.
- Simple blockchain implementation.
- Genesis block creation and broadcasting.
- Handling of incoming connections and messages.

## Requirements

- Rust (latest stable version)
- Cargo (latest stable version)

## Dependencies

The project relies on the following Rust crates:

- `serde`: Serialization and deserialization of data structures.
- `tokio`: Asynchronous runtime for handling TCP connections.
- `rand`: Random number generation.
- `sha2`: SHA-256 hashing.
- `chrono`: Date and time handling.

## Installation

1. Clone the repository:

```sh
git clone https://github.com/yourusername/simple-rust-blockchain.git
cd simple-rust-blockchain
```

2. Build the project:

```sh
cargo build --release
```

3. Run the project:

```sh
cargo run
```

## Code Overview

### Main Components

1. **Peer**: Represents a node in the network.
2. **Message**: Defines the types of messages exchanged between peers.
3. **Block**: Represents a block in the blockchain.
4. **Blockchain**: Manages the list of blocks.

### Main Functions

1. **main**: Sets up the server, listens for incoming connections, and manages the genesis block creation.
2. **handle_connection**: Handles incoming connections, reads messages, and updates the peer list.
3. **handle_message**: Processes received messages and broadcasts new blocks.
4. **create_and_broadcast_new_block**: Creates a new block and broadcasts it to all connected peers.

### Usage

1. **Running the Node**: The node listens on port 8080. When the node starts, it attempts to connect to other nodes running on the same machine on ports 8080, 8081, and 8082.

2. **Genesis Block Creation**: The first node (running on port 8080) will create a genesis block after 10 seconds and broadcast it to other connected peers.

3. **Adding New Blocks**: When a node receives a new block, it adds it to its local blockchain and broadcasts it to other peers.

## Example Output

When you run the node, you should see output similar to this:

```sh
Server running on 127.0.0.1:8080
Accepted connection from 127.0.0.1:56518
Sent NewPeer message to 127.0.0.1:56518
Received message from 127.0.0.1:56518: NewPeer(Peer { id: 1234567890, address: 127.0.0.1:56518 })
Creating genesis block in 10 secs
Broadcasting new block: Block { index: 0, previous_hash: "0", timestamp: 1234567890, data: "Genesis Block", hash: "abc123...", writer: Peer { id: 1, address: 127.0.0.1:8080 } }
```

## Contributing

If you have suggestions for improving this project, please open an issue or submit a pull request. Contributions are welcome!

## Links

https://jamesbachini.com
https://www.youtube.com/c/JamesBachini
https://bachini.substack.com
https://podcasters.spotify.com/pod/show/jamesbachini
https://twitter.com/james_bachini
https://www.linkedin.com/in/james-bachini/
https://github.com/jamesbachini

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.
