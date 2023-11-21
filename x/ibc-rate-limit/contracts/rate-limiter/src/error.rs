use cosmwasm_std::{StdError, Timestamp, Uint256};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("IBC Rate Limit exceeded for {channel}/{denom}. Tried to transfer {amount} which exceeds capacity on the '{quota_name}' quota ({used}/{max}). Try again after {reset:?}")]
    RateLimitExceded {
        channel: String,
        denom: String,
        amount: Uint256,
        quota_name: String,
        used: Uint256,
        max: Uint256,
        reset: Timestamp,
    },

    #[error("Quota {quota_id} not found for channel {channel_id}")]
    QuotaNotFound {
        quota_id: String,
        channel_id: String,
        denom: String,
    },
    #[error("more tokens than allowed attempted to be transferred")]
    InsufficientBypassAllowance,
    #[error("only one intent per (sender, channel_id, denom) may be present")]
    IntentAlreadyPresent,
    #[error("no intent matching (sender, channel_id, denom) is present")]
    IntentNotPresent,
    #[error("the amount of is less than the threshold of {0}")]
    InsufficientBypassAmount(u8),
}
