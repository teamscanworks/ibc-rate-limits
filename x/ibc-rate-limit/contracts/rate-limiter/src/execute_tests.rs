#![cfg(test)]

use crate::execute::{bypass_update, submit_intent, intent_ok, remove_intent};
use crate::contract::*;
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{ Addr,  Uint256, Timestamp};

use crate::msg::{InstantiateMsg, PathMsg, QuotaMsg};
use crate::state::tests::{RESET_TIME_WEEKLY, RESET_TIME_DAILY};
use crate::state::{RATE_LIMIT_TRACKERS, TEMPORARY_RATE_LIMIT_BYPASS, INTENT_QUEUE, Path};

const IBC_ADDR: &str = "IBC_MODULE";
const GOV_ADDR: &str = "GOV_MODULE";


#[test] // Tests we ccan instantiate the contract and that the owners are set correctly
fn test_bypass_update_no_threshold() {
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
    let res = bypass_update(deps.as_mut(), mock_env().block.time, Addr::unchecked(GOV_ADDR), sender.clone(), channel_id.clone(), denom.clone(), Uint256::from_u128(100)).unwrap();
    assert_eq!(res.attributes[0].key, "sender_bypass");
    assert_eq!(res.attributes[0].value, sender.to_string());

    let bypass_queue = TEMPORARY_RATE_LIMIT_BYPASS.may_load(&deps.storage, (channel_id, denom)).unwrap().unwrap();
    assert!(bypass_queue.len() == 1);
    assert_eq!(bypass_queue[0].0, sender.to_string());
    assert_eq!(bypass_queue[0].1, Uint256::from_u128(100));
}

#[test]
fn test_submit_intent_threshold() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(1700553596);
    let quota_daily = QuotaMsg::new("daily", RESET_TIME_DAILY, 30, 40);
    let quota_weekly = QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 30, 40);
    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![PathMsg {
            channel_id: format!("any"),
            denom: format!("denom"),
            quotas: vec![quota_daily.clone(), quota_weekly.clone()],
        }],
    };
    let info = mock_info(IBC_ADDR, &vec![]);
    let path = &Path::new("any", "denom");
    let _ = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    {
        let mut trackers = RATE_LIMIT_TRACKERS.load(
            &deps.storage,
            path.into()
        ).unwrap();
        trackers.iter_mut().for_each(|tracker| {
            tracker.quota.channel_value = Some(Uint256::from_u128(
                1_000_000_000
            ));
        });
        RATE_LIMIT_TRACKERS.save(
            &mut deps.storage,
            path.into(),
            &trackers
        ).unwrap();
    }
    let threshold = Uint256::from_u128(250000000);
    let not_threshold = threshold - Uint256::one();

    let channel_id =format!("any");
    let denom = format!("denom");
    let sender = Addr::unchecked("SENDER_ADDR");


    submit_intent(
        deps.as_mut(),
        env.clone(),
        Addr::unchecked(GOV_ADDR),
        sender.clone(),
        channel_id.clone(),
        denom.clone(),
        threshold
    ).unwrap();

    assert!(submit_intent(
        deps.as_mut(),
        env.clone(),
        Addr::unchecked(GOV_ADDR),
        sender.clone(),
        channel_id.clone(),
        denom.clone(),
        not_threshold
    ).is_err());
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
    let res = bypass_update(deps.as_mut(), mock_env().block.time, Addr::unchecked(GOV_ADDR), sender.clone(), channel_id.clone(), denom.clone(), Uint256::from_u128(100)).unwrap();
    assert_eq!(res.attributes[0].key, "sender_bypass");
    assert_eq!(res.attributes[0].value, sender.to_string());

    let bypass_queue = TEMPORARY_RATE_LIMIT_BYPASS.may_load(&deps.storage, (channel_id.clone(), denom.clone())).unwrap().unwrap();
    assert!(bypass_queue.len() == 1);
    assert_eq!(bypass_queue[0].0, sender.to_string());
    assert_eq!(bypass_queue[0].1, Uint256::from_u128(100));

    let res = bypass_update(deps.as_mut(), mock_env().block.time, Addr::unchecked(GOV_ADDR), sender.clone(), channel_id.clone(), denom.clone(), Uint256::zero()).unwrap();
    assert_eq!(res.attributes[0].key, "sender_bypass");
    assert_eq!(res.attributes[0].value, sender.to_string());

    let bypass_queue = TEMPORARY_RATE_LIMIT_BYPASS.may_load(&deps.storage, (channel_id, denom)).unwrap().unwrap();
    assert!(bypass_queue.len() == 1);
    assert_eq!(bypass_queue[0].0, sender.to_string());
    assert_eq!(bypass_queue[0].1, Uint256::zero());

}

#[test]
fn test_submit_intent() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();

    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![],
    };

    let channel_id = format!("channel_id");
    let denom = format!("denom");
    let sender = Addr::unchecked("SENDER");

    let info = mock_info(IBC_ADDR, &vec![]);
    
    let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    env.block.time = Timestamp::from_seconds(1700383122);

    assert_eq!(0, res.messages.len());
    let res = submit_intent(
        deps.as_mut(),
        env.clone(),
        Addr::unchecked(GOV_ADDR),
        sender.clone(),
        channel_id,
        denom,
        Uint256::from_u128(1_000_000)
    ).unwrap();

    assert_eq!(res.attributes[0].key, "submit_intent");
    assert_eq!(res.attributes[0].value, sender.to_string());
    assert_eq!(res.attributes[1].key, "amount");
    assert_eq!(res.attributes[1].value, Uint256::from_u128(1_000_000).to_string());
    assert_eq!(res.attributes[2].key, "channel_id");
    assert_eq!(res.attributes[2].value, "channel_id");
    assert_eq!(res.attributes[3].key, "denom");
    assert_eq!(res.attributes[3].value, "denom");
    assert_eq!(res.attributes[4].key, "intent_action");
    assert_eq!(res.attributes[4].value, "submit");


    let intent = INTENT_QUEUE.load(&deps.storage, (sender.to_string(), "channel_id".to_string(), "denom".to_string())).unwrap();
    assert_eq!(intent.0, Uint256::from_u128(1_000_000));
    assert_eq!(intent.1, env.block.time.plus_seconds(86400));

    assert!(!intent_ok(intent, env.block.time, Uint256::from_u128(1_000_000)));

    env.block.time = env.block.time.plus_seconds(86400);

    assert!(intent_ok(intent, env.block.time, Uint256::from_u128(1_000_000)));

    assert!(submit_intent(deps.as_mut(), env.clone(),Addr::unchecked(GOV_ADDR), sender.clone(), "channel_id".to_string(), "denom".to_string(), Uint256::from_u128(1_000_000)).is_err());

    let intent = remove_intent(deps.as_mut(),  Addr::unchecked(GOV_ADDR), sender.clone(), "channel_id".to_string(), "denom".to_string()).unwrap();

    assert_eq!(intent.attributes[0].key, "remove_intent");
    assert_eq!(intent.attributes[0].value, sender.to_string());
    assert_eq!(intent.attributes[1].key, "channel_id");
    assert_eq!(intent.attributes[1].value, "channel_id");
    assert_eq!(intent.attributes[2].key, "denom");
    assert_eq!(intent.attributes[2].value, "denom");
    assert_eq!(intent.attributes[3].key, "intent_action");
    assert_eq!(intent.attributes[3].value, "remove");

    assert!(remove_intent(deps.as_mut(),  Addr::unchecked(GOV_ADDR), sender.clone(), "channel_id".to_string(), "denom".to_string()).is_err());

    assert!(INTENT_QUEUE.load(&deps.storage, (sender.to_string(), "channel_id".to_string(), "denom".to_string())).is_err());
}