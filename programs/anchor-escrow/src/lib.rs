use anchor_lang::prelude::*;
pub mod error;
pub mod instructions;
pub use instructions::*;
pub mod state;

declare_id!("BuQZzvsCrEbN2TDPs9LovXq95jdX64knK27etKR98s6V");

// #[program]
// pub mod anchor_escrow {
//     use super::*;

//     pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
//         crate::instructions::initialize::handle_initialize(ctx)
//     }

//     pub fn increment(ctx: Context<Increment>) -> Result<()> {
//         crate::instructions::increment::handle_increment(ctx)
//     }
// }


#[program]
pub mod anchor_escrow {
    use super::*;

    pub fn make(ctx: Context<Make>,seed: u64, receive: u64, amount: u64) -> Result<()>{
        instructions::make::handler(ctx, seed, receive, amount)
    }
    pub fn take(ctx: Context<Take>) -> Result<()>{
        instructions::take::handler(ctx)
    }
    pub fn refund(){}
}