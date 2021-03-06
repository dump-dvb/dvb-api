// This example explores how to properly close a connection.
//
use super::{ReducedTelegram, Stop, WebSocketTelegram};
use async_tungstenite::WebSocketStream;
use futures_util::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;
use tungstenite::Message;
use {
    async_tungstenite::{accept_async, tokio::TokioAdapter},
    futures::{executor::block_on, SinkExt, StreamExt},
    serde::{Deserialize, Serialize},
    std::{
        env,
        sync::{Arc, Mutex},
    },
    tokio::net::TcpListener,
};

#[derive(Serialize, Deserialize, Debug)]
struct Filter {
    #[serde(default)]
    regions: Vec<u32>,
    #[serde(default)]
    junctions: Vec<u32>,
    #[serde(default)]
    lines: Vec<u32>,
}

impl Filter {
    pub fn fits(&self, telegram: &ReducedTelegram) -> bool {
        (self.regions.is_empty() || self.regions.contains(&telegram.region_code))
            && (self.junctions.is_empty() || self.junctions.contains(&telegram.position_id))
            && (self.lines.is_empty() || self.lines.contains(&telegram.line))
    }
}

pub struct UserState {
    filter: Option<Filter>,
    dead: bool,
}

pub type ProtectedState = Arc<Mutex<UserState>>;

pub struct Socket {
    write_socket: SplitSink<WebSocketStream<TokioAdapter<TcpStream>>, Message>,
    read_socket: SplitStream<WebSocketStream<TokioAdapter<TcpStream>>>,
    state: ProtectedState,
}

pub async fn connection_loop(mut connections: ConnectionPool) {
    let default_websock_port = String::from("127.0.0.1:9001");
    let websocket_port = env::var("DEFAULT_WEBSOCKET_HOST").unwrap_or(default_websock_port);

    let server = TcpListener::bind(websocket_port).await.unwrap();

    while let Ok((tcp, addr)) = server.accept().await {
        println!("New Socket Connection {}!", addr);

        let ws = accept_async(TokioAdapter::new(tcp))
            .await
            .expect("ws handshake");

        // spliting the socket into read and write component
        let (writer, reader) = ws.split();

        let state: ProtectedState = Arc::new(Mutex::new(UserState {
            dead: false,
            filter: None,
        }));

        connections
            .push(Socket {
                write_socket: writer,
                read_socket: reader,
                state: state.clone(),
            })
            .await;
    }
}

impl Socket {
    pub async fn read(&mut self) -> bool {
        match self.read_socket.next().await.transpose() {
            Ok(Some(Message::Text(data))) => {
                println!("data: {:?}", &data);
                match serde_json::from_str(&data) {
                    Ok(parsed_struct) => {
                        println!("Updating Filter!");
                        self.state.lock().unwrap().filter = Some(parsed_struct);
                    }
                    _ => {}
                }
                false
            }
            _ => {
                self.state.lock().unwrap().dead = true;
                true
            }
        }
    }
    pub async fn write(&mut self, telegram: &ReducedTelegram, stop: &Stop) -> bool {
        {
            let state = self.state.lock().unwrap();
            if state.filter.is_some() && !state.filter.as_ref().unwrap().fits(telegram) {
                return false;
            }
        }

        let sock_tele = WebSocketTelegram {
            reduced: telegram.clone(),
            meta_data: stop.clone(),
        };

        let serialized = serde_json::to_string(&sock_tele).unwrap();

        match self.write_socket.send(Message::Text(serialized)).await {
            Ok(_) => false,
            Err(_) => {
                self.state.lock().unwrap().dead = true;
                true
            }
        }
    }
}

impl UserState {
    pub fn new() -> UserState {
        UserState {
            filter: None,
            dead: false,
        }
    }
}

#[derive(Clone)]
pub struct ConnectionPool {
    connections: Arc<Mutex<Vec<Socket>>>,
}

impl ConnectionPool {
    pub fn new() -> ConnectionPool {
        ConnectionPool {
            connections: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn clone(&mut self) -> ConnectionPool {
        ConnectionPool {
            connections: Arc::clone(&self.connections),
        }
    }

    pub async fn write_all(&self, extracted: &ReducedTelegram, stop_meta_information: &Stop) {
        let mut dead_sockets = Vec::new();

        let mut unlocked_self = self.connections.lock().unwrap();

        for (i, socket) in unlocked_self.iter_mut().enumerate() {
            match block_on(tokio::time::timeout(
                    tokio::time::Duration::from_secs(1),
                    socket.write(&extracted, &stop_meta_information))) {
                Ok(err) => {
                    if err {
                        dead_sockets.push(i);
                        continue;
                    }
                }
                Err(_) => {
                    println!("timeout write {}", i);
                    dead_sockets.push(i);
                }
            }
            /*println!("read {}", i);
            match block_on(tokio::time::timeout(
                    tokio::time::Duration::from_secs(1),
                    socket.read())) {
                Ok(err) => {
                    if err {
                        dead_sockets.push(i);
                    }
                },
                Err(_) => {
                    println!("timeout read {}", i);
                    dead_sockets.push(i);
                }
            }*/
        }

        // removing dead sockets
        let mut remove_count = 0;
        for index in dead_sockets.iter() {
            println!("Removing {}", index);
            unlocked_self.remove(index - remove_count);
            remove_count += 1;
        }
    }

    pub async fn push(&mut self, sock: Socket) {
        let mut unlocked_self = self.connections.lock().unwrap();
        unlocked_self.push(sock);
        println!("ConnectionPool size: {}", unlocked_self.len());
    }
}
