
## Background
This repo showcases how to use LiteSVM to simulate interacting with a mainnet program, in this example we are interacting with JUPV6 aggregator where we want to swap some wSol for USDC

LiteSVM creates 
an in process solana svm - this is important to know because this means its internal state is different from mainnet state, so we have to set all the relevant accounts eg. mint account, mainnet program account that we need locally using the `set_account` method, but first, we open up a terminal and clone the accounts with the `solana account <address>` command, then you can use the `get_accounts_field` helper method from `src/helpers` to parse the downloaded accounts to access the fields , or you can just copy it directly from the cloned accounts into the account struct.


ATA creation is not possible in this context (unless we set the ATA program locally) so we use the `TokenAccount` struct from `litesvm-token` to create a token account for our pubkey, then set any arbitary amount of any mint we want, you can see an example in the LiteSVM docs and in the `main.rs` file in this repo.


## Running the code
I've set most of the fields for the relevant accounts manually, we only need the `Jupiter V6 Program Data account` because its quite large (3.67mb) as it contains the data for the executable program for Jup v6 aggregator itself.

So first we clone the program data account for the jupiter v6 program with this command.

 `solana account 4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT --output json-compact --url https://api.mainnet-beta.solana.com > JUP_V6_PROGRAM_DATA_ACCOUNT.json`

 Then go the `main.rs` on line `235` and make sure you specify the correct path to the file that is.

 ```rust
 let jup_v6_program_data_bytes = get_account_fields("../JUP_V6_PROGRAM_DATA_ACCOUNT.json")
                                    .unwrap()
                                    .account
                                    .data
                                    .0;;
 ```

 then run it 

```rust 
 cargo run
```


## Cloning other accounts
To clone the other accounts locally (not needed because i've manually set the data for the rest).

### Clone wSOL mint account
`solana account So11111111111111111111111111111111111111112 --output json-compact --url https://api.mainnet-beta.solana.com > So1111111111_mainnet.json `

### Clone USDC mint account 
`solana account EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v --output json-compact --url https://api.mainnet-beta.solana.com > USDC_mainnet.json`

### Clone Jupiter V6 aggregator program
`solana account JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 --output json-compact --url https://api.mainnet-beta.solana.com > JUP_V6_mainnet.json`


## Error 

Currently i get an error for the jup program, but the cpi call to the jup v6 aggregator works, i will take a deeper look into it.

```
transaction failure FailedTransactionMetadata {
  err: InstructionError(0, ProgramFailedToComplete),
  meta: TransactionMetadata {
    signature: zSgVLT8PL6gPX1shJrY5h4b6VWNyXDJB4nJYbB4ZrVwzHjBtR7yzp3K39CbBLfVrxvYBSUhwMqQkaRm24x8NH7U,
    logs: [
      "Program LMMGrBSX84ZC519PSBkppyVdT4XfM3VP3hw4XLXqhrf invoke [1]",
      "Program log: Instruction: Swap",
      "Program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 invoke [2]",
      "Program log: Instruction: SharedAccountsRoute",
      "Program log: panicked at programs/nostd-token/src/accessor.rs:23:39:\nrange end index 64 out of range for slice of length 0",
      "Program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 consumed 4475 of 136506 compute units",
      "Program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 failed: SBF program panicked",
      "Program LMMGrBSX84ZC519PSBkppyVdT4XfM3VP3hw4XLXqhrf consumed 67969 of 200000 compute units",
      "Program LMMGrBSX84ZC519PSBkppyVdT4XfM3VP3hw4XLXqhrf failed: Program failed to complete"
    ],
    inner_instructions: [
      [
        InnerInstruction {
          instruction: CompiledInstruction {
            program_id_index: 20,
            accounts: [
              23, 28, 0, 3, 10, 16, 2, 22, 33, 20, 20, 31, 20,
              25, 27, 15, 28, 10, 4, 7, 13, 1, 14, 23, 32, 32,
              26, 24, 9, 34, 29, 11, 5, 16, 4, 19, 19, 19, 28, 23
            ],
            data: [
              193, 32, 155, 51, 65, 214, 156, 129, 7, 2, 0, 0, 0,
              25, 100, 0, 1, 58, 0, 100, 1, 2, 128, 132, 30, 0, 0,
              0, 0, 0, 40, 73, 3, 0, 0, 0, 0, 0, 50, 0, 0
            ]
          },
          stack_height: 2
        }
      ]
    ],
    compute_units_consumed: 67969,
    return_data: TransactionReturnData {
      program_id: JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4,
      data: []
    }
  }
}

```