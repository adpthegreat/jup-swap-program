mod helpers;

use tokio;
use std::env;
use borsh::{BorshDeserialize, BorshSerialize};
use {
    litesvm::LiteSVM,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::{pubkey, Pubkey},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    spl_associated_token_account::{
        get_associated_token_address,
        instruction::create_associated_token_account_idempotent
    },
    spl_token::ID as TOKEN_PROGRAM_ID
};
use jup_swap::{
    quote::QuoteRequest,
    swap::SwapRequest,
    transaction_config::{DynamicSlippageSettings, TransactionConfig},
    JupiterSwapApiClient,
};
use crate::helpers::get_discriminator;

const INPUT_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const INPUT_AMOUNT: u64 = 2_000_000;
const OUTPUT_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

const CPI_SWAP_PROGRAM_ID: Pubkey = pubkey!("HALaoXiDUqEvwCLdoxHRvsDmYJQ5djZH7MozvNwMhuGm");
const JUPITER_V6_AGG_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");


#[derive(BorshSerialize, BorshDeserialize)]
struct SwapIxData {
    pub data: Vec<u8>,
    pub amount: u64,
}

#[tokio::main]
async fn main() {
    let jup_swap_program_id = Pubkey::new_unique();
    let mut svm = LiteSVM::new();
    let api_base_url = env::var("API_BASE_URL").unwrap_or("https://quote-api.jup.ag/v6".into());
     let jupiter_swap_api_client = JupiterSwapApiClient::new(api_base_url);

    let quote_request = QuoteRequest {
        amount: INPUT_AMOUNT,
        input_mint: INPUT_MINT,
        output_mint: OUTPUT_MINT,
        ..QuoteRequest::default()
    };

    // GET /quote
    let quote_response = match jupiter_swap_api_client.quote(&quote_request).await {
        Ok(quote_response) => quote_response,
        Err(e) => {
            println!("quote failed: {e:#?}");
            return;
        }
    };

    let (vault, _) = Pubkey::find_program_address(&[b"vault"], &CPI_SWAP_PROGRAM_ID);

    let response = jupiter_swap_api_client
        .swap_instructions(&SwapRequest {
            user_public_key: vault,
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

    println!("Vault: {}", vault);
    let input_token_account = get_associated_token_address(&vault, &INPUT_MINT);
    let output_token_account = get_associated_token_address(&vault, &OUTPUT_MINT);

    let bytes = include_bytes!("../../jup-swap-program/program_bytes/jup_swap_program.so"); 
    svm.add_program(CPI_SWAP_PROGRAM_ID, bytes);

    svm.add_program_from_file(JUPITER_V6_AGG_PROGRAM_ID, "../../jup-swap-program/program_bytes/jup_agg_v6.so"); //jup agg v6 dump dump with solana program dump command 

    let payer = Keypair::new();
    let payer_address = payer.pubkey();

    let receiver = Keypair::new();
    let receiver_token_account = get_associated_token_address(&vault, &OUTPUT_MINT);

    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let blockhash = svm.latest_blockhash();

    let create_output_ata_ix = create_associated_token_account_idempotent(
        &payer_address,
        &vault,
        &OUTPUT_MINT,
        &TOKEN_PROGRAM_ID,
    );

    let instruction_data = SwapIxData {
        data: response.swap_instruction.data,
        amount: 1000 // any amount
    };

    let mut serialized_data = Vec::from(get_discriminator("global:swap"));
    instruction_data.serialize(&mut serialized_data).unwrap();

    let mut accounts = vec![
        AccountMeta::new_readonly(INPUT_MINT, false), // input mint
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // input mint program (for now, just hardcoded to SPL and not SPL 2022)
        AccountMeta::new_readonly(OUTPUT_MINT, false),      // output mint
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // output mint program (for now, just hardcoded to SPL and not SPL 2022)
        AccountMeta::new(vault, false),                     // vault
        AccountMeta::new(input_token_account, false),       // vault input token account
        AccountMeta::new(output_token_account, false),      // vault output token account
        AccountMeta::new_readonly(JUPITER_V6_AGG_PROGRAM_ID, false), // jupiter program
    ];
    //Add the addtional accounts from the response 
    let remaining_accounts = response.swap_instruction.accounts;
    accounts.extend(remaining_accounts.into_iter().map(|mut account| {
        account.is_signer = false;
        account
    }));

    //Create the instruction
    let ixs = [
        Instruction {
            program_id: CPI_SWAP_PROGRAM_ID,
            data: serialized_data,
            accounts: accounts,
        }
    ];

    //Construct the Versioned message 
    let msg = Message::new_with_blockhash(&ixs, Some(&payer_address), &blockhash);
    let versioned_msg = VersionedMessage::Legacy(msg);

    //Send the transaction
    let tx = VersionedTransaction::try_new(versioned_msg, &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap_err();
}


