use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use rand::Rng;
use sha2::{Digest, Sha256};
use chrono::Utc;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Peer {
    id: u64,
    address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Message {
    NewPeer(Peer),
    NewBlock(Block),
    KeepAlive,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Block {
    index: u64,
    previous_hash: String,
    timestamp: u64,
    data: String,
    hash: String,
    writer: Peer,
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

    fn get_last_block(&self) -> Option<&Block> {
        self.blocks.last()
    }
}

type Peers = Arc<RwLock<HashMap<u64, Peer>>>;

#[tokio::main]
async fn main() {
    let blockchain = Arc::new(RwLock::new(Blockchain::new()));
    let peers: Peers = Arc::new(RwLock::new(HashMap::new()));

    let ports = [8080, 8081];
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

    let peer_id = rand::thread_rng().gen::<u64>();
    let local_peer = Peer { id: peer_id, address: local_addr };

    // Start logging peers list
    let peers_clone = peers.clone();
    tokio::spawn(async move {
        log_peers(peers_clone).await;
    });

    // Connect to the other known peers
    let peer_addresses = vec![
        "127.0.0.1:8080".parse().unwrap(),
        "127.0.0.1:8081".parse().unwrap(),
        //"127.0.0.1:8082".parse().unwrap(),
    ];

    for addr in peer_addresses {
        if local_addr != addr {
            let peers_clone = peers.clone();
            let local_peer_clone = local_peer.clone();
            tokio::spawn(async move {
                connect_to_peer(addr, local_peer_clone, peers_clone).await;
            });
        }
    }

    // Create genesis block if this is peer 1
    if local_addr.port() == 8080 {
        let blockchain_clone = blockchain.clone();
        let peers_clone = peers.clone();
        let local_peer_clone = local_peer.clone();
        tokio::spawn(async move {
            println!("Creating genesis block in 10 secs");
            sleep(Duration::from_secs(10)).await;
            create_and_broadcast_new_block(blockchain_clone, peers_clone, local_peer_clone, true).await;
        });
    }

    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        println!("Accepted connection from {}", addr);
        let peers_clone = peers.clone();
        let local_peer_clone = local_peer.clone();
        tokio::spawn(async move {
            handle_connection(socket, addr, peers_clone, local_peer_clone).await;
        });
    }
}

async fn connect_to_peer(addr: SocketAddr, local_peer: Peer, peers: Peers) {
    loop {
        match TcpStream::connect(addr).await {
            Ok(stream) => {
                println!("Connected to peer: {}", addr);
                handle_outgoing_connection(stream, local_peer.clone(), peers.clone()).await;
            }
            Err(e) => {
                println!("Failed to connect to peer {}: {}. Retrying in 5 seconds...", addr, e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn handle_outgoing_connection(mut stream: TcpStream, local_peer: Peer, peers: Peers) {
    let message = serde_json::to_string(&Message::NewPeer(local_peer.clone())).unwrap();

    if stream.write_all(message.as_bytes()).await.is_ok() {
        println!("Sent NewPeer message to {}", stream.peer_addr().unwrap());
    } else {
        println!("Failed to send NewPeer message");
        return;
    }

    handle_peer_communication(stream, local_peer, peers).await;
}

async fn handle_connection(mut socket: TcpStream, addr: SocketAddr, peers: Peers, local_peer: Peer) {
    let message = serde_json::to_string(&Message::NewPeer(local_peer.clone())).unwrap();
    if let Err(e) = socket.write_all(message.as_bytes()).await {
        println!("Failed to send NewPeer message to {}: {}", addr, e);
        return;
    }
    println!("Sent NewPeer message to {}", addr);

    handle_peer_communication(socket, local_peer, peers).await;
}

async fn handle_peer_communication(mut stream: TcpStream, local_peer: Peer, peers: Peers) {
    let mut buffer = vec![0; 4096];

    loop {
        match tokio::time::timeout(Duration::from_secs(300), stream.read(&mut buffer)).await {
            Ok(Ok(0)) => {
                println!("Connection closed by {}", local_peer.address);
                break;
            }
            Ok(Ok(n)) => {
                let received_message: Message = match serde_json::from_slice(&buffer[..n]) {
                    Ok(msg) => msg,
                    Err(e) => {
                        println!("Failed to deserialize message: {}", e);
                        continue;
                    }
                };
                println!("Received message from {}: {:?}", local_peer.address, received_message);
                handle_message(received_message, &peers, &mut stream).await;
            }
            Ok(Err(e)) => {
                println!("Failed to read from socket: {}", e);
                break;
            }
            Err(e) => {
                println!("Socket error: {}", e);
                break;
            }
        }
    }

    {
        let mut peers_write = peers.write().await;
        peers_write.remove(&local_peer.id);
    }
}

async fn handle_message(message: Message, peers: &Peers, stream: &mut TcpStream) {
    println!("New MSG");
    match message {
        Message::NewPeer(new_peer) => {
            println!("New peer connected: {:?}", new_peer);
            let mut peers_write = peers.write().await;
            peers_write.insert(new_peer.id, new_peer.clone());
            println!("Added new peer to peers list: {:?}", new_peer);
        }
        Message::NewBlock(block) => {
            println!("New block received: {:?}", block);
            /*
            let peers_read = peers.read().await;
            let message = serde_json::to_string(&Message::NewBlock(block)).unwrap();
            for (_, peer) in peers_read.iter() {
                if peer.address != stream.peer_addr().unwrap() {
                    if let Ok(mut other_stream) = TcpStream::connect(peer.address).await {
                        if let Err(e) = other_stream.write_all(message.as_bytes()).await {
                            println!("Failed to send NewBlock message to {}: {}", peer.address, e);
                        }
                    }
                }
            }
            */
        }
        Message::KeepAlive => {
            println!("KeepAlive received");
        }
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

async fn create_and_broadcast_new_block(blockchain: Arc<RwLock<Blockchain>>, peers: Peers, current_writer: Peer, is_genesis: bool) {
    let data = if is_genesis { "Genesis Block".to_string() } else { "Some data".to_string() };

    let (index, previous_hash) = {
        let blockchain_read = blockchain.read().await;
        if is_genesis {
            (0, String::from("0"))
        } else {
            let previous_block = blockchain_read.get_last_block().unwrap();
            (previous_block.index + 1, previous_block.hash.clone())
        }
    };

    let timestamp = Utc::now().timestamp() as u64;
    let hash = calculate_hash(index, &previous_hash, timestamp, &data);

    let next_writer = current_writer.clone();

    let new_block = Block::new(index, previous_hash, timestamp, data, hash, next_writer.clone());
    {
        let mut blockchain_write = blockchain.write().await;
        blockchain_write.add_block(new_block.clone());
    }

    let message = serde_json::to_string(&Message::NewBlock(new_block.clone())).unwrap();

    println!("Broadcasting new block: {:?}", message);
    let peers_read = peers.read().await;
    println!("Broadcasting to peers: {:?}", peers_read);
    for (_, peer) in peers_read.iter() {
        if let Ok(mut stream) = TcpStream::connect(peer.address).await {
            println!("Sending to peer: {:?}", peer.address);
            if let Ok(m) = stream.write_all(message.as_bytes()).await {
                println!("Sent NewBlock message: {:?}", message);
            }
        }
    }
}

async fn log_peers(peers: Peers) {
    loop {
        sleep(Duration::from_secs(3)).await;
        let peers_read = peers.read().await;
        println!("Peers list: {:?}", *peers_read);
    }
}
