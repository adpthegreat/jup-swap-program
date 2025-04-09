use std::fs::File;
use std::io::Read;
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize, Deserializer};
use serde::de::Error as DeError;
use serde::de;
use base64::{decode, Engine};
use base64::engine::general_purpose::STANDARD as base64_engine;
// Function to generate Anchor's instruction discriminator
pub fn get_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let result = hasher.finalize();
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&result[..8]);
    discriminator
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AccountData {
    pub account: Account,
    pub pubkey: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Account {
    #[serde(deserialize_with = "deserialize_base64_data")]
    pub data: (Vec<u8>, String), // Tuple: (base64_data, encoding)
    pub executable: bool,
    pub lamports: u64,
    pub owner: String,
    pub rentEpoch: u64,
    pub space: u64,
}

fn deserialize_base64_data<'de, D>(deserializer: D) -> Result<(Vec<u8>, String), D::Error>
where
    D: Deserializer<'de>,
{
    let raw: (String, String) = Deserialize::deserialize(deserializer)?;
    let decoded = decode(&raw.0).map_err(D::Error::custom)?;
    Ok((decoded, raw.1))
}

pub fn get_account_fields(filepath: &str) -> Result<AccountData, Box<dyn std::error::Error>> {
    let mut file = File::open(filepath)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let parsed: AccountData = serde_json::from_str(&contents)?;

    // println!("Pubkey: {}", parsed.pubkey);
    // println!("Owner: {}", parsed.account.owner);
    // println!("Lamports: {}", parsed.account.lamports);
    // println!("Executable: {}", parsed.account.executable);
    // println!("Rent Epoch: {}", parsed.account.rentEpoch);
    // println!("Space: {}", parsed.account.space);
    // println!("Base64 Data: {}", parsed.account.data.0);
    // println!("Encoding: {}", parsed.account.data.1);

    Ok(parsed)
}
