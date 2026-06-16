use jsonwebtoken::{DecodingKey, EncodingKey, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub struct Jwt {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl Jwt {
    pub fn new(jwt_secret: String) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(jwt_secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(jwt_secret.as_bytes()),
        }
    }

    pub fn encode(&self, claims: &Claims) -> Result<String, Box<dyn Error>> {
        let header = jsonwebtoken::Header::default();
        let token = encode(&header, claims, &self.encoding_key)?;
        Ok(token)
    }

    pub fn decode(&self, token: &str) -> Result<Claims, Box<dyn Error>> {
        let token_data = decode(token, &self.decoding_key, &Validation::default())?;
        Ok(token_data.claims)
    }
}
