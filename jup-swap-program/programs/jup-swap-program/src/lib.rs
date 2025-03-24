use anchor_lang::{prelude::*,solana_program::{instruction::Instruction, program::invoke_signed}};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface, Transfer, transfer, TransferChecked, transfer_checked};
use jupiter_aggregator::program::Jupiter;
use std::str::FromStr;

declare_program!(jupiter_aggregator);
declare_id!("HALaoXiDUqEvwCLdoxHRvsDmYJQ5djZH7MozvNwMhuGm");

//  - accepts a receiver pubkey and an amount u64
//  - does the purchase of amount of some token on Jupiter
//  - sends the purchased tokens to receiver

const VAULT_SEED: &[u8] = b"vault";

pub fn jupiter_program_id() -> Pubkey {
    Pubkey::from_str("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4").unwrap()
}

#[program]
pub mod jup_swap_program {
    use super::*;

      pub fn swap(ctx: Context<Swap>, data: Vec<u8>, amount: u64) -> Result<()> {
        //validate jupiter program id
        require_keys_eq!(*ctx.accounts.jupiter_program.key, jupiter_program_id());

        //Convert the reamaining accounts gotten from the jupiter swap api to Account Meta Objects
        let accounts: Vec<AccountMeta> = ctx
            .remaining_accounts
            .iter()
            .map(|acc| {
                let is_signer = acc.key == &ctx.accounts.vault.key();
                AccountMeta {
                    pubkey: *acc.key,
                    is_signer,
                    is_writable: acc.is_writable,
                }
            })
            .collect();
        
        //Convert the remaining accounts to account infos
        let accounts_infos: Vec<AccountInfo> = ctx
            .remaining_accounts
            .iter()
            .map(|acc| AccountInfo { ..acc.clone() })
            .collect();

        //PDA signer seeds for the vault 
        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, &[ctx.bumps.vault]]];

        //invoke the cpi call to jupiter program 
        invoke_signed(
            &Instruction {
                program_id: ctx.accounts.jupiter_program.key(),
                accounts,
                data,
            },
            &accounts_infos,
            signer_seeds,
        )?;
            msg!("Transferring tokens...");
            msg!(
                "Mint: {}",
                &ctx.accounts.output_mint.to_account_info().key()
            );
            msg!(
                "From Vault Output Token Account: {}",
                &ctx.accounts.vault_output_token_account.key()
            );
            msg!(
                "To Recipient Token Address: {}",
                &ctx.accounts.recipient_token_account.key()
            );
         msg!("Vault PDA: {}", ctx.accounts.vault.key());
         msg!("Vault Output Token Account Authority: {:?}", ctx.accounts.vault_output_token_account.owner);

        //Transfer swapped tokens to recipient 
        let decimals = ctx.accounts.output_mint.decimals;
        let cpi_accounts = TransferChecked { 
                mint:ctx.accounts.output_mint.to_account_info(),
                from: ctx.accounts.vault_output_token_account.to_account_info(),
                to: ctx.accounts.recipient_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_program =  ctx.accounts.output_mint_token_program.to_account_info();
        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        transfer_checked(cpi_context, amount, decimals)?;

        msg!("Tokens transferred successfully.");

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Swap<'info> {
    pub input_mint: InterfaceAccount<'info, Mint>,
    pub input_mint_token_program: Interface<'info, TokenInterface>,
    pub output_mint: InterfaceAccount<'info, Mint>,
    pub output_mint_token_program: Interface<'info, TokenInterface>,

    #[account(
      mut,
      seeds=[VAULT_SEED],
      bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(
      mut,
      associated_token::mint=input_mint,
      associated_token::authority=vault,
      associated_token::token_program=input_mint_token_program,
    )]
    pub vault_input_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
      mut,
      associated_token::mint=output_mint,
      associated_token::authority=vault,
      associated_token::token_program=output_mint_token_program,
    )]
    pub vault_output_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint=output_mint,
        associated_token::authority=recipient,
        associated_token::token_program=output_mint_token_program,
    )]
    pub recipient_token_account: InterfaceAccount<'info, TokenAccount>,
    pub recipient: SystemAccount<'info>, 
    pub jupiter_program: Program<'info, Jupiter>,
}
