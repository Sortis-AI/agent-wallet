use thiserror::Error;

#[derive(Error, Debug)]
pub enum AwError {
    #[error("HTTP error: {status} {url}")]
    Http {
        status: u16,
        url: String,
        body: String,
    },

    #[error("price exceeds budget: requested {requested} {currency}, budget {budget}")]
    PriceExceeded {
        requested: f64,
        budget: f64,
        currency: String,
    },

    #[error("insufficient funds: need {needed} {currency}, have {available}")]
    InsufficientFunds {
        needed: f64,
        available: f64,
        currency: String,
    },

    #[error("config error: {0}")]
    Config(String),

    #[error("wallet error: {0}")]
    Wallet(String),

    #[error("payment error: {0}")]
    Payment(String),

    #[error("MPP protocol error: {0}")]
    Mpp(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AwError {
    pub fn exit_code(&self) -> i32 {
        match self {
            AwError::Http { .. } => 1,
            AwError::PriceExceeded { .. } => 2,
            AwError::InsufficientFunds { .. } => 3,
            AwError::Config(_) | AwError::Wallet(_) => 4,
            AwError::Payment(_) | AwError::Mpp(_) | AwError::Other(_) => 1,
        }
    }
}
