use crate::binary::*;
use crate::misc::*;
use crate::packets::*;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::ops::Add;
use std::ops::Sub;
use std::time::Duration;
use std::time::Instant;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;

pub mod binary;
pub mod misc;
pub mod packets;
pub mod web;

const PACKET_ATTEMPT_DELAY: Duration = Duration::from_millis(10);
const PING_TIMEOUT: Duration = Duration::from_millis(48000);
const PING_TIMEOUT_SLACK: Duration = Duration::from_millis(8000);

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
}

struct ClientData {
    user_id: i32,
}

#[allow(dead_code)]
struct Credentials {
    username: String,
    password_hash: String,
}

impl ClientData {
    pub async fn login(
        _credentials: Credentials,
        stream: &mut tokio::net::TcpStream,
        web_client: web::Client,
    ) -> ClientData {
        let client_data = ClientData { user_id: 1 };
        let user_response = web_client.get_user(client_data.user_id).await;
        send_user_presence(web_client.clone(), stream, user_response.id).await;
        send_stats_update_with_response(web_client.clone(), &user_response, stream).await;
        client_data
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

    let user_id = 1;

    stream.write_object(LoginReply { user_id }).await;
    stream
        .write_object(ChannelJoinSuccess {
            channel_name: "#osu".to_owned(),
        })
        .await;
    let client_data = ClientData::login(
        Credentials {
            username: username.to_string(),
            password_hash: password_hash.to_string(),
        },
        stream,
        client.clone(),
    )
    .await;
    send_user_presence(client.clone(), stream, 2).await;
    stream
        .write_object(SendMessage {
            sending_client: "GamerDuck".to_owned(),
            content: "Hello I am gamer".to_owned(),
            channel: "#osu".to_owned(),
        })
        .await;

    {
        use tokio::io::AsyncWriteExt;
        stream.flush().await.unwrap();
    }

    client_data
}

async fn handle_client(mut stream: tokio::net::TcpStream, client: web::Client) {
    println!("New Client! {:?}", stream.peer_addr());

    let client_data = initialize_client(&mut stream, client.clone()).await;

    let mut last_ping = Instant::now();
    let mut last_pong = Instant::now();

    'outer: loop {
        let read_type: RequestType = match {
            let mut buf = [0; 2];
            loop {
                let res = nonblock_unwrap(stream.try_read(&mut buf)).unwrap();
                if res.is_none() {}
                if let Some(n) = res {
                    if n == buf.len() {
                        break;
                    } else {
                        println!("Client connection closed");
                        break 'outer;
                    }
                } else {
                    if time_diff(last_ping, last_pong) > PING_TIMEOUT.add(PING_TIMEOUT_SLACK) {
                        println!("Client didn't respond to ping, assumed dead");
                        break 'outer;
                    }
                    if last_ping.elapsed() > PING_TIMEOUT.sub(PING_TIMEOUT_SLACK) {
                        println!("Ping!");
                        stream.write_object(Ping {}).await;
                        last_ping = Instant::now();
                    }
                    tokio::time::sleep(PACKET_ATTEMPT_DELAY).await;
                }
            }
            RequestType::try_from(u16::from_le_bytes(buf))
        } {
            Ok(o) => o,
            Err(e) => panic!("{}", e),
        };

        let _unused_byte = stream.read_u8().await.unwrap();
        let packet_length = stream.read_u32_le().await.unwrap();
        println!("Received a {:?} packet", read_type);

        use RequestType::*;
        match read_type {
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:13382").await?;
    let client = web::Client::new(reqwest::Client::builder().build()?);
    loop {
        let (stream, _addr) = listener.accept().await?;
        tokio::spawn(handle_client(stream, client.clone()));
    }
}
