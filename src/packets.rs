use crate::{
    binary::{StreamExt, WriteToBinaryStream},
    misc::ByteCount,
    sum, RequestType,
};
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;

macro_rules! packet {
    ($n:expr, struct $name:ident { $($fname:ident : $ftype:ty),* }) => {
        #[derive(Debug, PartialEq)]
        pub struct $name {
            $(pub $fname : $ftype),*
        }

        impl $name {
            pub fn byte_count(&self) -> usize {
                sum!($(self.$fname.byte_count()),*)
            }

            pub fn header(&self) -> Header {
                Header {
                    read_type: $n,
                    length: self.byte_count() as u32
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Header {
    pub read_type: u16,
    pub length: u32,
}

#[async_trait]
impl WriteToBinaryStream for Header {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream) {
        stream.write_u16_le(self.read_type).await.unwrap();
        stream.write_u8(0).await.unwrap();
        stream.write_u32_le(self.length).await.unwrap();
    }
}

packet! {
    RequestType::BanchoLoginReply.into(),
    struct LoginReply {
        user_id: i32
    }
}

#[async_trait]
impl WriteToBinaryStream for LoginReply {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream) {
        stream.write_object(self.header()).await;
        stream.write_i32_le(self.user_id).await.unwrap();
    }
}

packet! {
    RequestType::BanchoChannelJoinSuccess.into(),
    struct ChannelJoinSuccess {
        channel_name: String
    }
}

#[async_trait]
impl WriteToBinaryStream for ChannelJoinSuccess {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream) {
        stream.write_object(self.header()).await;
        stream.write_length_string(&self.channel_name).await;
    }
}

#[derive(Debug, PartialEq)]
pub struct StatusUpdate {
    pub status: u8,
    pub status_text: String,
    pub beatmap_checksum: String,
    pub current_mods: i16,
    pub play_mode: u8,
    pub beatmap_id: i32,
}

impl ByteCount for StatusUpdate {
    fn byte_count(&self) -> usize {
        self.status.byte_count()
            + self.status_text.byte_count()
            + self.beatmap_checksum.byte_count()
            + self.current_mods.byte_count()
            + self.play_mode.byte_count()
            + self.beatmap_id.byte_count()
    }
}

packet! {
    RequestType::BanchoHandleOsuUpdate.into(),
    struct OsuUpdate {
        user_id: i32,
        status_update: StatusUpdate,
        ranked_score: i64,
        accuracy: f32,
        play_count: i32,
        total_score: i64,
        rank: i32,
        performance_points: i16
    }
}

#[async_trait]
impl WriteToBinaryStream for OsuUpdate {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream) {
        stream.write_object(self.header()).await;
        stream.write_i32_le(self.user_id).await.unwrap();
        stream.write_u8(self.status_update.status).await.unwrap();
        stream
            .write_length_string(&self.status_update.status_text)
            .await;
        stream
            .write_length_string(&self.status_update.beatmap_checksum)
            .await;
        stream
            .write_i16_le(self.status_update.current_mods)
            .await
            .unwrap();
        stream.write_u8(self.status_update.play_mode).await.unwrap();
        stream
            .write_i32_le(self.status_update.beatmap_id)
            .await
            .unwrap();
        stream.write_i64_le(self.ranked_score).await.unwrap();
        stream.write_f32_le(self.accuracy).await.unwrap();
        stream.write_i32_le(self.play_count).await.unwrap();
        stream.write_i64_le(self.total_score).await.unwrap();
        stream.write_i32_le(self.rank).await.unwrap();
        stream.write_i16_le(self.performance_points).await.unwrap();
    }
}

packet! {
    RequestType::BanchoSendMessage.into(),
    struct SendMessage {
        sending_client: String,
        content: String,
        channel: String
    }
}

#[async_trait]
impl WriteToBinaryStream for SendMessage {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream) {
        stream.write_object(self.header()).await;
        stream.write_length_string(&self.sending_client).await;
        stream.write_length_string(&self.content).await;
        stream.write_length_string(&self.channel).await;
    }
}

packet! {
    RequestType::BanchoPing.into(),
    struct Ping {}
}

#[async_trait]
impl WriteToBinaryStream for Ping {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream) {
        stream.write_object(self.header()).await;
    }
}

packet! {
    RequestType::BanchoUserPresence.into(),
    struct UserPresence {
        user_id: i32,
        username: String,
        timezone: u8,
        country_code: u8,
        permissions_b: u8,
        longitude: f32,
        latitude: f32,
        rank: i32
    }
}

#[async_trait]
impl WriteToBinaryStream for UserPresence {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream) {
        stream.write_object(self.header()).await;
        stream.write_i32_le(self.user_id).await.unwrap();
        stream.write_length_string(&self.username).await;
        stream.write_u8(self.timezone).await.unwrap();
        stream.write_u8(self.country_code).await.unwrap();
        stream.write_u8(self.permissions_b).await.unwrap();
        stream.write_f32_le(self.longitude).await.unwrap();
        stream.write_f32_le(self.latitude).await.unwrap();
        stream.write_i32_le(self.rank).await.unwrap();
    }
}

packet! {
    RequestType::BanchoRestart.into(),
    struct NotifyRestart {
        retry_ms: i32
    }
}

#[async_trait]
impl WriteToBinaryStream for NotifyRestart {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream) {
        stream.write_object(self.header()).await;
        stream.write_i32_le(self.retry_ms).await.unwrap();
    }
}
