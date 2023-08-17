use cosmwasm_std::{Addr, Timestamp, Uint256, DepsMut, Env};
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
    pub fn _balance(&self) -> (Uint256, Uint256) {
        (
            self.inflow.saturating_sub(self.outflow),
            self.outflow.saturating_sub(self.inflow),
        )
    }

    /// checks if the flow, in the current state, has exceeded a max allowance
    pub fn _exceeds(&self, direction: &FlowType, max_inflow: Uint256, max_outflow: Uint256) -> bool {
        let (balance_in, balance_out) = self._balance();
        match direction {
            FlowType::In => balance_in > max_inflow,
            FlowType::Out => balance_out > max_outflow,
        }
    }

    /// returns the balance in a direction. This is used for displaying cleaner errors
    pub fn _balance_on(&self, direction: &FlowType) -> Uint256 {
        let (balance_in, balance_out) = self._balance();
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
    pub fn _expire(&mut self, now: Timestamp, duration: u64) {
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

    /// Applies a transfer. If the Flow is expired (now > period_end), it will
    /// reset it before applying the transfer.
    fn apply_transfer(
        &mut self,
        direction: &FlowType,
        funds: Uint256,
        now: Timestamp,
        quota: &Quota,
        v1_migrated: bool
    ) {
        if v1_migrated && self.is_expired(now) {
            self._expire(now, quota.duration);
        }
        //let mut expired = false;
        //if self.is_expired(now) {
        //    self.expire(now, quota.duration);
        //    expired = true;
        //}
        self.add_flow(direction.clone(), funds);
        //expired
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

impl Quota {
    /// Calculates the max capacity (absolute value in the same unit as
    /// total_value) in each direction based on the total value of the denom in
    /// the channel. The result tuple represents the max capacity when the
    /// transfer is in directions: (FlowType::In, FlowType::Out)
    pub fn _capacity(&self) -> (Uint256, Uint256) {
        match self.channel_value {
            Some(total_value) => (
                total_value * Uint256::from(self.max_percentage_recv) / Uint256::from(100_u32),
                total_value * Uint256::from(self.max_percentage_send) / Uint256::from(100_u32),
            ),
            None => (0_u32.into(), 0_u32.into()), // This should never happen, but ig the channel value is not set, we disallow any transfer
        }
    }

    /// returns the capacity in a direction. This is used for displaying cleaner errors
    pub fn _capacity_on(&self, direction: &FlowType) -> Uint256 {
        let (max_in, max_out) = self._capacity();
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

/// RateLimit is the main structure tracked for each channel/denom pair. Its quota
/// represents rate limit configuration, and the flow its
/// current state (i.e.: how much value has been transfered in the current period)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RateLimit {
    pub quota: Quota,
    pub flow: Flow,
    // not very storage efficient, can probably 
    // remove storing the decayed value for all 3 previous_X types
    // and calculate that at run time
    //
    // alternatively can only track previous_inflow and previous_outflow
    pub previous_channel_value: Option<Uint256>,
    pub previous_inflow: Option<Uint256>,
    pub previous_outflow: Option<Uint256>,
    pub decayed_last_updated: Option<u64>,
    pub decayed_value: Option<cosmwasm_std::Decimal256>,
    pub decayed_infow: Option<cosmwasm_std::Decimal256>,
    pub decayed_outflow: Option<cosmwasm_std::Decimal256>,
    pub period_start: Option<Timestamp>,
    // when set to true, enables averaged_X calculations, this is done
    // to preserve backwards compatability with pre-existing rate limits
    // and behavior expectations
    pub v1_migration: Option<bool>,
}

// The channel value on send depends on the amount on escrow. The ibc transfer
// module modifies the escrow amount by "funds" on sends before calling the
// contract. This function takes that into account so that the channel value
// that we track matches the channel value at the moment when the ibc
// transaction started executing
pub(crate) fn calculate_channel_value(
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

impl RateLimit {
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
        // Flow used before this transaction is applied.
        // This is used to make error messages more informative
        let initial_flow = self.averaged_balance_on(direction).unwrap();

        // Apply the transfer. From here on, we will updated the flow with the new transfer
        // and check if  it exceeds the quota at the current time

        self.flow.apply_transfer(direction, funds, now, &self.quota, self.v1_migration.unwrap_or(false));
        // Cache the channel value if it has never been set or it has expired.
        //
        // NOTE: due to the way rollover is currently working, this
        // may be None / expired for backwards compatability, or it may be Some(0)
        if self.quota.channel_value.unwrap_or(Uint256::zero()).is_zero() {
            self.quota.channel_value = Some(calculate_channel_value(
                channel_value,
                &path.denom,
                funds,
                direction,
            ))
        }

        let (max_in, max_out) = self.averaged_capacity().unwrap();
        // Return the effects of applying the transfer or an error.
        match self.averaged_exceeds(direction, max_in, max_out) {
            true => Err(ContractError::RateLimitExceded {
                channel: path.channel.to_string(),
                denom: path.denom.to_string(),
                amount: funds,
                quota_name: self.quota.name.to_string(),
                used: initial_flow,
                max: self.averaged_capacity_on(direction).unwrap(),
                reset: self.flow.period_end,
            }),
            false => Ok(RateLimit {
                quota: self.quota.clone(), // Cloning here because self.quota.name (String) does not allow us to implement Copy
                flow: self.flow, // We can Copy flow, so this is slightly more efficient than cloning the whole RateLimit
                previous_channel_value: self.previous_channel_value,
                decayed_last_updated: self.decayed_last_updated,
                decayed_value: self.decayed_value,
                decayed_infow: self.decayed_infow,
                decayed_outflow: self.decayed_outflow,
                period_start: Some(now),
                previous_inflow: self.previous_inflow,
                previous_outflow: self.previous_outflow,
                v1_migration: self.v1_migration,
            }),
        }
    }
    pub fn is_v1_migrated(&self) -> bool {
        self.v1_migration.unwrap_or(false)
    }
    // executes business logic to handle period changes
    pub fn handle_rollover(&mut self, env: cosmwasm_std::Env) {
        self.flow._expire(env.block.time, self.quota.duration);
        self.period_start = Some(env.block.time.clone());
        self.previous_channel_value = self.quota.channel_value.clone();
        self.previous_inflow = Some(self.flow.inflow);
        self.previous_outflow = Some(self.flow.outflow);
    }
    // returns current channel value averaged against the previous channel value using a decaying function
    pub fn averaged_channel_value(&self) -> Option<cosmwasm_std::Decimal256> {
        if !self.is_v1_migrated() {
            return Some(cosmwasm_std::Decimal256::new(self.quota.channel_value?));
        }
        // when a rule is first initialized there is no previous period value, so there is nothing to average
        if self.previous_channel_value.unwrap_or(Uint256::zero()).is_zero() {
            return Some(cosmwasm_std::Decimal256::new(self.quota.channel_value?));
        }
        let decayed_channel_value = cosmwasm_std::Decimal256::new(self.previous_channel_value?).checked_sub(self.decayed_value?).ok()?;
        Some((cosmwasm_std::Decimal256::new(self.quota.channel_value?) + decayed_channel_value) / cosmwasm_std::Decimal256::from_atomics(2_u64, 0).ok()?)

    }
    // like Quota::capacity but using decaying average
    pub fn averaged_capacity(&self) -> Option<(Uint256, Uint256)> {
        if !self.is_v1_migrated() {
            return Some(self.quota._capacity());
        }
        let averaged_channel_value = self.averaged_channel_value()?;
        let averaged_channel_value: Uint256 = averaged_channel_value.atomics();
        Some((
            averaged_channel_value * Uint256::from(self.quota.max_percentage_recv) / Uint256::from(100_u32),
            averaged_channel_value * Uint256::from(self.quota.max_percentage_send) / Uint256::from(100_u32),
        ))
    }
    // like Quota::_capacity_on but using decaying average
    pub fn averaged_capacity_on(&self, direction: &FlowType) -> Option<Uint256> {
        if !self.is_v1_migrated() {
            return Some(self.quota._capacity_on(direction));
        }
        let (max_in, max_out) = self.averaged_capacity()?;
        match direction {
            FlowType::In => Some(max_in),
            FlowType::Out => Some(max_out),
        }
    }
    // like Flow::_balance but using decaying average
    pub fn averaged_balance(&self) -> Option<(Uint256, Uint256)> {
        if !self.is_v1_migrated() {
            return Some(self.flow._balance());
        }
        let decayed_inflow = self.decayed_infow.unwrap_or(cosmwasm_std::Decimal256::zero());
        let decayed_outflow = self.decayed_outflow.unwrap_or(cosmwasm_std::Decimal256::zero());

        if decayed_inflow.is_zero() || decayed_outflow.is_zero() {
            return Some(self.flow._balance());
        }
        let two = cosmwasm_std::Decimal256::one() + cosmwasm_std::Decimal256::one();
        let averaged_inflow = ((decayed_inflow + cosmwasm_std::Decimal256::new(self.flow.inflow)) / (two)).atomics();
        let averaged_outflow = ((decayed_outflow + cosmwasm_std::Decimal256::new(self.flow.outflow)) / (two)).atomics();
        Some((
            averaged_inflow.saturating_sub(averaged_outflow),
            averaged_outflow.saturating_sub(averaged_inflow)
        ))
    }
    pub fn averaged_balance_on(&self, direction: &FlowType) -> Option<Uint256> {
        if !self.is_v1_migrated() {
            return Some(self.flow._balance_on(direction));
        }
        let (balance_in, balance_out) = if let Some((b_in, b_out)) = self.averaged_balance() {
            (b_in, b_out)
        } else {
            return Some(self.flow._balance_on(direction));
        };
        match direction {
            FlowType::In => Some(balance_in),
            FlowType::Out => Some(balance_out)
        }
    }
    pub fn averaged_exceeds(&self, direction: &FlowType, max_inflow: Uint256, max_outflow: Uint256) -> bool {
        if !self.is_v1_migrated() {
            return self.flow._exceeds(direction, max_inflow, max_outflow);
        }
        let (balance_in, balance_out) = if let Some((b_in, b_out)) = self.averaged_balance() {
            (b_in, b_out)
        } else {
            return self.flow._exceeds(direction, max_inflow, max_outflow);
        };
        match direction {
            FlowType::In => balance_in > max_inflow,
            FlowType::Out => balance_out > max_outflow,
        }
    }
    // returns the amount of time that has passed in the given time period, based on the current timestamp recorded in the block
    // this transaction is executing in
    pub fn period_percent_passed(&self, block_time_second: u64) -> Option<cosmwasm_std::Decimal256> {
        // todo: measure the gas costs of calling `self.period_start.seconds()` twice vs storing the result of the function call in memory as is done now
        let period_start_seconds = self.period_start?.seconds();
        return Some(cosmwasm_std::Decimal256::percent(((block_time_second - period_start_seconds) * 100) / (self.flow.period_end.seconds() - period_start_seconds)));

        //return Some(cosmwasm_std::Decimal256::percent(((block_time_second - period_start_seconds) * 100) / (period_end_seconds - period_start_seconds)));
    }
    // used to perform a decay value update when needed, returning the newly decayed value if updated, otherwise
    // returning hte existing decayed value.
    //
    // the decayed value is subtracted from the previous channel value, as the decayed value represents the amou
    // checks if a decay operation should be applied to the value from the previous time period
    // returning the existing decayed value if there is no difference in block height or timestamp
    pub fn check_decay_rate(&mut self, env: cosmwasm_std::Env) -> Option<cosmwasm_std::Decimal256> {
        //#[cfg(test)]
        //println!("self {self:#?}");

        if self.decayed_last_updated? == env.block.height {
          #[cfg(test)]
          println!("decayed_last_updated(stale: false)");

            return self.decayed_value;
        }
                // should realistically only happen the first period after the rate limit is initialized
        if self.previous_channel_value.unwrap_or(Uint256::zero()).is_zero() {
            #[cfg(test)]
            println!("previous_channel_value(zero)");

            // todo: should we count the decimal places
            return Some(cosmwasm_std::Decimal256::new(self.quota.channel_value?));
            // return cosmwasm_std::Decimal::from_atomics(self.quota.channel_value?, 0).ok();
        }

        if self.period_start? == env.block.time {
            #[cfg(test)]
            println!("period_start.eq(env.block.time)");

            // no time passed, return zero, this has the edge case of two blocks potentially having
            // the same timestamp under certain conditions (fast block, loose constraints around timestamp requirements, etc...)
            return self.decayed_value;
        }
        #[cfg(test)]
        println!("checking period passed");

        let percent_passed = self.period_percent_passed(env.block.time.seconds())?;
        let previous_channel_value = cosmwasm_std::Decimal256::new(self.previous_channel_value?);
        let decayed_amount =  previous_channel_value * percent_passed;
        self.decayed_value = Some(previous_channel_value - decayed_amount);
        return self.decayed_value;
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
pub const RATE_LIMIT_TRACKERS: Map<(String, String), Vec<RateLimit>> = Map::new("flow");

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

        assert_eq!(flow._balance(), (0_u32.into(), 0_u32.into()));
        flow.add_flow(FlowType::In, 5_u32.into());
        assert_eq!(flow._balance(), (5_u32.into(), 0_u32.into()));
        flow.add_flow(FlowType::Out, 2_u32.into());
        assert_eq!(flow._balance(), (3_u32.into(), 0_u32.into()));
        // Adding flow doesn't affect expiration
        assert!(!flow.is_expired(epoch.plus_seconds(RESET_TIME_DAILY)));

        flow._expire(epoch.plus_seconds(RESET_TIME_WEEKLY), RESET_TIME_WEEKLY);
        assert_eq!(flow._balance(), (0_u32.into(), 0_u32.into()));
        assert_eq!(flow.inflow, Uint256::from(0_u32));
        assert_eq!(flow.outflow, Uint256::from(0_u32));
        assert_eq!(flow.period_end, epoch.plus_seconds(RESET_TIME_WEEKLY * 2));

        // Expiration has moved
        assert!(!flow.is_expired(epoch.plus_seconds(RESET_TIME_WEEKLY).plus_nanos(1)));
        assert!(!flow.is_expired(epoch.plus_seconds(RESET_TIME_WEEKLY * 2)));
        assert!(flow.is_expired(epoch.plus_seconds(RESET_TIME_WEEKLY * 2).plus_nanos(1)));
    }
}
