#![cfg(test)]
use std::collections::HashMap;

use crate::helpers::{expired_rate_limits};
use crate::sudo::rollover_expired_rate_limits;
use crate::msg::MigrateMsg;

use crate::packet::Packet;
use crate::{contract::*, test_msg_recv, test_msg_send, ContractError};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MockApi, MockStorage, MockQuerier};
use cosmwasm_std::{from_binary, Addr, Attribute, Env, Uint256, Querier, OwnedDeps, MemoryStorage};
use cw_multi_test::{App, AppBuilder, BankKeeper, ContractWrapper, Executor};
use cosmwasm_std::Timestamp;
use crate::helpers::tests::verify_query_response;
use crate::msg::{InstantiateMsg, PathMsg, QueryMsg, QuotaMsg, SudoMsg};
use crate::state::tests::{RESET_TIME_WEEKLY, RESET_TIME_DAILY, RESET_TIME_MONTHLY};
use crate::state::{RateLimit, GOVMODULE, IBCMODULE, RATE_LIMIT_TRACKERS};
const IBC_ADDR: &str = "IBC_MODULE";
const GOV_ADDR: &str = "GOV_MODULE";

pub const SECONDS_PER_DAY: u64 = 86400;
pub const SECONDS_PER_HOUR: u64 = 3600;

pub(crate) struct TestEnv {
    pub env: Env,
    pub deps: OwnedDeps<MemoryStorage, MockApi, MockQuerier>
}
fn new_test_env(paths: &[PathMsg]) -> TestEnv {

    let mut deps: OwnedDeps<MemoryStorage, MockApi, MockQuerier> = mock_dependencies();
    let env = mock_env();
    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: paths.to_vec(),
    };
    let info = mock_info(GOV_ADDR, &vec![]);
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    TestEnv {
        deps,
        env,
    }
}

impl TestEnv {
    pub fn plus_hours(&mut self, hours: u64) {
        self.env.block.time = self.env.block.time.plus_seconds( hours * SECONDS_PER_HOUR);
    }
    pub fn plus_days(&mut self, days: u64) {
        self.env.block.time = self.env.block.time.plus_seconds(days * SECONDS_PER_DAY);
    }
}

// performs a very basic migration test, ensuring that standard migration logic works
#[test]
fn test_basic_migration() {
    let test_env = new_test_env(&[PathMsg {
        channel_id: format!("any"),
        denom: format!("denom"),
        quotas: vec![QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 10, 10)],
    }]);



    for key in RATE_LIMIT_TRACKERS.keys(&test_env.deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        match key {
            Ok((k, v)) => {
                println!("got key {}, {}", k, v);
            }
            Err(err) => {
                println!("got error {err:#?}");
            }
        }
    }
}

#[test] // Tests we ccan instantiate the contract and that the owners are set correctly
fn proper_instantiation() {
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

    // The ibc and gov modules are properly stored
    assert_eq!(IBCMODULE.load(deps.as_ref().storage).unwrap(), IBC_ADDR);
    assert_eq!(GOVMODULE.load(deps.as_ref().storage).unwrap(), GOV_ADDR);
}

#[test] // Tests that when a packet is transferred, the peropper allowance is consummed
fn consume_allowance() {
    let mut deps = mock_dependencies();

    let quota = QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 10, 10);
    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![PathMsg {
            channel_id: format!("any"),
            denom: format!("denom"),
            quotas: vec![quota],
        }],
    };
    let info = mock_info(GOV_ADDR, &vec![]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom") ,
        channel_value: 3_300_u32.into(),
        funds: 300_u32.into()
    );
    let res = sudo(deps.as_mut(), mock_env(), msg).unwrap();

    let Attribute { key, value } = &res.attributes[4];
    assert_eq!(key, "weekly_used_out");
    assert_eq!(value, "300");

    let msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_300_u32.into(),
        funds: 300_u32.into()
    );
    let err = sudo(deps.as_mut(), mock_env(), msg).unwrap_err();
    assert!(matches!(err, ContractError::RateLimitExceded { .. }));
}

#[test] // Tests that the balance of send and receive is maintained (i.e: recives are sustracted from the send allowance and sends from the receives)
fn symetric_flows_dont_consume_allowance() {
    let mut deps = mock_dependencies();

    let quota = QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 10, 10);
    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![PathMsg {
            channel_id: format!("any"),
            denom: format!("denom"),
            quotas: vec![quota],
        }],
    };
    let info = mock_info(GOV_ADDR, &vec![]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let send_msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_300_u32.into(),
        funds: 300_u32.into()
    );
    let recv_msg = test_msg_recv!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_000_u32.into(),
        funds: 300_u32.into()
    );

    let res = sudo(deps.as_mut(), mock_env(), send_msg.clone()).unwrap();
    let Attribute { key, value } = &res.attributes[3];
    assert_eq!(key, "weekly_used_in");
    assert_eq!(value, "0");
    let Attribute { key, value } = &res.attributes[4];
    assert_eq!(key, "weekly_used_out");
    assert_eq!(value, "300");

    let res = sudo(deps.as_mut(), mock_env(), recv_msg.clone()).unwrap();
    let Attribute { key, value } = &res.attributes[3];
    assert_eq!(key, "weekly_used_in");
    assert_eq!(value, "0");
    let Attribute { key, value } = &res.attributes[4];
    assert_eq!(key, "weekly_used_out");
    assert_eq!(value, "0");

    // We can still use the path. Even if we have sent more than the
    // allowance through the path (900 > 3000*.1), the current "balance"
    // of inflow vs outflow is still lower than the path's capacity/quota
    let res = sudo(deps.as_mut(), mock_env(), recv_msg.clone()).unwrap();
    let Attribute { key, value } = &res.attributes[3];
    assert_eq!(key, "weekly_used_in");
    assert_eq!(value, "300");
    let Attribute { key, value } = &res.attributes[4];
    assert_eq!(key, "weekly_used_out");
    assert_eq!(value, "0");

    let err = sudo(deps.as_mut(), mock_env(), recv_msg.clone()).unwrap_err();

    assert!(matches!(err, ContractError::RateLimitExceded { .. }));
}

#[test] // Tests that we can have different quotas for send and receive. In this test we use 4% send and 1% receive
fn asymetric_quotas() {
    let mut deps = mock_dependencies();

    let quota = QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 4, 1);
    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![PathMsg {
            channel_id: format!("any"),
            denom: format!("denom"),
            quotas: vec![quota],
        }],
    };
    let info = mock_info(GOV_ADDR, &vec![]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // Sending 2%
    let msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_060_u32.into(),
        funds: 60_u32.into()
    );
    let res = sudo(deps.as_mut(), mock_env(), msg).unwrap();
    let Attribute { key, value } = &res.attributes[4];
    assert_eq!(key, "weekly_used_out");
    assert_eq!(value, "60");

    // Sending 2% more. Allowed, as sending has a 4% allowance
    let msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_060_u32.into(),
        funds: 60_u32.into()
    );

    let res = sudo(deps.as_mut(), mock_env(), msg).unwrap();
    let Attribute { key, value } = &res.attributes[4];
    assert_eq!(key, "weekly_used_out");
    assert_eq!(value, "120");

    // Receiving 1% should still work. 4% *sent* through the path, but we can still receive.
    let recv_msg = test_msg_recv!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_000_u32.into(),
        funds: 30_u32.into()
    );
    let res = sudo(deps.as_mut(), mock_env(), recv_msg).unwrap();
    let Attribute { key, value } = &res.attributes[3];
    assert_eq!(key, "weekly_used_in");
    assert_eq!(value, "0");
    let Attribute { key, value } = &res.attributes[4];
    assert_eq!(key, "weekly_used_out");
    assert_eq!(value, "90");

    // Sending 2%. Should fail. In balance, we've sent 4% and received 1%, so only 1% left to send.
    let msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_060_u32.into(),
        funds: 60_u32.into()
    );
    let err = sudo(deps.as_mut(), mock_env(), msg.clone()).unwrap_err();
    assert!(matches!(err, ContractError::RateLimitExceded { .. }));

    // Sending 1%: Allowed; because sending has a 4% allowance. We've sent 4% already, but received 1%, so there's send cappacity again
    let msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_060_u32.into(),
        funds: 30_u32.into()
    );
    let res = sudo(deps.as_mut(), mock_env(), msg.clone()).unwrap();
    let Attribute { key, value } = &res.attributes[3];
    assert_eq!(key, "weekly_used_in");
    assert_eq!(value, "0");
    let Attribute { key, value } = &res.attributes[4];
    assert_eq!(key, "weekly_used_out");
    assert_eq!(value, "120");
}

#[test] // Tests we can get the current state of the trackers
fn query_state() {
    let mut deps: OwnedDeps<MemoryStorage, MockApi, MockQuerier> = mock_dependencies();

    let quota = QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 10, 10);
    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![PathMsg {
            channel_id: format!("any"),
            denom: format!("denom"),
            quotas: vec![quota],
        }],
    };
    let info = mock_info(GOV_ADDR, &vec![]);
    let env = mock_env();
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    let query_msg = QueryMsg::GetQuotas {
        channel_id: format!("any"),
        denom: format!("denom"),
    };

    let res = query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap();
    let value: Vec<RateLimit> = from_binary(&res).unwrap();
    assert_eq!(value[0].quota.name, "weekly");
    assert_eq!(value[0].quota.max_percentage_send, 10);
    assert_eq!(value[0].quota.max_percentage_recv, 10);
    assert_eq!(value[0].quota.duration, RESET_TIME_WEEKLY);
    assert_eq!(value[0].flow.inflow, Uint256::from(0_u32));
    assert_eq!(value[0].flow.outflow, Uint256::from(0_u32));
    assert_eq!(
        value[0].flow.period_end,
        env.block.time.plus_seconds(RESET_TIME_WEEKLY)
    );

    let send_msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_300_u32.into(),
        funds: 300_u32.into()
    );
    sudo(deps.as_mut(), mock_env(), send_msg.clone()).unwrap();

    let recv_msg = test_msg_recv!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_000_u32.into(),
        funds: 30_u32.into()
    );
    sudo(deps.as_mut(), mock_env(), recv_msg.clone()).unwrap();

    // Query
    let res = query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap();
    let value: Vec<RateLimit> = from_binary(&res).unwrap();
    verify_query_response(
        &value[0],
        "weekly",
        (10, 10),
        RESET_TIME_WEEKLY,
        30_u32.into(),
        300_u32.into(),
        env.block.time.plus_seconds(RESET_TIME_WEEKLY),
    );
}

#[test] // Tests quota percentages are between [0,100]
fn bad_quotas() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![PathMsg {
            channel_id: format!("any"),
            denom: format!("denom"),
            quotas: vec![QuotaMsg {
                name: "bad_quota".to_string(),
                duration: 200,
                send_recv: (5000, 101),
            }],
        }],
    };
    let info = mock_info(IBC_ADDR, &vec![]);

    let env = mock_env();
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // If a quota is higher than 100%, we set it to 100%
    let query_msg = QueryMsg::GetQuotas {
        channel_id: format!("any"),
        denom: format!("denom"),
    };
    let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
    let value: Vec<RateLimit> = from_binary(&res).unwrap();
    verify_query_response(
        &value[0],
        "bad_quota",
        (100, 100),
        200,
        0_u32.into(),
        0_u32.into(),
        env.block.time.plus_seconds(200),
    );
}

#[test] // Tests that undo reverts a packet send without affecting expiration or channel value
fn undo_send() {
    let mut deps = mock_dependencies();

    let quota = QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 10, 10);
    let msg = InstantiateMsg {
        gov_module: Addr::unchecked(GOV_ADDR),
        ibc_module: Addr::unchecked(IBC_ADDR),
        paths: vec![PathMsg {
            channel_id: format!("any"),
            denom: format!("denom"),
            quotas: vec![quota],
        }],
    };
    let info = mock_info(GOV_ADDR, &vec![]);
    let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let send_msg = test_msg_send!(
        channel_id: format!("channel"),
        denom: format!("denom"),
        channel_value: 3_300_u32.into(),
        funds: 300_u32.into()
    );
    let undo_msg = SudoMsg::UndoSend {
        packet: Packet::mock(
            format!("channel"),
            format!("channel"),
            format!("denom"),
            300_u32.into(),
        ),
    };

    sudo(deps.as_mut(), mock_env(), send_msg.clone()).unwrap();

    let trackers = RATE_LIMIT_TRACKERS
        .load(&deps.storage, ("any".to_string(), "denom".to_string()))
        .unwrap();
    assert_eq!(
        trackers.first().unwrap().flow.outflow,
        Uint256::from(300_u32)
    );
    let period_end = trackers.first().unwrap().flow.period_end;
    let channel_value = trackers.first().unwrap().quota.channel_value;

    sudo(deps.as_mut(), mock_env(), undo_msg.clone()).unwrap();

    let trackers = RATE_LIMIT_TRACKERS
        .load(&deps.storage, ("any".to_string(), "denom".to_string()))
        .unwrap();
    assert_eq!(trackers.first().unwrap().flow.outflow, Uint256::from(0_u32));
    assert_eq!(trackers.first().unwrap().flow.period_end, period_end);
    assert_eq!(trackers.first().unwrap().quota.channel_value, channel_value);
}

#[test]
fn test_basic_message() {
    let json = r#"{"send_packet":{"packet":{"sequence":2,"source_port":"transfer","source_channel":"channel-0","destination_port":"transfer","destination_channel":"channel-0","data":{"denom":"stake","amount":"125000000000011250","sender":"osmo1dwtagd6xzl4eutwtyv6mewra627lkg3n3w26h6","receiver":"osmo1yvjkt8lnpxucjmspaj5ss4aa8562gx0a3rks8s"},"timeout_height":{"revision_height":100}}}}"#;
    let _parsed: SudoMsg = serde_json_wasm::from_str(json).unwrap();
    //println!("{parsed:?}");
}

#[test]
fn test_testnet_message() {
    let json = r#"{"send_packet":{"packet":{"sequence":4,"source_port":"transfer","source_channel":"channel-0","destination_port":"transfer","destination_channel":"channel-1491","data":{"denom":"uosmo","amount":"100","sender":"osmo1cyyzpxplxdzkeea7kwsydadg87357qnahakaks","receiver":"osmo1c584m4lq25h83yp6ag8hh4htjr92d954vklzja"},"timeout_height":{},"timeout_timestamp":1668024637477293371}}}"#;
    let _parsed: SudoMsg = serde_json_wasm::from_str(json).unwrap();
    //println!("{parsed:?}");
}

#[test]
fn test_tokenfactory_message() {
    let json = r#"{"send_packet":{"packet":{"sequence":4,"source_port":"transfer","source_channel":"channel-0","destination_port":"transfer","destination_channel":"channel-1491","data":{"denom":"transfer/channel-0/factory/osmo12smx2wdlyttvyzvzg54y2vnqwq2qjateuf7thj/czar","amount":"100000000000000000","sender":"osmo1cyyzpxplxdzkeea7kwsydadg87357qnahakaks","receiver":"osmo1c584m4lq25h83yp6ag8hh4htjr92d954vklzja"},"timeout_height":{},"timeout_timestamp":1668024476848430980}}}"#;
    let _parsed: SudoMsg = serde_json_wasm::from_str(json).unwrap();
    //println!("{parsed:?}");
}


#[test]
fn test_expired_rate_limits() {
    let mut test_env = new_test_env(&[PathMsg {
        channel_id: format!("any"),
        denom: format!("denom"),
        quotas: vec![
            QuotaMsg::new("daily", RESET_TIME_DAILY, 1, 1),
            QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 5, 5),
            QuotaMsg::new("monthly", RESET_TIME_MONTHLY, 5, 5),
        ],
    }]);
    // no rules should be expired
    let expired_limits = expired_rate_limits(test_env.deps.as_ref(), test_env.env.block.time);
    assert_eq!(expired_limits.len(), 0);

    // advance timestamp by half day
    test_env.plus_hours(12);

    // still no rules should be expired
    let expired_limits = expired_rate_limits(test_env.deps.as_ref(), test_env.env.block.time);
    assert_eq!(expired_limits.len(), 0);

    // advance timestamp by 13 hours
    test_env.plus_hours(13);

    // only 1 rule should be expired
    let expired_limits = expired_rate_limits(test_env.deps.as_ref(), test_env.env.block.time);
    assert_eq!(expired_limits[0].1.len(), 1);
    assert_eq!(expired_limits[0].1[0].quota.name, "daily");

    // advance timestamp by 6 days
    test_env.plus_days(6);

    // weekly + daily rules should be expired
    let expired_limits = expired_rate_limits(test_env.deps.as_ref(), test_env.env.block.time);
    assert_eq!(expired_limits[0].1.len(), 2);
    // as long as the ordering of the `range(..)` function is the same
    // this test shouldn't fail
    assert_eq!(expired_limits[0].1[0].quota.name, "daily");
    assert_eq!(expired_limits[0].1[1].quota.name, "weekly");
    // advance timestamp by 24 days for a total of 31 days passed
    test_env.plus_days(24);
 
    // daily, weekly, monthly rules should be expired
    let expired_limits = expired_rate_limits(test_env.deps.as_ref(), test_env.env.block.time);
    assert_eq!(expired_limits[0].1.len(), 3);
    assert_eq!(expired_limits[0].1[0].quota.name, "daily");
    assert_eq!(expired_limits[0].1[1].quota.name, "weekly");
    assert_eq!(expired_limits[0].1[2].quota.name, "monthly");
}
#[test]
fn test_rollover_expired_rate_limits() {
    let mut test_env = new_test_env(&[PathMsg {
        channel_id: format!("any"),
        denom: format!("denom"),
        quotas: vec![
            QuotaMsg::new("daily", RESET_TIME_DAILY, 1, 1),
            QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 5, 5),
            QuotaMsg::new("monthly", RESET_TIME_MONTHLY, 5, 5),
        ],
    }]);

    // shorthand for returning all rules
    fn get_rules(test_env: &TestEnv) -> HashMap<String, RateLimit> {
        let rules = RATE_LIMIT_TRACKERS.range(&test_env.deps.storage, None, None, cosmwasm_std::Order::Ascending).flatten().collect::<Vec<_>>();
        let mut indexed_rules: HashMap<String, RateLimit> = HashMap::new();
        rules.into_iter().for_each(|(_, rules)| {
            rules.into_iter().for_each(|rule| {indexed_rules.insert(rule.quota.name.clone(), rule);});
        });
        indexed_rules
    }

    // store a copy of the unchanged rules
    let original_rules = get_rules(&test_env);
    // ensure the helper function indexes rules as expected
    assert!(original_rules.contains_key("daily"));
    assert!(original_rules.contains_key("weekly"));
    assert!(original_rules.contains_key("monthly"));

    // no rules should be expired
    let expired_limits = expired_rate_limits(test_env.deps.as_ref(), test_env.env.block.time);
    assert_eq!(expired_limits.len(), 0);

    // advance timestamp by a day
    test_env.plus_hours(25);

    // only 1 rule should be expired
    let expired_limits = expired_rate_limits(test_env.deps.as_ref(), test_env.env.block.time);
    assert_eq!(expired_limits[0].1.len(), 1);
    assert_eq!(expired_limits[0].1[0].quota.name, "daily");

    // trigger expiration of daily rate limits
    rollover_expired_rate_limits(test_env.deps.as_mut(), test_env.env.clone()).unwrap();

    // store a copy of rules after the daily limit has changed
    let daily_rules_changed = get_rules(&test_env);
    // ensure the daily period is different
    assert!(daily_rules_changed.get("daily").unwrap().flow.period_end > original_rules.get("daily").unwrap().flow.period_end);
    // ensure weekly and monthly rules are the same
    assert!(daily_rules_changed.get("weekly").unwrap().flow.period_end == original_rules.get("weekly").unwrap().flow.period_end);
    assert!(daily_rules_changed.get("monthly").unwrap().flow.period_end == original_rules.get("monthly").unwrap().flow.period_end);

    // advance timestamp by half day, no rules should be changed
    test_env.plus_hours(12);

    // there should be no expired rate limits
    let expired_limits = expired_rate_limits(test_env.deps.as_ref(), test_env.env.block.time);
    assert_eq!(expired_limits.len(), 0);

    // advance timestamp by another half day
    test_env.plus_hours(13);

    // daily rule should change again
    rollover_expired_rate_limits(test_env.deps.as_mut(), test_env.env.clone()).unwrap();

    let daily_rules_changed2 = get_rules(&test_env);
    // ensure the daily period is different
    assert!(daily_rules_changed2.get("daily").unwrap().flow.period_end > daily_rules_changed.get("daily").unwrap().flow.period_end);
    // ensure weekly and monthly rules are the same
    assert!(daily_rules_changed2.get("weekly").unwrap().flow.period_end == original_rules.get("weekly").unwrap().flow.period_end);
    assert!(daily_rules_changed2.get("monthly").unwrap().flow.period_end == original_rules.get("monthly").unwrap().flow.period_end);

    // advance timestamp by 6 days
    test_env.plus_days(6);

    // daily rule + weekly rules should change
    rollover_expired_rate_limits(test_env.deps.as_mut(), test_env.env.clone()).unwrap();

    let weekly_rules_changed = get_rules(&test_env);
    // ensure the daily period is different
    assert!(weekly_rules_changed.get("daily").unwrap().flow.period_end > daily_rules_changed2.get("daily").unwrap().flow.period_end);
    // ensure weekly is different
    assert!(weekly_rules_changed.get("weekly").unwrap().flow.period_end > daily_rules_changed2.get("weekly").unwrap().flow.period_end);
    // ensure monthly is unchanged
    assert!(weekly_rules_changed.get("monthly").unwrap().flow.period_end == original_rules.get("monthly").unwrap().flow.period_end);

    // advance timestamp by 24 days
    test_env.plus_days(24);

    // all rules should now rollover
    rollover_expired_rate_limits(test_env.deps.as_mut(), test_env.env.clone()).unwrap();

    let monthly_rules_changed = get_rules(&test_env);
    // ensure all three periods have reset
    assert!(monthly_rules_changed.get("daily").unwrap().flow.period_end > weekly_rules_changed.get("daily").unwrap().flow.period_end);
    assert!(monthly_rules_changed.get("weekly").unwrap().flow.period_end > weekly_rules_changed.get("weekly").unwrap().flow.period_end);
    assert!(monthly_rules_changed.get("monthly").unwrap().flow.period_end > weekly_rules_changed.get("monthly").unwrap().flow.period_end);

}

#[test]
fn test_decay_two_period() {
    let mut test_env = new_test_env(&[PathMsg {
        channel_id: format!("any"),
        denom: format!("denom"),
        quotas: vec![
            QuotaMsg::new("daily", RESET_TIME_DAILY, 1, 1),
            QuotaMsg::new("weekly", RESET_TIME_WEEKLY, 5, 5),
            QuotaMsg::new("monthly", RESET_TIME_MONTHLY, 5, 5),
        ],
    }]);

    // shorthand for returning all rules
    fn get_rules(test_env: &TestEnv) -> HashMap<String, RateLimit> {
        let rules = RATE_LIMIT_TRACKERS.range(&test_env.deps.storage, None, None, cosmwasm_std::Order::Ascending).flatten().collect::<Vec<_>>();
        let mut indexed_rules: HashMap<String, RateLimit> = HashMap::new();
        rules.into_iter().for_each(|(_, rules)| {
            rules.into_iter().for_each(|rule| {indexed_rules.insert(rule.quota.name.clone(), rule);});
        });
        indexed_rules
    }

    let rules = get_rules(&test_env);

    for (k, rate_limit) in rules {
        println!("key: {k}, rate_limit: {rate_limit:#?}");
    }
    /*

    let mut env = mock_env();
    env.block.height = 12_345;
    env.block.time = Timestamp::from_seconds(1690757434);
    let mut rv = RateLimitValue {
        previous_period_value: 10_000,
        current_period_value: 5_000,
        period_start: Timestamp::from_seconds(1690757434),
        period_end: Timestamp::from_seconds(1690786248),
        interval_check_height: 12_344,
        decayed_value: cosmwasm_std::Decimal::raw(0),
    };
    let rate = rv.check_decay_rate(env.clone()).unwrap();
    assert!(rate == cosmwasm_std::Decimal::zero());
    env.block.height = 12_346;
    let rate = rv.check_decay_rate(env.clone()).unwrap();
    assert!(rate == cosmwasm_std::Decimal::zero());
    env.block.time = Timestamp::from_seconds(1690763805);
    let rate: u128 = rv.check_decay_rate(env.clone()).unwrap().atomics().into();
    assert!(rate == 2200000000000000000000);
    let val: u128 = rv.averaged_value(env.clone(), 8_000).unwrap().atomics().into();
    assert!(val == 5100000000000000000000);
*/

}