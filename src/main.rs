use crate::binary::*;
use crate::misc::*;
use crate::packets::*;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::ops::Add;
use std::ops::Sub;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use web::Credentials;
use web::LoginError;

pub mod binary;
pub mod misc;
pub mod packets;
pub mod web;

const PACKET_ATTEMPT_DELAY: Duration = Duration::from_millis(10);
const PING_TIMEOUT: Duration = Duration::from_millis(48000);
const PING_TIMEOUT_SLACK: Duration = Duration::from_millis(8000);

#[derive(Debug)]
enum InternalBanchoMessage {
    Ping, // Used to check if a channel is alive
    Stop,
    NewIrcMessage {
        author: String,
        message: String,
        channel: String,
    },
}

#[derive(Debug)]
enum InternalClientMessage {
    NewIrcMessage {
        author: String,
        message: String,
        channel: String,
    },
}

// TODO maybe categorize into Bancho and Osu?
#[derive(Debug, num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(u16)]
enum RequestType {
    /// Id: 0
    OsuSendUserStatus = 0,
    /// Id: 1
    OsuSendIrcMessage = 1,
    /// Id: 2
    OsuExit = 2,
    /// Id: 3
    OsuRequestStatusUpdate = 3,
    /// Id: 4
    OsuPong = 4,
    /// Id: 5
    BanchoLoginReply = 5,
    /// Id: 11
    BanchoHandleOsuUpdate = 11,
    /// Id: 7
    BanchoSendMessage = 7,
    /// Id: 8
    BanchoPing = 8,
    /// Id: 24
    BanchoAnnounce = 24,
    /// Id: 64
    BanchoChannelJoinSuccess = 64,
    /// Id: 68
    OsuBeatmapInfoRequest = 68,
    /// Id: 79
    OsuReceiveUpdates = 79,
    /// Id: 83
    BanchoUserPresence = 83,
    /// Id: 85
    OsuUserStatsRequest = 85,
    /// Id: 86
    BanchoRestart = 86,
}

#[derive(Debug)]
struct ClientData {
    username: String,
    user_id: i32,
}

impl ClientData {
    pub async fn login(
        credentials: Credentials,
        stream: &mut tokio::net::TcpStream,
        web_client: web::Client,
    ) -> Result<ClientData, LoginError> {
        let user_id = web_client.login(&credentials).await?;
        let client_data = ClientData {
            user_id,
            username: credentials.username,
        };
        let user_response = web_client.get_user(client_data.user_id).await;
        send_user_presence(web_client.clone(), stream, user_response.id).await;
        send_stats_update_with_response(web_client.clone(), &user_response, stream).await;
        Ok(client_data)
    }
}

async fn send_user_presence(
    web_client: web::Client,
    stream: &mut tokio::net::TcpStream,
    user_id: i32,
) {
    let rank = web_client.get_rank(user_id).await;
    let username = web_client.get_username(user_id).await;
    println!("Sending user presence for {}. Rank: {}", username, rank);
    stream
        .write_object(UserPresence {
            user_id,
            rank,
            username,
            timezone: 0,
            country_code: 0,
            permissions_b: 0,
            longitude: 0.0,
            latitude: 0.0,
        })
        .await;
}

async fn send_stats_update_with_response(
    web_client: web::Client,
    user_response: &web::User,
    stream: &mut tokio::net::TcpStream,
) {
    let rank = web_client.get_rank(user_response.id).await;
    println!("Sending stats {:?}. Rank: {}", user_response, rank);
    stream
        .write_object(OsuUpdate {
            user_id: user_response.id,
            status_update: StatusUpdate {
                status: 0,
                status_text: "I am gaming".to_owned(),
                beatmap_checksum: "-1".to_owned(),
                current_mods: 0,
                play_mode: 0,
                beatmap_id: 0,
            },
            ranked_score: user_response.ranked_score,
            accuracy: user_response.accuracy,
            play_count: user_response.play_count,
            total_score: user_response.total_score,
            rank,
            performance_points: user_response.performance_points as i16,
        })
        .await;
}

pub async fn send_stats_update(
    web_client: web::Client,
    stream: &mut tokio::net::TcpStream,
    user_id: i32,
) {
    let user_response = web_client.get_user(user_id).await;
    send_stats_update_with_response(web_client, &user_response, stream).await;
}

async fn initialize_client(stream: &mut tokio::net::TcpStream, client: web::Client) -> ClientData {
    let mut reader = BufReader::new(&mut *stream);

    let mut username = String::new();
    reader.read_line(&mut username).await.unwrap();
    let username = username.trim();

    let mut password_hash = String::new();
    reader.read_line(&mut password_hash).await.unwrap();
    let password_hash = password_hash.trim();

    let mut client_data = String::new();
    reader.read_line(&mut client_data).await.unwrap();
    let client_data = client_data.trim();

    println!("Username: {}", username);
    println!("Password Hash: {}", password_hash);
    println!("Client Data: {}", client_data);

    let login_result = ClientData::login(
        Credentials {
            username: username.to_string(),
            password_hash: password_hash.to_string(),
        },
        stream,
        client.clone(),
    )
    .await;

    let login_reply = LoginReply {
        user_id: login_result.as_ref().map(|c| c.user_id).unwrap_or(-1),
    };
    println!("Login Reply: {:?}", login_reply);
    stream.write_object(login_reply).await;

    if let Ok(client_data) = login_result {
        stream
            .write_object(ChannelJoinSuccess {
                channel_name: "#osu".to_owned(),
            })
            .await;

        send_user_presence(client.clone(), stream, 2).await;
        stream
            .write_object(SendMessage {
                sending_client: "GamerDuck".to_owned(),
                content: "Hello I am gamer".to_owned(),
                channel: "#osu".to_owned(),
            })
            .await;

        stream.flush().await.unwrap();
        client_data
    } else {
        stream.flush().await.unwrap();
        panic!("Unable to login, {:?}", login_result.unwrap_err());
    }
}

async fn handle_client(
    mut stream: tokio::net::TcpStream,
    client: web::Client,
    msg_rx: std::sync::mpsc::Receiver<InternalBanchoMessage>,
    bancho_tx: tokio::sync::mpsc::Sender<InternalClientMessage>,
) {
    println!("New Client! {:?}", stream.peer_addr());

    let client_data = initialize_client(&mut stream, client.clone()).await;

    let mut last_ping = Instant::now();
    let mut last_pong = Instant::now();

    'outer: loop {
        let read_type: RequestType = match {
            let mut buf = [0; 2];
            loop {
                let res = nonblock_unwrap(stream.try_read(&mut buf)).unwrap();
                if let Some(n) = res {
                    if n == buf.len() {
                        break;
                    } else {
                        println!("Client connection closed");
                        break 'outer;
                    }
                }
                if time_diff(last_ping, last_pong) > PING_TIMEOUT.add(PING_TIMEOUT_SLACK) {
                    println!("Client didn't respond to ping, assumed dead");
                    break 'outer;
                }
                if last_ping.elapsed() > PING_TIMEOUT.sub(PING_TIMEOUT_SLACK) {
                    println!("Ping!");
                    stream.write_object(Ping {}).await;
                    last_ping = Instant::now();
                }
                let res = msg_rx.try_recv();
                match res {
                    Ok(msg) => match msg {
                        InternalBanchoMessage::Ping => {}
                        InternalBanchoMessage::Stop => {
                            println!("Received a stop message");
                            stream.write_object(NotifyRestart { retry_ms: 8000 }).await;
                            break 'outer;
                        }
                        InternalBanchoMessage::NewIrcMessage {
                            author,
                            message,
                            channel,
                        } => {
                            if author != client_data.username {
                                println!(
                                    "Sending message '{}' in {} by {} to {}",
                                    message, channel, author, client_data.username
                                );
                                stream
                                    .write_object(SendMessage {
                                        sending_client: author,
                                        content: message,
                                        channel,
                                    })
                                    .await;
                            }
                        }
                    },
                    Err(e) => match e {
                        std::sync::mpsc::TryRecvError::Empty => {}
                        std::sync::mpsc::TryRecvError::Disconnected => {
                            panic!("Internal Message Channel was disconnected!")
                        }
                    },
                }
                tokio::time::sleep(PACKET_ATTEMPT_DELAY).await;
            }
            RequestType::try_from(u16::from_le_bytes(buf))
        } {
            Ok(o) => o,
            Err(e) => panic!("{}", e),
        };

        let _unused_byte = stream.read_u8().await.unwrap();
        let packet_length = stream.read_u32_le().await.unwrap();
        println!("{}: {:?}", client_data.username, read_type);

        use RequestType::*;
        match read_type {
            OsuSendIrcMessage => {
                let _sending_client = stream.read_length_string().await;
                let content = stream.read_length_string().await;
                let channel = stream.read_length_string().await;
                bancho_tx
                    .send(InternalClientMessage::NewIrcMessage {
                        author: client_data.username.clone(),
                        message: content,
                        channel,
                    })
                    .await
                    .unwrap();
            }
            OsuExit => {
                println!("Client Exited!");
                break 'outer;
            }
            OsuRequestStatusUpdate => {
                send_stats_update(client.clone(), &mut stream, client_data.user_id).await;
            }
            OsuPong => {
                println!("Pong!");
                last_pong = Instant::now();
            }
            OsuBeatmapInfoRequest => {}
            OsuReceiveUpdates => {
                let mode = stream.read_i32_le().await.unwrap();
                println!("New Update Mode {}! TODO", mode);
            }
            OsuUserStatsRequest => {
                let ids = stream.read_length_i32_array().await;
                println!("Request for users: {:?}", ids);
                for id in ids {
                    println!("Sending {}", id);
                    send_stats_update(client.clone(), &mut stream, id).await;
                }
            }
            n => {
                println!("Unknown Packet Type {:?}", n);
                // Reads the whole packet and discards it to not mess up future reads
                stream
                    .read_exact(&mut vec![0; packet_length as usize])
                    .await
                    .unwrap();
            }
        }
    }

    println!("Goodbye!");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    // client
    let listener = tokio::net::TcpListener::bind("0.0.0.0:13382").await?;
    // web
    let web_listener = tokio::net::TcpListener::bind("0.0.0.0:13383").await?;
    println!("Running");
    let client = web::Client::new(reqwest::Client::builder().build()?);
    let msg_txs: Arc<RwLock<Vec<std::sync::mpsc::SyncSender<InternalBanchoMessage>>>> =
        Arc::new(RwLock::new(Vec::new()));

    let (bancho_tx, mut bancho_rx) = tokio::sync::mpsc::channel::<InternalClientMessage>(1);
    ctrlc::set_handler({
        let msg_txs = msg_txs.clone();
        move || {
            println!("Sending stop message, please wait..");
            let msg_txs = msg_txs.read().unwrap();
            for msg_tx in msg_txs.iter() {
                let _ = msg_tx.send(InternalBanchoMessage::Stop);

                const MAX_ATTEMPT: u32 = 100;
                let mut attempt = 0;
                // Wait for disconnect
                while msg_tx.send(InternalBanchoMessage::Ping).is_ok() && attempt < MAX_ATTEMPT {
                    std::thread::sleep(Duration::from_millis(50));
                    attempt += 1;
                    if attempt % 10 == 0 && attempt != 0 {
                        println!("Attempt {}", attempt);
                    }
                }
                if attempt >= MAX_ATTEMPT {
                    println!("Client thread refused to stop");
                }
            }
            println!("Sent stop message, exiting..");
            std::process::exit(0);
        }
    })
    .unwrap();
    loop {
        tokio::select! {
            res = listener.accept() => {
                let (stream, _addr) = res?;

                let (msg_tx, msg_rx) = std::sync::mpsc::sync_channel(0);
                let mut msg_txs = msg_txs.write().unwrap();
                msg_txs.push(msg_tx);

                tokio::spawn(handle_client(stream, client.clone(), msg_rx, bancho_tx.clone()));
            }
            res = web_listener.accept() => {
                let (mut stream, _addr) = res?;
                let request_type = stream.read_u16_le().await.unwrap();
                match request_type {
                    // Get user count
                    1 => {
                        let msg_txs = msg_txs.read().unwrap();
                        stream.write_u16_le(msg_txs.len() as u16).await.unwrap();
                    }
                    _ => panic!("Unknown request type from web")
                }
            }
            res = bancho_rx.recv() => {
                let res = res.unwrap();
                match res {
                    InternalClientMessage::NewIrcMessage { author, message, channel } => {
                        let msg_txs = msg_txs.read().unwrap();
                        for msg_tx in msg_txs.iter() {
                            msg_tx.send(InternalBanchoMessage::NewIrcMessage {
                                author: author.clone(),
                                message: message.clone(),
                                channel: channel.clone()
                            }).unwrap();
                        }
                    },
                }
            }
        };
    }
}
