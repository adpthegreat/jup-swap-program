 //Incomplete / Doesn't work 
 mod helpers;

use tokio;
use std::{env,fs};
use borsh::{BorshDeserialize, BorshSerialize};
use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::{pubkey, Pubkey},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    spl_associated_token_account::{
        ID as ATA_ID,
        get_associated_token_address,
    },
    spl_token::{
         state::{Account as TokenAccount, AccountState},
    }
    // spl_token::ID as TOKEN_PROGRAM_ID
};
use jup_swap::{
    quote::QuoteRequest,
    swap::SwapRequest,
    transaction_config::{DynamicSlippageSettings, TransactionConfig},
    JupiterSwapApiClient,
};
use crate::helpers::{get_account_fields, get_discriminator};

const INPUT_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const INPUT_AMOUNT: u64 = 2_000_000;
const OUTPUT_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

const CPI_SWAP_PROGRAM_ID: Pubkey = pubkey!("LMMGrBSX84ZC519PSBkppyVdT4XfM3VP3hw4XLXqhrf");
const JUPITER_V6_AGG_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
const JUPITER_V6_PROGRAM_EXECUTABLE_DATA_ACCOUNT: Pubkey = pubkey!("4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT");
const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");


#[derive(BorshSerialize, BorshDeserialize)]
struct SwapIxData {
    pub data: Vec<u8>,
    pub amount: u64,
}

#[tokio::main]
async fn main() {
    println!("Starting Jupiter Swap...");
    let jup_swap_program_id = Pubkey::new_unique();
    let mut svm = LiteSVM::new();
    let api_base_url = env::var("API_BASE_URL").unwrap_or("https://quote-api.jup.ag/v6".into());
    let jupiter_swap_api_client = JupiterSwapApiClient::new(api_base_url);

    println!("Fetching quote...");
    let quote_request = QuoteRequest {
        amount: INPUT_AMOUNT,
        input_mint: INPUT_MINT,
        output_mint: OUTPUT_MINT,
        ..QuoteRequest::default()
    };

    // GET /quote
    let quote_response = match jupiter_swap_api_client.quote(&quote_request).await {
        Ok(quote_response) => {
            // println!("Quote received successfully {:?}.", quote_response);
            println!("Quote received successfully.");
            quote_response
        },
        Err(e) => {
            println!("quote failed: {e:#?}");
            return;
        }
    };

    let payer = Keypair::new();
    let payer_address = payer.pubkey();
    svm.airdrop(&payer_address, 1_000_000_000).unwrap();
    println!("Payer Address: {}", payer_address);

    let (vault, _) = Pubkey::find_program_address(&[b"vault"], &CPI_SWAP_PROGRAM_ID);
    
    svm.airdrop(&vault, 1_000_000_000).unwrap(); 

    let balance = svm.get_balance(&vault).unwrap();

    println!("this is the vault {} balance {}", vault, balance);

    // - we're trying to use the response to get additional data and instructions to execute our swap
    // - if i use the vault pda (pubkey) i get a attempt to debit an account but found no record of a prior credit." prob because its not a mainnet keypair 
    // - but i still get the swap instructions and data though let me see what i can to 
    let response = jupiter_swap_api_client
        .swap_instructions(&SwapRequest {
            user_public_key: payer_address, //pubkey!("Cd8JNmh6iBHJR2RXKJMLe5NRqYmpkYco7anoar1DWFyy"), //payer_address, 
            quote_response,
            config: TransactionConfig {
                skip_user_accounts_rpc_calls: true,
                wrap_and_unwrap_sol: false,
                dynamic_compute_unit_limit: true,
                dynamic_slippage: Some(DynamicSlippageSettings {
                    min_bps: Some(50),
                    max_bps: Some(1000),
                }),
                ..TransactionConfig::default()
            },
        })
        .await
        .unwrap();

    println!("response {:?}", response);
    println!("Vault: {}", vault);
    let input_token_account = get_associated_token_address(&vault, &INPUT_MINT);
    let output_token_account = get_associated_token_address(&vault, &OUTPUT_MINT);
    println!("Input Token Account: {}", input_token_account);
    println!("Output Token Account: {}", output_token_account);

    let bytes = include_bytes!("../../jup-swap-program/target/deploy/jup_swap_program.so"); 
    svm.add_program(CPI_SWAP_PROGRAM_ID, bytes);
    
    //modify path 
    svm.add_program_from_file(JUPITER_V6_AGG_PROGRAM_ID, "../../jup-swap-program/program_bytes/jup_agg_v6.so"); //jup agg v6 dump dump with solana program dump command 

    let recipient = Keypair::new();
    let recipient_address = recipient.pubkey();
    println!("Recipient Address: {}", recipient_address);

    let recipient_token_account = get_associated_token_address(&recipient_address, &OUTPUT_MINT);
     
    let vault_input_token_acc = TokenAccount {
        mint: INPUT_MINT,
        owner: vault,
        amount: 0,
        delegate: COption::None,  
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    println!("vault_input_token_acc owner: {}",vault_input_token_acc.owner);

    let vault_output_token_acc = TokenAccount { 
        mint: OUTPUT_MINT,
        owner: vault,
        amount: 0,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };

    let recipient_output_token_acc = TokenAccount {
        mint: OUTPUT_MINT,
        owner: recipient_address,
        amount: 0,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    // I used only one token_acc_bytes and shared it for the three accounts, an i was getting an account ownership error 2015 and the wrong owner ,
    // was the recipient pubkey  because `TokenAccount::pack` packed it as the last data for the account 
    //  let mut token_acc_bytes = [0u8; TokenAccount::LEN];
    let mut input_token_acc_bytes = [0u8; TokenAccount::LEN];
    let mut output_token_acc_bytes = [0u8; TokenAccount::LEN];
    let mut recipient_token_acc_bytes = [0u8; TokenAccount::LEN];   

     TokenAccount::pack(vault_input_token_acc , &mut input_token_acc_bytes).unwrap();
     TokenAccount::pack(vault_output_token_acc , &mut output_token_acc_bytes).unwrap();
     TokenAccount::pack(recipient_output_token_acc, &mut recipient_token_acc_bytes).unwrap();
     svm.set_account(
         input_token_account,
         Account {
             lamports: 0,
             data: input_token_acc_bytes.to_vec(),
             owner: TOKEN_PROGRAM_ID,
             executable: false,
             rent_epoch: 0,
            },
        )
        .unwrap();
         println!("vault_input_token_acc owner heereeeeee: {}",vault_input_token_acc.owner);
    svm.set_account(
        output_token_account,
        Account {
            lamports: 0,
            data: output_token_acc_bytes.to_vec(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        recipient_token_account,
        Account {
            lamports: 0,
            data: recipient_token_acc_bytes.to_vec(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    //set the mints, initialize them
    svm.set_account(
        INPUT_MINT,
        Account {
            lamports: 1_017_845_286_023,
            data: base64::decode("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==").unwrap(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: std::u64::MAX,
        }
    )
    .unwrap();
    svm.set_account(
        OUTPUT_MINT,
        Account {
            lamports: 387_385_103_258,
            data: base64::decode("AQAAAJj+huiNm+Lqi8HMpIeLKYjCQPUrhCS/tA7Rot3LXhmbmio8hjXFIgAGAQEAAABicKqKWcWUBbRShshncubNEm6bil06OFNtN/e0FOi2Zw==").unwrap(), 
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: std::u64::MAX,
        }
    )
    .unwrap();
     //set the programdata_address of JUPITER_V6 or else we get an error 
    //https://github.com/LiteSVM/litesvm/blob/c572f9091827692bb8776a8f54b82f1d02014e6e/crates/litesvm/src/accounts_db.rs#L255
    //that is, the program data executable account address
    let jup_v6_program_data_bytes = get_account_fields("../JUP_V6_PROGRAM_DATA_ACCOUNT.json")
                                    .unwrap()
                                    .account
                                    .data
                                    .0;
    svm.set_account(
        JUPITER_V6_PROGRAM_EXECUTABLE_DATA_ACCOUNT,
        Account {
            lamports: 20131083120,
            data: jup_v6_program_data_bytes,
            owner: pubkey!("BPFLoaderUpgradeab1e11111111111111111111111"),
            executable: false,
            rent_epoch: u64::MAX, // 18446744073709551615
        }
    )
    .unwrap();
    svm.set_account(
        JUPITER_V6_AGG_PROGRAM_ID,
        Account {
            lamports: 1_141_440,
            data: base64::decode("AgAAADAPUGBbvrcwh4nmwPvj5KNgIENvbqDK2oAmDbRv9mCE").unwrap(),
            owner: pubkey!("BPFLoaderUpgradeab1e11111111111111111111111"),
            executable: true,
            rent_epoch: u64::MAX, // 18446744073709551615
        }
    )
    .unwrap();

    println!("Swap Instruction Data: {:?}", response.swap_instruction.data);
    let instruction_data = SwapIxData {
        data: response.swap_instruction.data,
        amount: 100 // any amount tbh
    };

    let mut serialized_data = Vec::from(get_discriminator("global:swap"));
    instruction_data.serialize(&mut serialized_data).unwrap();
    println!("Serialized Swap Instruction Data: {:?}", serialized_data);
//it iterates through and tries to resolve all the accounts passed in the instruction
    let mut accounts = vec![
        AccountMeta::new_readonly(INPUT_MINT, false), // input mint
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // input mint program (for now, just hardcoded to SPL and not SPL 2022)
        AccountMeta::new_readonly(OUTPUT_MINT, false),      // output mint
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // output mint program (for now, just hardcoded to SPL and not SPL 2022)
        AccountMeta::new(vault, false),                     // vault
        AccountMeta::new(input_token_account, false),       // vault input token account
        AccountMeta::new(output_token_account, false),      // vault output token account
        AccountMeta::new(recipient_token_account, false),    // recipient token account
        AccountMeta::new(recipient_address, false),                  // recipient 
        AccountMeta::new_readonly(ATA_ID, false),                       // ATA program
        AccountMeta::new_readonly(JUPITER_V6_AGG_PROGRAM_ID, false), // jupiter program
    ];
    // //Add the addtional accounts from the response 
    let remaining_accounts = response.swap_instruction.accounts;
    accounts.extend(remaining_accounts.into_iter().map(|mut account| {
        account.is_signer = false;
        account
    }));

    //Create the instruction
    let ixs = [
        Instruction {
            program_id: CPI_SWAP_PROGRAM_ID,
            accounts: accounts,
            data: serialized_data,
        }
    ];
    let blockhash = svm.latest_blockhash();
    println!("Latest Blockhash: {}", blockhash);
    // //Construct the Versioned message 
    let msg = Message::new_with_blockhash(&ixs, Some(&payer_address), &blockhash);
    let versioned_msg = VersionedMessage::Legacy(msg);

    //Send the transaction
    let tx = VersionedTransaction::try_new(versioned_msg, &[&payer]).unwrap();
    
    match svm.send_transaction(tx) {
        Ok(res) => println!("transaction success {:?}", res),
        Err(res) => println!("transaction failure {:?}", res)
    };
}


// https://github.com/jup-ag/jupiter-swap-api-client/blob/1554823f17b7a6f035f89847da6b44df1aaced59/jupiter-swap-api-client/src/transaction_config.rs#L194