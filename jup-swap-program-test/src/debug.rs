use {
    base64::{prelude::BASE64_STANDARD, Engine},
    bincode::{config::Options, serialize},
    serde_json::{json, Value},
    jsonrpc_core::{
        futures::future::{self, FutureExt, OptionFuture},
        types::error,
        BoxFuture, Error, Metadata, Result,
    },
    solana_account_decoder_client_types::{
        token::{TokenAccountType, UiTokenAccount, UiTokenAmount},
        UiAccount, UiAccountData, UiAccountEncoding,
    },
    solana_pubkey::Pubkey,
    solana_rpc_client_api::{
        client_error::{
            Error as ClientError, ErrorKind as ClientErrorKind, Result as ClientResult,
        },
        config::{RpcAccountInfoConfig, *},
        request::{RpcError, RpcRequest, RpcResponseErrorData, TokenAccountsFilter},
        response::*,
    },
    solana_transaction_status_client_types::{
        EncodedConfirmedBlock, EncodedConfirmedTransactionWithStatusMeta, TransactionStatus,
        UiConfirmedBlock, UiTransactionEncoding,TransactionBinaryEncoding
    },
    std::{
        any::type_name,
        convert::TryFrom,
        str::FromStr,
        time::Duration,
    },
};

const MAX_BASE58_SIZE: usize = 1683; // Golden, bump if PACKET_DATA_SIZE changes
const MAX_BASE64_SIZE: usize = 1644; // Golden, bump if PACKET_DATA_SIZE changes
/// Maximum over-the-wire size of a Transaction

///   1280 is IPv6 minimum MTU

///   40 bytes is the size of the IPv6 header

///   8 bytes is the size of the fragment header

pub const PACKET_DATA_SIZE: usize = 1280 - 40 - 8; 

pub fn serialize_and_encode<T>(input: &T, encoding: UiTransactionEncoding) -> ClientResult<String>
where
    T: serde::ser::Serialize,
{
    let serialized = serialize(input)
        .map_err(|e| ClientErrorKind::Custom(format!("Serialization failed: {e}")))?;
    let encoded = match encoding {
        UiTransactionEncoding::Base58 => bs58::encode(serialized).into_string(),
        UiTransactionEncoding::Base64 => BASE64_STANDARD.encode(serialized),
        _ => {
            return Err(ClientErrorKind::Custom(format!(
                "unsupported encoding: {encoding}. Supported encodings: base58, base64"
            ))
            .into())
        }
    };
    Ok(encoded)
}

pub fn decode_and_deserialize<T>(
    encoded: String,
    encoding: TransactionBinaryEncoding,
) -> Result<(Vec<u8>, T)>
where
    T: serde::de::DeserializeOwned,
{
    let wire_output = match encoding {
        TransactionBinaryEncoding::Base58 => {
            if encoded.len() > MAX_BASE58_SIZE {
                return Err(Error::invalid_params(format!(
                    "base58 encoded {} too large: {} bytes (max: encoded/raw {}/{})",
                    type_name::<T>(),
                    encoded.len(),
                    MAX_BASE58_SIZE,
                    PACKET_DATA_SIZE,
                )));
            }
            bs58::decode(encoded)
                .into_vec()
                .map_err(|e| Error::invalid_params(format!("invalid base58 encoding: {e:?}")))?
        }
        TransactionBinaryEncoding::Base64 => {
            if encoded.len() > MAX_BASE64_SIZE {
                return Err(Error::invalid_params(format!(
                    "base64 encoded {} too large: {} bytes (max: encoded/raw {}/{})",
                    type_name::<T>(),
                    encoded.len(),
                    MAX_BASE64_SIZE,
                    PACKET_DATA_SIZE,
                )));
            }
            BASE64_STANDARD
                .decode(encoded)
                .map_err(|e| Error::invalid_params(format!("invalid base64 encoding: {e:?}")))?
        }
    };
    if wire_output.len() > PACKET_DATA_SIZE {
        return Err(Error::invalid_params(format!(
            "decoded {} too large: {} bytes (max: {} bytes)",
            type_name::<T>(),
            wire_output.len(),
            PACKET_DATA_SIZE
        )));
    }
    bincode::options()
        .with_limit(PACKET_DATA_SIZE as u64)
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .deserialize_from(&wire_output[..])
        .map_err(|err| {
            Error::invalid_params(format!(
                "failed to deserialize {}: {}",
                type_name::<T>(),
                &err.to_string()
            ))
        })
        .map(|output| (wire_output, output))
}

//address lookup table accounts: []
// if the ALTs are not gotten, that is we get an empty [] , then we get this error when it tries to deserialize server side
//         simulated_cu: Err(
//             Error {
//                 request: Some(SimulateTransaction),
//                 kind: RpcError(
//                     RpcResponseError {
//                         code: -32602,
//                         message: "failed to deserialize solana_transaction::Transaction: io error: failed to fill whole buffer",
//                         data: Empty,
//                     }
//                 ),
//             }
//         )
// it can also make the txn really large so we end up with this error 
// Err(
//     Error {
//       request: Some(SimulateTransaction),
//       kind: RpcError(
//         RpcResponseError {
//           code: -32602,
//           message: "base64 encoded solana_transaction::Transaction too large: 1964 bytes (max: encoded/raw 1644/1232)",
//           data: Empty,
//         }
//       ),
//     }
//   )

// make sure we are getting the address lookup tables !!