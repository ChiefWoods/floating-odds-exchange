use quasar_lang::prelude::*;

#[event(discriminator = 0)]
pub struct MarketInitialized {
    pub market: Address,
    pub authority: Address,
    pub seed: u64,
    pub slot: u64,
}

#[event(discriminator = 1)]
pub struct MarketLaunched {
    pub market: Address,
    pub slot: u64,
}

#[event(discriminator = 2)]
pub struct BetCreated {
    pub buyer: Address,
    pub market: Address,
    pub in_amount: u64,
    pub out_amount: u64,
    pub slot: u64,
}

#[event(discriminator = 3)]
pub struct WinningsClaimed {
    pub claimer: Address,
    pub market: Address,
    pub amount: u64,
    pub slot: u64,
}

#[event(discriminator = 4)]
pub struct MarketResolved {
    pub market: Address,
    pub slot: u64,
    // memcpy bug
    // pub outcome: u8,
}

#[event(discriminator = 5)]
pub struct MarketPaused {
    pub market: Address,
    pub slot: u64,
}
