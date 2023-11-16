#![cfg(test)]

use crate::execute::bypass_update;
use crate::packet::Packet;
use crate::{contract::*, test_msg_recv, test_msg_send, ContractError};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_binary, Addr, Attribute, Uint256};

use crate::helpers::tests::verify_query_response;
use crate::msg::{InstantiateMsg, PathMsg, QueryMsg, QuotaMsg, SudoMsg};
use crate::state::tests::RESET_TIME_WEEKLY;
use crate::state::{RateLimit, GOVMODULE, IBCMODULE, RATE_LIMIT_TRACKERS, TEMPORARY_RATE_LIMIT_BYPASS};

const IBC_ADDR: &str = "IBC_MODULE";
const GOV_ADDR: &str = "GOV_MODULE";

#[test] // Tests we ccan instantiate the contract and that the owners are set correctly
fn test_bypass_update() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![],
    };
    let info = mock_info(IBC_ADDR, &vec![]);

    // we can just call .unwrap() to assert this was a success
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());
    let channel_id =format!("channel");
    let denom = format!("denom");
    let sender = Addr::unchecked("SENDER_ADDR");
    let res = bypass_update(deps.as_mut(), sender.clone(), channel_id.clone(), denom.clone(), Uint256::from_u128(100)).unwrap();
    assert_eq!(res.attributes[0].key, "sender_bypass");
    assert_eq!(res.attributes[0].value, sender.to_string());

    let bypass_queue = TEMPORARY_RATE_LIMIT_BYPASS.may_load(&deps.storage, (channel_id, denom)).unwrap().unwrap();
    assert!(bypass_queue.len() == 1);
    assert_eq!(bypass_queue[0].0, sender.to_string());
    assert_eq!(bypass_queue[0].1, Uint256::from_u128(100));
}

#[test]
fn test_bypass_update_to_zero() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![],
    };
    let info = mock_info(IBC_ADDR, &vec![]);

    // we can just call .unwrap() to assert this was a success
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());
    let channel_id =format!("channel");
    let denom = format!("denom");
    let sender = Addr::unchecked("SENDER_ADDR");
    let res = bypass_update(deps.as_mut(), sender.clone(), channel_id.clone(), denom.clone(), Uint256::from_u128(100)).unwrap();
    assert_eq!(res.attributes[0].key, "sender_bypass");
    assert_eq!(res.attributes[0].value, sender.to_string());

    let bypass_queue = TEMPORARY_RATE_LIMIT_BYPASS.may_load(&deps.storage, (channel_id.clone(), denom.clone())).unwrap().unwrap();
    assert!(bypass_queue.len() == 1);
    assert_eq!(bypass_queue[0].0, sender.to_string());
    assert_eq!(bypass_queue[0].1, Uint256::from_u128(100));

    let res = bypass_update(deps.as_mut(), sender.clone(), channel_id.clone(), denom.clone(), Uint256::zero()).unwrap();
    assert_eq!(res.attributes[0].key, "sender_bypass");
    assert_eq!(res.attributes[0].value, sender.to_string());

    let bypass_queue = TEMPORARY_RATE_LIMIT_BYPASS.may_load(&deps.storage, (channel_id, denom)).unwrap().unwrap();
    assert!(bypass_queue.len() == 1);
    assert_eq!(bypass_queue[0].0, sender.to_string());
    assert_eq!(bypass_queue[0].1, Uint256::zero());

}