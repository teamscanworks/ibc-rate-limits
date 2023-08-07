use cosmwasm_std::{Addr, Timestamp, Uint256};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::cmp;

use cw_storage_plus::{Item, Map};

use crate::{msg::QuotaMsg, ContractError};

/// This represents the key for our rate limiting tracker. A tuple of a denom and
/// a channel. When interactic with storage, it's preffered to use this struct
/// and call path.into() on it to convert it to the composite key of the
/// RATE_LIMIT_TRACKERS map
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Path {
    pub denom: String,
    pub channel: String,
}

impl Path {
    pub fn new(channel: impl Into<String>, denom: impl Into<String>) -> Self {
        Path {
            channel: channel.into(),
            denom: denom.into(),
        }
    }
}

impl From<Path> for (String, String) {
    fn from(path: Path) -> (String, String) {
        (path.channel, path.denom)
    }
}

impl From<&Path> for (String, String) {
    fn from(path: &Path) -> (String, String) {
        (path.channel.to_owned(), path.denom.to_owned())
    }
}

#[derive(Debug, Clone)]
pub enum FlowType {
    In,
    Out,
}

/// A Flow represents the transfer of value for a denom through an IBC channel
/// during a time window.
///
/// It tracks inflows (transfers into osmosis) and outflows (transfers out of
/// osmosis).
///
/// The period_end represents the last point in time for which this Flow is
/// tracking the value transfer.
///
/// Periods are discrete repeating windows. A period only starts when a contract
/// call to update the Flow (SendPacket/RecvPackt) is made, and not right after
/// the period ends. This means that if no calls happen after a period expires,
/// the next period will begin at the time of the next call and be valid for the
/// specified duration for the quota.
///
/// This is a design decision to avoid the period calculations and thus reduce gas consumption
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, Copy)]
pub struct Flow {
    pub inflow: Uint256,
    pub outflow: Uint256,
    pub period_end: Timestamp,
}

impl Flow {
    pub fn new(
        inflow: impl Into<Uint256>,
        outflow: impl Into<Uint256>,
        now: Timestamp,
        duration: u64,
    ) -> Self {
        Self {
            inflow: inflow.into(),
            outflow: outflow.into(),
            period_end: now.plus_seconds(duration),
        }
    }

    /// The balance of a flow is how much absolute value for the denom has moved
    /// through the channel before period_end. It returns a tuple of
    /// (balance_in, balance_out) where balance_in in is how much has been
    /// transferred into the flow, and balance_out is how much value transferred
    /// out.
    pub fn balance(&self) -> (Uint256, Uint256) {
        (
            self.inflow.saturating_sub(self.outflow),
            self.outflow.saturating_sub(self.inflow),
        )
    }

    /// checks if the flow, in the current state, has exceeded a max allowance
    pub fn exceeds(&self, direction: &FlowType, max_inflow: Uint256, max_outflow: Uint256) -> bool {
        let (balance_in, balance_out) = self.balance();
        match direction {
            FlowType::In => balance_in > max_inflow,
            FlowType::Out => balance_out > max_outflow,
        }
    }

    /// returns the balance in a direction. This is used for displaying cleaner errors
    pub fn balance_on(&self, direction: &FlowType) -> Uint256 {
        let (balance_in, balance_out) = self.balance();
        match direction {
            FlowType::In => balance_in,
            FlowType::Out => balance_out,
        }
    }

    /// If now is greater than the period_end, the Flow is considered expired.
    pub fn is_expired(&self, now: Timestamp) -> bool {
        self.period_end < now
    }

    // Mutating methods

    /// Expire resets the Flow to start tracking the value transfer from the
    /// moment this method is called.
    pub fn expire(&mut self, now: Timestamp, duration: u64) {
        self.inflow = Uint256::from(0_u32);
        self.outflow = Uint256::from(0_u32);
        self.period_end = now.plus_seconds(duration);
    }

    /// Updates the current flow incrementing it by a transfer of value.
    pub fn add_flow(&mut self, direction: FlowType, value: Uint256) {
        match direction {
            FlowType::In => self.inflow = self.inflow.saturating_add(value),
            FlowType::Out => self.outflow = self.outflow.saturating_add(value),
        }
    }

    /// Updates the current flow reducing it by a transfer of value.
    pub fn undo_flow(&mut self, direction: FlowType, value: Uint256) {
        match direction {
            FlowType::In => self.inflow = self.inflow.saturating_sub(value),
            FlowType::Out => self.outflow = self.outflow.saturating_sub(value),
        }
    }

    /// Applies a transfer. when QuotaV2 is used, flows are not reset as that is handled during beforeBlock stages
    fn apply_transfer(
        &mut self,
        direction: &FlowType,
        funds: Uint256,
        now: Timestamp,
        quota: &QuotaType,
    ) -> bool {
        match quota {
            QuotaType::V1(quota) => {
                let mut expired = false;
                if self.is_expired(now) {
                    self.expire(now, quota.duration);
                    expired = true;
                }
                self.add_flow(direction.clone(), funds);
                expired
            }
            QuotaType::V2(..) => {
                self.add_flow(direction.clone(), funds);
                false
            }
        }
    }
}

/// A Quota is the percentage of the denom's total value that can be transferred
/// through the channel in a given period of time (duration)
///
/// Percentages can be different for send and recv
///
/// The name of the quota is expected to be a human-readable representation of
/// the duration (i.e.: "weekly", "daily", "every-six-months", ...)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Quota {
    pub name: String,
    pub max_percentage_send: u32,
    pub max_percentage_recv: u32,
    pub duration: u64,
    pub channel_value: Option<Uint256>,
}
/// A Quota is the percentage of the denom's total value that can be transferred
/// through the channel in a given period of time (duration)
///
/// Percentages can be different for send and recv
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QuotaV2 {
    pub name: String,
    pub max_percentage_send: u32,
    pub max_percentage_recv: u32,
    pub channel_value: Option<Uint256>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum QuotaType {
    V1(Quota),
    V2(QuotaV2),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum RateLimitType {
    V1 { quota: QuotaType, flow: Flow },
    V2 { quota: QuotaType, flow: Flow },
}
impl QuotaType {
    pub fn max_percentage_send(&self) -> u32 {
        match self {
            Self::V1(quota) => quota.max_percentage_send,
            Self::V2(quota) => quota.max_percentage_send
        }
    }
    pub fn max_percentage_recv(&self) -> u32 {
        match self {
            Self::V1(quota) => quota.max_percentage_recv,
            Self::V2(quota) => quota.max_percentage_recv
        }
    }
    pub fn name(&self) -> String {
        match self {
            Self::V1(quota) => quota.name.clone(),
            Self::V2(quota) => quota.name.clone(),
        }
    }
    pub fn capacity(&self) -> (Uint256, Uint256) {
        match self {
            QuotaType::V1(quota) => quota.capacity(),
            QuotaType::V2(quota) => quota.capacity(),
        }
    }
    pub fn capacity_on(&self, direction: &FlowType) -> Uint256 {
        match self {
            QuotaType::V1(quota) => quota.capacity_on(direction),
            QuotaType::V2(quota) => quota.capacity_on(direction),
        }
    }
    pub fn duration(self) -> u64 {
        match self {
            Self::V1(quota) => quota.duration,
            Self::V2(quota) => 0,
        }
    }
    pub fn channel_value(&self) -> Option<Uint256> {
        match self {
            Self::V1(quota) => quota.channel_value,
            Self::V2(quota) => quota.channel_value
        }
    }
}
impl Quota {
    /// Calculates the max capacity (absolute value in the same unit as
    /// total_value) in each direction based on the total value of the denom in
    /// the channel. The result tuple represents the max capacity when the
    /// transfer is in directions: (FlowType::In, FlowType::Out)
    pub fn capacity(&self) -> (Uint256, Uint256) {
        match self.channel_value {
            Some(total_value) => (
                total_value * Uint256::from(self.max_percentage_recv) / Uint256::from(100_u32),
                total_value * Uint256::from(self.max_percentage_send) / Uint256::from(100_u32),
            ),
            None => (0_u32.into(), 0_u32.into()), // This should never happen, but ig the channel value is not set, we disallow any transfer
        }
    }

    /// returns the capacity in a direction. This is used for displaying cleaner errors
    pub fn capacity_on(&self, direction: &FlowType) -> Uint256 {
        let (max_in, max_out) = self.capacity();
        match direction {
            FlowType::In => max_in,
            FlowType::Out => max_out,
        }
    }
}

impl QuotaV2 {
    /// Calculates the max capacity (absolute value in the same unit as
    /// total_value) in each direction based on the total value of the denom in
    /// the channel. The result tuple represents the max capacity when the
    /// transfer is in directions: (FlowType::In, FlowType::Out)
    pub fn capacity(&self) -> (Uint256, Uint256) {
        match self.channel_value {
            Some(total_value) => (
                total_value * Uint256::from(self.max_percentage_recv) / Uint256::from(100_u32),
                total_value * Uint256::from(self.max_percentage_send) / Uint256::from(100_u32),
            ),
            None => (0_u32.into(), 0_u32.into()), // This should never happen, but ig the channel value is not set, we disallow any transfer
        }
    }

    /// returns the capacity in a direction. This is used for displaying cleaner errors
    pub fn capacity_on(&self, direction: &FlowType) -> Uint256 {
        let (max_in, max_out) = self.capacity();
        match direction {
            FlowType::In => max_in,
            FlowType::Out => max_out,
        }
    }
}

impl From<&QuotaMsg> for Quota {
    fn from(msg: &QuotaMsg) -> Self {
        let send_recv = (
            cmp::min(msg.send_recv.0, 100),
            cmp::min(msg.send_recv.1, 100),
        );
        Quota {
            name: msg.name.clone(),
            max_percentage_send: send_recv.0,
            max_percentage_recv: send_recv.1,
            duration: msg.duration,
            channel_value: None,
        }
    }
}
impl From<&QuotaMsg> for QuotaV2 {
    fn from(msg: &QuotaMsg) -> Self {
        let send_recv = (
            cmp::min(msg.send_recv.0, 100),
            cmp::min(msg.send_recv.1, 100),
        );
        QuotaV2 {
            name: msg.name.clone(),
            max_percentage_send: send_recv.0,
            max_percentage_recv: send_recv.1,
            channel_value: None,
        }
    }
}

impl From<RateLimit> for RateLimitType {
    fn from(value: RateLimit) -> Self {
        Self::V1 { quota: QuotaType::V1(value.quota), flow: value.flow }
    }
}

impl From<RateLimitV2> for RateLimitType {
    fn from(value: RateLimitV2) -> Self {
        Self::V2 { quota: QuotaType::V2(value.quota), flow: value.flow}
    }
}


/// RateLimit is the main structure tracked for each channel/denom pair. Its quota
/// represents rate limit configuration, and the flow its
/// current state (i.e.: how much value has been transfered in the current period)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RateLimit {
    pub quota: Quota,
    pub flow: Flow,
}
/// RateLimit is the main structure tracked for each channel/denom pair. Its quota
/// represents rate limit configuration, and the flow its
/// current state (i.e.: how much value has been transfered in the current period)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RateLimitV2 {
    pub quota: QuotaV2,
    pub flow: Flow,
}

// The channel value on send depends on the amount on escrow. The ibc transfer
// module modifies the escrow amount by "funds" on sends before calling the
// contract. This function takes that into account so that the channel value
// that we track matches the channel value at the moment when the ibc
// transaction started executing
fn calculate_channel_value(
    channel_value: Uint256,
    denom: &str,
    funds: Uint256,
    direction: &FlowType,
) -> Uint256 {
    match direction {
        FlowType::Out => {
            if denom.contains("ibc") {
                channel_value + funds // Non-Native tokens get removed from the supply on send. Add that amount back
            } else {
                // The commented-out code in the golang calculate channel value is what we want, but we're currently using the whole supply temporarily for efficiency. see rate_limit.go/CalculateChannelValue(..)
                //channel_value - funds // Native tokens increase escrow amount on send. Remove that amount here
                channel_value
            }
        }
        FlowType::In => channel_value,
    }
}

impl RateLimitType {
    pub fn undo_flow(&mut self, direction: FlowType, funds: Uint256) {
        let flow = match self {
            Self::V1 { quota: _, flow } => flow,
            Self::V2 { quota: _, flow} => flow
        };
        flow.undo_flow(direction, funds)
    }
    pub fn flow_period_end(&self) -> Timestamp {
        match self {
            RateLimitType::V1 { quota: _, flow } => {
                flow.period_end
            }
            RateLimitType::V2 { quota: _, flow } => {
                flow.period_end
            }
        }
    }
    pub fn flow_balance(&self) -> (Uint256, Uint256) {
        match self {
            RateLimitType::V1 { quota, flow } => {
                flow.balance()
            }
            RateLimitType::V2 { quota, flow } => {
                flow.balance()
            }
        }
    }
    pub fn quota_capacity(&self) -> (Uint256, Uint256) {
        match self {
            RateLimitType::V1 { quota, flow } => {
                quota.capacity()
            }
            RateLimitType::V2 { quota, flow } => {
                quota.capacity()
            }
        }
    }
    pub fn channel_value(&self) -> Option<Uint256> {
        match self {
            RateLimitType::V1 { quota, flow } => quota.channel_value(),
            RateLimitType::V2 { quota, flow } => quota.channel_value(),
        }
    }
    pub fn quota_name(&self) ->  String  {
        match self {
            RateLimitType::V1 { quota, flow } =>quota.name(),
            RateLimitType::V2 { quota, flow } => quota.name(),
        }
    }
    pub fn period_end(&self) ->  String  {
        match self {
            RateLimitType::V1 { quota, flow } =>quota.name(),
            RateLimitType::V2 { quota, flow } => quota.name(),
        }
    }
    pub fn expire_flow(&mut self, now: Timestamp) {
        match self {
            RateLimitType::V1 { quota, flow } =>flow.expire(now, quota.clone().duration()),
            RateLimitType::V2 { quota, flow } => flow.expire(now, quota.clone().duration()),
        }
    }
    pub fn flow(&self) -> Flow {
        match self {
            Self::V1 { quota, flow } => flow.clone(),
            Self::V2 { quota, flow } => flow.clone(),
        }
    }
    pub fn quota_type(&self) -> QuotaType {
        match self { 
            Self::V1 { quota, flow: _} => quota.clone(),
            Self::V2{quota, flow: _} => quota.clone()
        }
    }
    /// Checks if a transfer is allowed and updates the data structures
    /// accordingly.
    ///
    /// If the transfer is not allowed, it will return a RateLimitExceeded error.
    ///
    /// Otherwise it will return a RateLimitResponse with the updated data structures
    pub fn allow_transfer(
        &mut self,
        path: &Path,
        direction: &FlowType,
        funds: Uint256,
        channel_value: Uint256,
        now: Timestamp,
    ) -> Result<Self, ContractError> {
        let (quota, flow, is_v2) = match self {
            RateLimitType::V1 { quota, flow} => (quota, flow, false),
            RateLimitType::V2 { quota, flow} => (quota, flow, true)
        };
        // Flow used before this transaction is applied.
        // This is used to make error messages more informative
        let initial_flow = flow.balance_on(direction);
        // Apply the transfer. From here on, we will updated the flow with the new transfer
        // and check if  it exceeds the quota at the current time
        let expired = flow.apply_transfer(direction, funds, now, &quota);
        let (max_in, max_out) = match quota {
            QuotaType::V1(quota) => {
                if quota.channel_value.is_none() || expired {
                    quota.channel_value = Some(calculate_channel_value(channel_value, &path.denom, funds, direction));
                }
                quota.capacity()
            }
            QuotaType::V2(quota) => {
                if quota.channel_value.is_none() || expired {
                    quota.channel_value = Some(calculate_channel_value(channel_value, &path.denom, funds, direction));
                }
                quota.capacity()
            }
        };
        if flow.exceeds(direction, max_in, max_out) {
            return Err(ContractError::RateLimitExceded {
                channel: path.channel.to_string(),
                denom: path.denom.to_string(),
                amount: funds,
                quota_name: quota.name(),
                used: initial_flow,
                max: quota.capacity_on(direction),
                reset: flow.period_end,
            });
        } else {
            if is_v2 {
                Ok(RateLimitType::V2 {
                    quota: quota.clone(),
                    flow: *flow,
                })
            } else {
                Ok(RateLimitType::V1 {
                    quota: quota.clone(),
                    flow: *flow,
                })
            }
        }
    }
}

/// Only this address can manage the contract. This will likely be the
/// governance module, but could be set to something else if needed
pub const GOVMODULE: Item<Addr> = Item::new("gov_module");
/// Only this address can execute transfers. This will likely be the
/// IBC transfer module, but could be set to something else if needed
pub const IBCMODULE: Item<Addr> = Item::new("ibc_module");

/// RATE_LIMIT_TRACKERS is the main state for this contract. It maps a path (IBC
/// Channel + denom) to a vector of `RateLimit`s.
///
/// The `RateLimit` struct contains the information about how much value of a
/// denom has moved through the channel during the currently active time period
/// (channel_flow.flow) and what percentage of the denom's value we are
/// allowing to flow through that channel in a specific duration (quota)
///
/// For simplicity, the channel in the map keys refers to the "host" channel on
/// the osmosis side. This means that on PacketSend it will refer to the source
/// channel while on PacketRecv it refers to the destination channel.
///
/// It is the responsibility of the go module to pass the appropriate channel
/// when sending the messages
///
/// The map key (String, String) represents (channel_id, denom). We use
/// composite keys instead of a struct to avoid having to implement the
/// PrimaryKey trait
pub const RATE_LIMIT_TRACKERS_LEGACY: Map<(String, String), Vec<RateLimit>> = Map::new("flow");

/// Similiar to RATE_LIMIT_TRACKERS_LEGACY however it is intended to be used after v1 cosmwasm migrations are performed
pub const RATE_LIMIT_TRACKERS: Map<(String, String), Vec<RateLimitType>> = Map::new("flow");

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const RESET_TIME_DAILY: u64 = 60 * 60 * 24;
    pub const RESET_TIME_WEEKLY: u64 = 60 * 60 * 24 * 7;
    pub const RESET_TIME_MONTHLY: u64 = 60 * 60 * 24 * 30;

    #[test]
    fn flow() {
        let epoch = Timestamp::from_seconds(0);
        let mut flow = Flow::new(0_u32, 0_u32, epoch, RESET_TIME_WEEKLY);

        assert!(!flow.is_expired(epoch));
        assert!(!flow.is_expired(epoch.plus_seconds(RESET_TIME_DAILY)));
        assert!(!flow.is_expired(epoch.plus_seconds(RESET_TIME_WEEKLY)));
        assert!(flow.is_expired(epoch.plus_seconds(RESET_TIME_WEEKLY).plus_nanos(1)));

        assert_eq!(flow.balance(), (0_u32.into(), 0_u32.into()));
        flow.add_flow(FlowType::In, 5_u32.into());
        assert_eq!(flow.balance(), (5_u32.into(), 0_u32.into()));
        flow.add_flow(FlowType::Out, 2_u32.into());
        assert_eq!(flow.balance(), (3_u32.into(), 0_u32.into()));
        // Adding flow doesn't affect expiration
        assert!(!flow.is_expired(epoch.plus_seconds(RESET_TIME_DAILY)));

        flow.expire(epoch.plus_seconds(RESET_TIME_WEEKLY), RESET_TIME_WEEKLY);
        assert_eq!(flow.balance(), (0_u32.into(), 0_u32.into()));
        assert_eq!(flow.inflow, Uint256::from(0_u32));
        assert_eq!(flow.outflow, Uint256::from(0_u32));
        assert_eq!(flow.period_end, epoch.plus_seconds(RESET_TIME_WEEKLY * 2));

        // Expiration has moved
        assert!(!flow.is_expired(epoch.plus_seconds(RESET_TIME_WEEKLY).plus_nanos(1)));
        assert!(!flow.is_expired(epoch.plus_seconds(RESET_TIME_WEEKLY * 2)));
        assert!(flow.is_expired(epoch.plus_seconds(RESET_TIME_WEEKLY * 2).plus_nanos(1)));
    }
}
