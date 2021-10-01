#[derive(Debug, serde::Deserialize)]
pub struct User {
    pub username: String,
    pub id: i32,
    pub profile_image: Option<String>,

    pub total_score: i64,
    pub ranked_score: i64,
    pub accuracy: f32,
    pub play_count: i32,
    pub performance_points: f32,
}

pub struct Credentials {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug)]
pub enum LoginError {
    InvalidUsername,
    InvalidPassword,
    UnloginableUser,
}

#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
}

impl Client {
    pub fn new(client: reqwest::Client) -> Self {
        Client { client }
    }

    pub async fn login(&self, credentials: &Credentials) -> Result<i32, LoginError> {
        let resp = self
            .client
            .get(format!(
                "http://localhost/api/login?u={}&p={}",
                credentials.username, credentials.password_hash
            ))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        match resp.as_str() {
            "invalid username" => Err(LoginError::InvalidUsername),
            "invalid password" => Err(LoginError::InvalidPassword),
            "unloginable user" => Err(LoginError::UnloginableUser),
            s => Ok(s.parse().unwrap()),
        }
    }

    pub async fn get_user(&self, user_id: i32) -> User {
        self.client
            .get(format!("http://localhost/api/user/{}/info", user_id))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    pub async fn get_rank(&self, user_id: i32) -> i32 {
        self.client
            .get(format!("http://localhost/api/user/{}/rank", user_id))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    pub async fn get_username(&self, user_id: i32) -> String {
        self.client
            .get(format!("http://localhost/api/user/{}/username", user_id))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    }
}
