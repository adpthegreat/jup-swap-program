mod helpers;

use tokio;
use std::env;
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
        instruction::create_associated_token_account_idempotent
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
use crate::helpers::get_discriminator;

const INPUT_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const INPUT_AMOUNT: u64 = 2_000_000;
const OUTPUT_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

const CPI_SWAP_PROGRAM_ID: Pubkey = pubkey!("HALaoXiDUqEvwCLdoxHRvsDmYJQ5djZH7MozvNwMhuGm");
const JUPITER_V6_AGG_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
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

    println!("this is the vault balance {}", balance);

    // - we're trying to use the response to get additional data and instructions to execute our swap
    // - if i use the vault pda (pubkey) i get a attempt to debit an account but found no record of a prior credit." prob because its not a mainnet keypair 
    // - but i still get the swap instructions and data though let me see what i can to 
    let response = jupiter_swap_api_client
        .swap_instructions(&SwapRequest {
            user_public_key: vault,//pubkey!("Cd8JNmh6iBHJR2RXKJMLe5NRqYmpkYco7anoar1DWFyy"), 
            quote_response,
            config: TransactionConfig {
                skip_user_accounts_rpc_calls: true,
                wrap_and_unwrap_sol: false, //true 
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

    // println!("response {:?}", response);
    println!("Vault: {}", vault);
    let input_token_account = get_associated_token_address(&vault, &INPUT_MINT);
    let output_token_account = get_associated_token_address(&vault, &OUTPUT_MINT);
    println!("Input Token Account: {}", input_token_account);
    println!("Output Token Account: {}", output_token_account);

    let bytes = include_bytes!("../../jup-swap-program/target/deploy/jup_swap_program.so"); 
    svm.add_program(CPI_SWAP_PROGRAM_ID, bytes);

    svm.add_program_from_file(JUPITER_V6_AGG_PROGRAM_ID, "../../jup-swap-program/program_bytes/jup_agg_v6.so"); //jup agg v6 dump dump with solana program dump command 

    let recipient = Keypair::new();
    let recipient_address = recipient.pubkey();
    println!("Recipient Address: {}", recipient_address);

    let recipient_token_account = get_associated_token_address(&vault, &OUTPUT_MINT);
     
    let create_input_ata_ix = create_associated_token_account_idempotent( 
        &payer_address, 
        &vault,
        &INPUT_MINT,
        &TOKEN_PROGRAM_ID,
    );

    let create_output_ata_ix = create_associated_token_account_idempotent(
        &payer_address, 
        &vault,
        &OUTPUT_MINT,
        &TOKEN_PROGRAM_ID,
    );

    let create_recipient_ata_ix = create_associated_token_account_idempotent(
        &payer_address,
        &recipient_address,
        &OUTPUT_MINT,
        &TOKEN_PROGRAM_ID,
    );
    
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

    let mut token_acc_bytes = [0u8; TokenAccount::LEN];
     TokenAccount::pack(vault_input_token_acc , &mut token_acc_bytes).unwrap();
     TokenAccount::pack(vault_output_token_acc , &mut token_acc_bytes).unwrap();
     TokenAccount::pack(recipient_output_token_acc, &mut token_acc_bytes).unwrap();

    svm.set_account(
        input_token_account,
        Account {
            lamports: 1_000_000_000,
            data: token_acc_bytes.to_vec(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    svm.set_account(
        output_token_account,
        Account {
            lamports: 1_000_000_000,
            data: token_acc_bytes.to_vec(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        recipient_token_account,
        Account {
            lamports: 1_000_000_000,
            data: token_acc_bytes.to_vec(),
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let instruction_data = SwapIxData {
        data: response.swap_instruction.data,
        amount: 10000 // any amount tbh
    };

    let mut serialized_data = Vec::from(get_discriminator("global:swap"));
    instruction_data.serialize(&mut serialized_data).unwrap();
    println!("Serialized Swap Instruction Data: {:?}", serialized_data);

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
        create_input_ata_ix,
        create_output_ata_ix,
        create_recipient_ata_ix,
        Instruction {
            program_id: CPI_SWAP_PROGRAM_ID,
            data: serialized_data,
            accounts: accounts,
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




//with the ata creation ixs i get this error 
// Starting Jupiter Swap...
// Fetching quote...
// Quote received successfully.
// Payer Address: EbhvvChqxcW8Zwte7NEXhqMrAsFWZt8dTtZEMPjKyQ4i
// this is the vault balance 1000000000
// Vault: FmtgX2F84UJWc7icEwRhmTQQL119q69dVviCG8q2YUoQ
// Input Token Account: Dhqb1fJ6wwVKzhHCcGZdihi4ciaicMTrJUk1VroJdhMW
// Output Token Account: AMcGsR8qAWnndBWFKqQfitEeSNvRTrnKEwrKvcXyN6qj
// Recipient Address: ApXeaiFKdeGGX4rj48noSgEmYR2RZUD8dTLtuXoVZZp
// Serialized Swap Instruction Data: [248, 198, 158, 145, 225, 117, 135, 200, 42, 0, 0, 0, 193, 32, 155, 51, 65, 214, 156, 129, 6, 2, 0, 0, 0, 58, 1, 100, 0, 1, 58, 0, 100, 1, 2, 128, 132, 30, 0, 0, 0, 0, 0, 183, 88, 4, 0, 0, 0, 0, 0, 50, 0, 0, 16, 39, 0, 0, 0, 0, 0, 0]
// Latest Blockhash: CmpNeggWJ4JaWJeJ8YKN1Zypmk7uvQq3PECGUCAEMbky
// transaction failure FailedTransactionMetadata { err: InstructionError(0, IncorrectProgramId), meta: TransactionMetadata { signature: 2txtJGzMTVMTCkvBp4udrhV7eNiFVYdpjua7vTUptZGBPdVYWjLxmzWT7ZhHL33E7HMRmHhPAixp56BHU3demhWf, logs: ["Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL invoke [1]", "Program log: CreateIdempotent", "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]", "Program log: Instruction: GetAccountDataSize", "Program log: Error: IncorrectProgramId", "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 884 of 794518 compute units", "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA failed: incorrect program id for instruction", "Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL consumed 6366 of 800000 compute units", "Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL failed: incorrect program id for instruction"], inner_instructions: [[InnerInstruction { instruction: CompiledInstruction { program_id_index: 20, accounts: [18], data: [21, 7, 0] }, stack_height: 2 }]], compute_units_consumed: 6366, return_data: TransactionReturnData { program_id: TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA, data: [] } } }


//with the ata ix commented out, i get this error 

// Starting Jupiter Swap...
// Fetching quote...
// Quote received successfully.
// Payer Address: 9mTdF4i5v3BQdtjvZuEdSSXPRuPFzF6eqAyYcvjYiUJN
// this is the vault balance 1000000000
// Vault: FmtgX2F84UJWc7icEwRhmTQQL119q69dVviCG8q2YUoQ
// Input Token Account: Dhqb1fJ6wwVKzhHCcGZdihi4ciaicMTrJUk1VroJdhMW
// Output Token Account: AMcGsR8qAWnndBWFKqQfitEeSNvRTrnKEwrKvcXyN6qj
// Recipient Address: 2cPGGPW4fzpHj1FwPJJHf2BRTkV5Q88zn4JX1oYv5D2V
// Serialized Swap Instruction Data: [248, 198, 158, 145, 225, 117, 135, 200, 41, 0, 0, 0, 193, 32, 155, 51, 65, 214, 156, 129, 5, 2, 0, 0, 0, 25, 100, 0, 1, 61, 0, 100, 1, 2, 128, 132, 30, 0, 0, 0, 0, 0, 34, 91, 4, 0, 0, 0, 0, 0, 50, 0, 0, 16, 39, 0, 0, 0, 0, 0, 0]
// Latest Blockhash: CmpNeggWJ4JaWJeJ8YKN1Zypmk7uvQq3PECGUCAEMbky
// transaction failure FailedTransactionMetadata { err: InstructionError(0, Custom(0)), meta: TransactionMetadata { signature: MUMNfbXDyW3UuaDn8UHZcR67HiewrGqKJhBbkpLeXyv7rZxKt9ngFnTshhbhQP423pK7AFWMcJpdxQotjwE8jo6, logs: ["Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL invoke [1]", "Program log: CreateIdempotent", "Program log: Associated token account owner does not match address derivation", "Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL consumed 4993 of 800000 compute units", "Program ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL failed: custom program error: 0x0"], inner_instructions: [[]], compute_units_consumed: 4993, return_data: TransactionReturnData { program_id: ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL, data: [] } } }