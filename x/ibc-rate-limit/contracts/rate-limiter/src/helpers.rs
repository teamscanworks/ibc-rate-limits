#![cfg(test)]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{to_binary, Addr, CosmosMsg, StdResult, WasmMsg};

use crate::msg::ExecuteMsg;
use crate::msg::SudoMsg;

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

pub mod tests {
    use cosmwasm_std::{Timestamp, Uint256};

    use crate::state::{RateLimit, RateLimitType};

    pub fn verify_query_response(
        value: &RateLimitType,
        quota_name: &str,
        send_recv: (u32, u32),
        duration: u64,
        inflow: Uint256,
        outflow: Uint256,
        period_end: Timestamp,
    ) {
        let quota = value.quota_type();
        let flow = value.flow();
        assert_eq!(quota.name(), quota_name);
        assert_eq!(quota.max_percentage_send(), send_recv.0);
        assert_eq!(quota.max_percentage_recv(), send_recv.1);
        assert_eq!(quota.duration(), duration);
        assert_eq!(flow.inflow, inflow);
        assert_eq!(flow.outflow, outflow);
        assert_eq!(flow.period_end, period_end);
    }
}
