use async_trait::async_trait;
use core::panic;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[async_trait]
pub trait WriteToBinaryStream {
    async fn write_to(&self, stream: &mut tokio::net::TcpStream);
}

#[async_trait]
pub trait StreamExt {
    async fn read_length_string(&mut self) -> String;
    async fn read_length_i32_array(&mut self) -> Vec<i32>;

    async fn write_length_string(&mut self, s: &str);
    async fn write_object<B: WriteToBinaryStream + Send + Sync>(&mut self, b: B);
}

#[async_trait]
impl StreamExt for tokio::net::TcpStream {
    async fn read_length_i32_array(&mut self) -> Vec<i32> {
        let length = self.read_i16_le().await.unwrap();
        let mut v = Vec::with_capacity(length as usize);
        for _ in 0..length {
            let val = self.read_i32_le().await.unwrap();
            v.push(val);
        }
        v
    }

    async fn read_length_string(&mut self) -> String {
        let is_null = self.read_u8().await.unwrap();
        if is_null == 0 {
            panic!("Null string?");
        }
        let len = self.read_u8().await.unwrap();
        let mut buf = vec![0; len as usize];
        self.read_exact(&mut buf).await.unwrap();
        String::from_utf8(buf).unwrap()
    }

    async fn write_length_string(&mut self, s: &str) {
        self.write_u8(1).await.unwrap(); // Not Null?
        let len = s.len();
        if len >= 2usize.pow(7) - 1 {
            panic!("Not yet supported!");
        }
        self.write_u8(len as u8).await.unwrap();
        self.write(s.as_bytes()).await.unwrap();
    }

    async fn write_object<B: WriteToBinaryStream + Send + Sync>(&mut self, b: B) {
        b.write_to(self).await;
    }
}
