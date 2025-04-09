
## Background

This repo showcases how to use Litesvm to simulate working with a mainnet program, in this case we are using JUPV6 aggregator, litesvm creates
an in process solana svm - this is important because this means it is different from mainnet state, so we have to add every mint account, mainnet program 
that we need locally using the `set_account` method, ATA creation is not possible (unless we set the ATA prgram locally) but instead of doing that we can just
use the `Token_account` struct and set any arbitray amount of any mint we want for our pubkey 

i cloned the accounts using the `solana account <account-adress>` method, then i copied the relevant fields into my struct, for the program data account which actually owns the executable jupv6 aggregator program, i had to remove the data from the account i downloaded and read it from a separate txt file, but i'm going to refactor it, so that i can just use serde directly on the account to get the fields i need - more like a helper i can create and eventually add to `lite-svm`.

Currently i get an error for the jup program, but this is not one litesvm related , i will take a deeper look into it.