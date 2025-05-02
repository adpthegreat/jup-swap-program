use std::sync::Arc;
use std::fs::File;
use std::io::Read;
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize, Deserializer};
use serde::de::Error as DeError;
use serde::de;
use anyhow;
use anyhow::bail;
use base64::{decode, Engine};
use base64::engine::general_purpose::STANDARD as base64_engine;
use {
    solana_client::client_error::ClientError,
    solana_client::rpc_request::RpcError,
    solana_client::rpc_client::RpcClient,
    solana_pubkey::{pubkey, Pubkey},
    solana_address_lookup_table_interface::state::AddressLookupTable,
    solana_message::{v0::Message, VersionedMessage, AddressLookupTableAccount}
};
use crate::retryable_rpc::MAX_RETRY_COUNT;

// Function to generate Anchor's instruction discriminator
pub fn get_discriminator(name: &str) -> [u8; 8] { 
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let result = hasher.finalize();
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&result[..8]);
    discriminator
}

//function to get lookup table accounts 
//i wanted to make a retry where i would retry each account fetching, but theres a possibility 
// that some accounts can be gotten and some not, but we want all ALTs so that operation should 
// be atomic and we'll go with the retry below 

#[allow(dead_code)]
pub async fn get_address_lookup_table_accounts(
    rpc_client: &Arc<RpcClient>,
    addresses: Vec<Pubkey>,
) -> Result<Vec<AddressLookupTableAccount>, ClientError> {
    let mut accounts = Vec::new();
    for key in addresses {
        if let Ok(account) = rpc_client.get_account(&key) {
            if let Ok(address_lookup_table_account) = AddressLookupTable::deserialize(&account.data)
            {
                accounts.push(AddressLookupTableAccount {
                    key,
                    addresses: address_lookup_table_account.addresses.to_vec(),
                });
            }
        }
    }
    Ok(accounts)
}

//function to get lookup table accounts with retry
//will refactor this function to include the Vec<Alts> in the error 
#[allow(dead_code)]
pub async fn get_address_lookup_table_accounts_with_retry(
    rpc_client: &Arc<RpcClient>,
    addresses: Vec<Pubkey>,
) -> Result<Vec<AddressLookupTableAccount>, ClientError> { //can change to ClientResult<Vec<AddressLookupTableAccount>>
    let mut retry_count = 0;
    'retry_loop: for retry_count in 0..MAX_RETRY_COUNT {
        println!("Retry attempt #{}", retry_count + 1);
        match get_address_lookup_table_accounts(&rpc_client, addresses.clone()).await {
            Ok(accounts) => {
                if accounts.is_empty() {
                    println!("Received empty account list, retrying...");
                    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
                    continue 'retry_loop;
                }
                println!("Successfully retrieved {} accounts", accounts.len());
                return Ok(accounts)
            },
            Err(err) => {
                    println!("Error on attempt #{}: {:?}", retry_count + 1, err);
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    continue 'retry_loop;
            }
        }
    }
    Err(RpcError::ForUser(
        "Max retry count reached. \
         Failed to get lookup table accounts"
            .to_string(),
    )
    .into()) 
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

