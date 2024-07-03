use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, Duration};
use rand::{Rng, thread_rng};
use rand::prelude::SliceRandom;
use sha2::{Sha256, Digest};
use chrono::Utc;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Peer {
    id: u64,
    address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Message {
    NewPeer(Peer),
    NewBlock(Block),
    RequestBlock,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Block {
    index: u64,
    previous_hash: String,
    timestamp: u64,
    data: String,
    hash: String,
    writer: Peer,  // The writer of the next block
}

impl Block {
    fn new(index: u64, previous_hash: String, timestamp: u64, data: String, hash: String, writer: Peer) -> Self {
        Block {
            index,
            previous_hash,
            timestamp,
            data,
            hash,
            writer,
        }
    }
}

#[derive(Debug, Clone)]
struct Blockchain {
    blocks: Vec<Block>,
}

impl Blockchain {
    fn new() -> Self {
        Blockchain { blocks: Vec::new() }
    }

    fn add_block(&mut self, block: Block) {
        self.blocks.push(block);
    }

    fn create_genesis_block(&mut self, writer: Peer) {
        let genesis_block = Block::new(
            0,
            String::from("0"),
            Utc::now().timestamp() as u64,
            String::from("Genesis Block"),
            calculate_hash(0, "0", Utc::now().timestamp() as u64, "Genesis Block"),
            writer,
        );
        self.add_block(genesis_block);
    }
}

#[tokio::main]
async fn main() {
    let mut blockchain = Blockchain::new();
    let mut peers: Vec<Peer> = Vec::new();

    let ports = [8080, 8081, 8082];
    let mut listener = None;

    for port in &ports {
        match TcpListener::bind(("127.0.0.1", *port)).await {
            Ok(l) => {
                listener = Some(l);
                println!("Server running on 127.0.0.1:{}", port);
                break;
            },
            Err(e) => {
                eprintln!("Failed to bind to port {}: {}", port, e);
            }
        }
    }

    let listener = listener.expect("Failed to bind to any port");
    let local_addr = listener.local_addr().unwrap();

    // Create genesis block if this is peer 1
    if local_addr.port() == 8080 {
        sleep(Duration::from_secs(10)).await;
        let genesis_writer = Peer {
            id: 1,
            address: local_addr,
        };
        blockchain.create_genesis_block(genesis_writer.clone());
        println!("Genesis block created by peer 1");
        create_and_broadcast_new_block(&mut blockchain, &mut peers, genesis_writer).await;
    }

    // Connect to the other known peers
    let peer_addresses = vec![
        "127.0.0.1:8080".parse().unwrap(),
        "127.0.0.1:8081".parse().unwrap(),
        "127.0.0.1:8082".parse().unwrap(),
    ];

    for addr in &peer_addresses {
        if local_addr != *addr {
            match TcpStream::connect(addr).await {
                Ok(stream) => {
                    handle_outgoing_connection(stream, local_addr, &mut peers).await;
                }
                Err(e) => {
                    println!("Failed to connect to peer {}: {}", addr, e);
                }
            }
        }
    }

    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        println!("Accepted connection from {}", addr);
        let mut blockchain_clone = blockchain.clone();
        let mut peers_clone = peers.clone();

        tokio::spawn(async move {
            handle_connection(socket, addr, &mut blockchain_clone, &mut peers_clone).await;
        });
    }
}

async fn handle_outgoing_connection(mut stream: TcpStream, local_addr: SocketAddr, peers: &mut Vec<Peer>) {
    let peer_id = rand::thread_rng().gen::<u64>();
    let new_peer = Peer { id: peer_id, address: local_addr };
    let message = serde_json::to_string(&Message::NewPeer(new_peer)).unwrap();
    if stream.write_all(message.as_bytes()).await.is_ok() {
        println!("Sent NewPeer message to {}", stream.peer_addr().unwrap());
    } else {
        println!("Failed to send NewPeer message");
        return;
    }

    let mut buffer = [0; 1024];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                println!("Connection closed by {}", stream.peer_addr().unwrap());
                return;
            }
            Ok(n) => {
                let received_message: Message = match serde_json::from_slice(&buffer[..n]) {
                    Ok(msg) => msg,
                    Err(e) => {
                        println!("Failed to deserialize message: {}", e);
                        continue;
                    },
                };
                println!("Received message: {:?}", received_message);
                match received_message {
                    Message::NewPeer(new_peer) => {
                        if !peers.iter().any(|p| p.address == new_peer.address) {
                            peers.push(new_peer.clone());
                            println!("Added new peer: {:?}", new_peer);
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                println!("Failed to read from socket: {}", e);
                return;
            }
        }
    }
}

async fn handle_connection(mut socket: TcpStream, addr: SocketAddr, blockchain: &mut Blockchain, peers: &mut Vec<Peer>) {
    let peer_id = rand::thread_rng().gen::<u64>();
    let new_peer = Peer { id: peer_id, address: addr };

    if !peers.iter().any(|p| p.address == new_peer.address) {
        peers.push(new_peer.clone());
        println!("Added new peer: {:?}", new_peer);
    }

    let message = serde_json::to_string(&Message::NewPeer(new_peer)).unwrap();
    if let Err(e) = socket.write_all(message.as_bytes()).await {
        println!("Failed to send NewPeer message to {}: {}", addr, e);
        return;
    }
    println!("Sent NewPeer message to {}", addr);

    let mut buffer = [0; 1024];
    loop {
        match socket.read(&mut buffer).await {
            Ok(0) => {
                println!("Connection closed by {}", addr);
                return;
            }
            Ok(n) => {
                let received_message: Message = match serde_json::from_slice(&buffer[..n]) {
                    Ok(msg) => msg,
                    Err(e) => {
                        println!("Failed to deserialize message: {}", e);
                        continue;
                    },
                };
                println!("Received message from {}: {:?}", addr, received_message);
                match received_message {
                    Message::NewPeer(new_peer) => {
                        if !peers.iter().any(|p| p.address == new_peer.address) {
                            peers.push(new_peer.clone());
                            println!("Added new peer: {:?}", new_peer);
                        }
                    }
                    Message::NewBlock(block) => {
                        blockchain.add_block(block.clone());
                        println!("New block added: {:?}", blockchain.blocks.last().unwrap());
                        
                        // Check if this peer is the writer for the next block
                        if block.writer.address == addr {
                            println!("This peer is the writer. Creating a new block in 10 seconds...");
                            let mut blockchain_clone = blockchain.clone();
                            let mut peers_clone = peers.clone();
                            let next_writer = block.writer.clone();
                            tokio::spawn(async move {
                                sleep(Duration::from_secs(10)).await;
                                create_and_broadcast_new_block(&mut blockchain_clone, &mut peers_clone, next_writer).await;
                            });
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                println!("Failed to read from socket: {}", e);
                return;
            }
        };
    }
}

fn calculate_hash(index: u64, previous_hash: &str, timestamp: u64, data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(index.to_string());
    hasher.update(previous_hash);
    hasher.update(timestamp.to_string());
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

async fn create_and_broadcast_new_block(blockchain: &mut Blockchain, peers: &mut Vec<Peer>, current_writer: Peer) {
    let data = "Some data".to_string();
    let previous_block = blockchain.blocks.last().unwrap();
    let index = previous_block.index + 1;
    let previous_hash = &previous_block.hash;
    let timestamp = Utc::now().timestamp() as u64;
    let hash = calculate_hash(index, previous_hash, timestamp, &data);

    let next_writer = peers.choose(&mut thread_rng()).unwrap_or(&current_writer).clone();

    let new_block = Block::new(index, previous_hash.clone(), timestamp, data, hash, next_writer.clone());
    blockchain.add_block(new_block.clone());

    let message = serde_json::to_string(&Message::NewBlock(new_block.clone())).unwrap();

    println!("Broadcasting new block: {:?}", new_block);
    for peer in peers.iter() {
        match TcpStream::connect(peer.address).await {
            Ok(mut socket) => {
                if let Err(e) = socket.write_all(message.as_bytes()).await {
                    println!("Failed to send NewBlock message to {}: {}", peer.address, e);
                } else {
                    println!("Sent NewBlock message to {}", peer.address);
                }
            }
            Err(e) => {
                println!("Failed to connect to peer {}: {}", peer.address, e);
            }
        }
    }
}
