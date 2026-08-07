use quasar_lang::prelude::*;

#[error_code]
pub enum FloatingOddsExchangeError {
    /// Outcome must be from range 0 - 3
    InvalidOutcome,
    /// Mint metadata fields cannot be empty
    InvalidMintMetadata,
    /// Market has not been launched
    MarketNotLaunched,
    /// Market cannot be launched again
    MarketAlreadyLaunched,
    /// Market has already been resolved
    MarketAlreadyResolved,
    /// Market is paused
    MarketPaused,
    /// Market has not been resolved
    MarketNotResolved,
    /// Market can only be resolved as 'Yes', 'No', or 'Refunded'
    MarketIsAlreadyUndecided,
    /// Market authority does not match
    UnauthorizedAuthority,
    /// Pot mint must not be freezable
    PotMintFreezable,
    /// Buy mint does not match
    InvalidMint,
    /// Token account not initialized
    TokenAccountNotInitialized,
    /// Amount cannot be zero or negative
    InvalidAmount,
    /// Slippage exceeded
    SlippageExceeded,
    /// Out amount cannot be zero
    InsufficientLiquidity,
    /// Invalid metadata
    InvalidMetadata,
    /// Cannot convert to UTF-8 string
    InvalidUtf8String,
    /// Arithmetic overflow
    ArithmeticOverflow,
}
