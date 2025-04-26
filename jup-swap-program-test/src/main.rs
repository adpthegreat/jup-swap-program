mod helpers;
mod retryable_rpc;

use tokio;
use tokio::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{env,fs};
use borsh::{BorshDeserialize, BorshSerialize};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use {
    litesvm::LiteSVM,
    solana_account::Account,
    solana_client::{rpc_client::RpcClient, rpc_config::RpcSimulateTransactionConfig, client_error::ClientError},
    solana_commitment_config::CommitmentConfig,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_message::{v0::Message, VersionedMessage, AddressLookupTableAccount},
    solana_pubkey::{pubkey, Pubkey},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    solana_hash::Hash,
    spl_associated_token_account::{
        ID as ATA_ID,
        get_associated_token_address,
        instruction::create_associated_token_account_idempotent,
    },
    spl_token::{
         state::{Account as TokenAccount, AccountState},
    },
    // spl_token::ID as TOKEN_PROGRAM_ID
};
use jup_swap::{
    quote::QuoteRequest,
    swap::SwapRequest,
    transaction_config::{DynamicSlippageSettings, TransactionConfig},
    JupiterSwapApiClient,
};
use crate::helpers::{get_account_fields, get_discriminator,get_address_lookup_table_accounts};

const INPUT_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const INPUT_AMOUNT: u64 = 2_000_000;
const OUTPUT_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

const CPI_SWAP_PROGRAM_ID: Pubkey = pubkey!("LMMGrBSX84ZC519PSBkppyVdT4XfM3VP3hw4XLXqhrf");
const JUPITER_V6_AGG_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

struct LatestBlockhash {
    blockhash: RwLock<Hash>,
    slot: AtomicU64,
}

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
    let rpc_url = "http://127.0.0.1:8899";
    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        rpc_url.to_string(),
        CommitmentConfig::confirmed(),
    ));

    let rpc_client_clone = rpc_client.clone();
    
    let latest_blockhash = Arc::new(LatestBlockhash {
        blockhash: RwLock::new(Hash::default()),
        slot: AtomicU64::new(0),
    });


    let latest_blockhash_clone = latest_blockhash.clone();
    tokio::spawn(async move {
        loop {
            if let Ok((blockhash, slot)) =
                rpc_client_clone.get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            {
                let mut blockhash_write = latest_blockhash_clone.blockhash.write().await;
                *blockhash_write = blockhash;
                latest_blockhash_clone.slot.store(slot, Ordering::Relaxed);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

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

    // let balance = svm.get_balance(&vault).unwrap();

    // println!("this is the vault {} balance {}", vault, balance);

    // - we're trying to use the response to get additional data and instructions to execute our swap
    // - if i use the vault pda (pubkey) i get a attempt to debit an account but found no record of a prior credit." prob because its not a mainnet keypair 
    // - but i still get the swap instructions and data though let me see what i can to 
    let response = jupiter_swap_api_client
        .swap_instructions(&SwapRequest {
            user_public_key: pubkey!("Cd8JNmh6iBHJR2RXKJMLe5NRqYmpkYco7anoar1DWFyy"), //payer_address, 
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

    println!("response {:?}", &response);

    let address_lookup_table_accounts =
        get_address_lookup_table_accounts(&rpc_client, response.address_lookup_table_addresses)
            .await
            .unwrap();
    
    println!("Vault: {}", vault);
    
    let recipient = Keypair::new();
    let recipient_address = recipient.pubkey();

    println!("Recipient Address: {}", recipient_address);

    let input_token_account = get_associated_token_address(&vault, &INPUT_MINT);
    let output_token_account = get_associated_token_address(&vault, &OUTPUT_MINT);
    let recipient_token_account = get_associated_token_address(&recipient_address, &OUTPUT_MINT);

    println!("Input Token Account: {}", input_token_account);
    println!("Output Token Account: {}", output_token_account);

    println!("Recipient Token Account: {}", recipient_token_account);
    
    let create_output_ata_ix = create_associated_token_account_idempotent(
        &payer.pubkey(),
        &vault,
        &OUTPUT_MINT,
        &TOKEN_PROGRAM_ID,
    );
    let create_recipient_ata_ix = create_associated_token_account_idempotent(
        &payer.pubkey(),
        &recipient_address,
        &OUTPUT_MINT,
        &TOKEN_PROGRAM_ID,
    );

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
        AccountMeta::new_readonly(recipient_address, false),                  // recipient 
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
    let swap_ix = Instruction {
            program_id: CPI_SWAP_PROGRAM_ID,
            accounts: accounts,
            data: serialized_data
    };
    let simulate_cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
    let cup_ix = ComputeBudgetInstruction::set_compute_unit_price(200_000);
    loop {
        let slot = latest_blockhash.slot.load(Ordering::Relaxed);
        if slot != 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let recent_blockhash = latest_blockhash.blockhash.read().await;

    let simulate_message = Message::try_compile(
        &payer_address,
        &[
            simulate_cu_ix,
            cup_ix.clone(),
            create_output_ata_ix.clone(),
            create_recipient_ata_ix.clone(),
            swap_ix.clone(),
        ],
        &address_lookup_table_accounts,
        *recent_blockhash,
    )
    .unwrap();

    println!("simulate_message {:?}", simulate_message);
    
    let simulate_tx =
        VersionedTransaction::try_new(VersionedMessage::V0(simulate_message), &[&payer]).unwrap();

    println!("simulate_tx {:?}", &simulate_tx);

    let simulated_cu = match rpc_client.simulate_transaction_with_config(
        &simulate_tx,
        RpcSimulateTransactionConfig {
            replace_recent_blockhash: true,
            ..RpcSimulateTransactionConfig::default()
        },
    ) {
        Ok(simulate_result) => {
            println!("simulate_result {:?}", simulate_result);
            if simulate_result.value.err.is_some() {
                let e = simulate_result.value.err.unwrap();
                panic!(
                    "Failed to simulate transaction due to {:?} logs:{:?}",
                    e, simulate_result.value.logs
                );
            }
            simulate_result.value.units_consumed.unwrap()
        }
        Err(e) => {
            panic!("simulate failed: {e:#?}");
        }  
    };

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit((simulated_cu + 10_000) as u32);

    let recent_blockhash = latest_blockhash.blockhash.read().await;
    println!("Latest blockhash: {}", recent_blockhash);
    let message = Message::try_compile(
        &payer_address,
        &[cu_ix, cup_ix, create_output_ata_ix, create_recipient_ata_ix, swap_ix],
        &address_lookup_table_accounts,
        *recent_blockhash,
    )
    .unwrap();

    println!(
        "Base64 EncodedTransaction message: {}",
        STANDARD
            .encode(VersionedMessage::V0(message.clone()).serialize())
    );
    let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&payer]).unwrap();
    let retryable_client = retryable_rpc::RetryableRpcClient::new(&rpc_url);

    let tx_hash = tx.signatures[0];

    if let Ok(tx_hash) = retryable_client.send_and_confirm_transaction(&tx).await {
        println!(
            "Transaction confirmed {}",
            tx_hash
        );
    } else {
        println!(
            "Transaction failed {}",
            tx_hash
        );
        return;
    };
}


