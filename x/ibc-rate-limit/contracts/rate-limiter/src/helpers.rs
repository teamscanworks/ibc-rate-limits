#![cfg(test)]
use cosmwasm_std::Deps;
use cosmwasm_std::DepsMut;
use cosmwasm_std::Env;
use cosmwasm_std::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{to_binary, Addr, CosmosMsg, StdResult, WasmMsg};

use crate::msg::ExecuteMsg;
use crate::msg::SudoMsg;
use crate::state::RateLimit;
use crate::state::RATE_LIMIT_TRACKERS;
use crate::ContractError;

/// CwTemplateContract is a wrapper around Addr that provides a lot of helpers
/// for working with this.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RateLimitingContract(pub Addr);

impl RateLimitingContract {
    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>>(&self, msg: T) -> StdResult<CosmosMsg> {
        let msg = to_binary(&msg.into())?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds: vec![],
        }
        .into())
    }

    pub fn sudo<T: Into<SudoMsg>>(&self, msg: T) -> cw_multi_test::SudoMsg {
        let msg = to_binary(&msg.into()).unwrap();
        cw_multi_test::SudoMsg::Wasm(cw_multi_test::WasmSudo {
            contract_addr: self.addr().into(),
            msg,
        })
    }
}

/// helper function that is used to iterate over all existing rate limits automatically expiring flows for rules which have passed the end period
pub fn rollover_expired_rate_limits(deps: DepsMut, env: Env) -> Result<(), ContractError> {
    // possible alternative here is to not collect the iterator, and then use a dequeue or something similiar to track rate limit keys that need to be updated
    for (key, mut rules) in RATE_LIMIT_TRACKERS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending).flatten().collect::<Vec<_>>() {
        // avoid storage saves unless an actual rule was updated
        let mut rule_updated = false;
        rules.iter_mut().for_each(|rule| {
            if rule.flow.is_expired(env.block.time) {
                rule.flow.expire(env.block.time, rule.quota.duration);
                rule_updated = true;
            }
        });
        if rule_updated {
            RATE_LIMIT_TRACKERS.save(deps.storage, key, &rules)?;
        }
    }

    Ok(())
}

/// returns all rate limits that have expired and can be rolled over
pub fn expired_rate_limits(deps: Deps, time: Timestamp) -> Vec<((String, String), Vec<RateLimit>)> {
    RATE_LIMIT_TRACKERS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .flatten()
        .filter_map(|(k, rules)| {
            let rules = rules
            .into_iter()
            .filter(|rule| rule.flow.is_expired(time))
            .collect::<Vec<_>>();
            if rules.is_empty() {
                return None;
            }
            Some((
                k,
                rules,
            ))
        })
        .collect()
}

pub mod tests {
    use cosmwasm_std::{Timestamp, Uint256};

    use crate::state::RateLimit;

    pub fn verify_query_response(
        value: &RateLimit,
        quota_name: &str,
        send_recv: (u32, u32),
        duration: u64,
        inflow: Uint256,
        outflow: Uint256,
        period_end: Timestamp,
    ) {
        assert_eq!(value.quota.name, quota_name);
        assert_eq!(value.quota.max_percentage_send, send_recv.0);
        assert_eq!(value.quota.max_percentage_recv, send_recv.1);
        assert_eq!(value.quota.duration, duration);
        assert_eq!(value.flow.inflow, inflow);
        assert_eq!(value.flow.outflow, outflow);
        assert_eq!(value.flow.period_end, period_end);
    }
}
