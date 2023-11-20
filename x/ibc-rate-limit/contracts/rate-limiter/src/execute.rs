use crate::msg::{PathMsg, QuotaMsg};
use crate::state::{
    Flow, Path, RateLimit, GOVMODULE, IBCMODULE, INTENT_QUEUE, RATE_LIMIT_TRACKERS,
    TEMPORARY_RATE_LIMIT_BYPASS,
};
use crate::ContractError;
use cosmwasm_std::{Addr, DepsMut, Env, Response, Timestamp, Uint256};

pub fn add_new_paths(
    deps: DepsMut,
    path_msgs: Vec<PathMsg>,
    now: Timestamp,
) -> Result<(), ContractError> {
    for path_msg in path_msgs {
        let path = Path::new(path_msg.channel_id, path_msg.denom);

        RATE_LIMIT_TRACKERS.save(
            deps.storage,
            path.into(),
            &path_msg
                .quotas
                .iter()
                .map(|q| RateLimit {
                    quota: q.into(),
                    flow: Flow::new(0_u128, 0_u128, now, q.duration),
                })
                .collect(),
        )?
    }
    Ok(())
}

pub fn try_add_path(
    deps: DepsMut,
    sender: Addr,
    channel_id: String,
    denom: String,
    quotas: Vec<QuotaMsg>,
    now: Timestamp,
) -> Result<Response, ContractError> {
    // codenit: should we make a function for checking this authorization?
    let ibc_module = IBCMODULE.load(deps.storage)?;
    let gov_module = GOVMODULE.load(deps.storage)?;
    if sender != ibc_module && sender != gov_module {
        return Err(ContractError::Unauthorized {});
    }
    add_new_paths(deps, vec![PathMsg::new(&channel_id, &denom, quotas)], now)?;

    Ok(Response::new()
        .add_attribute("method", "try_add_channel")
        .add_attribute("channel_id", channel_id)
        .add_attribute("denom", denom))
}

pub fn try_remove_path(
    deps: DepsMut,
    sender: Addr,
    channel_id: String,
    denom: String,
) -> Result<Response, ContractError> {
    let ibc_module = IBCMODULE.load(deps.storage)?;
    let gov_module = GOVMODULE.load(deps.storage)?;
    if sender != ibc_module && sender != gov_module {
        return Err(ContractError::Unauthorized {});
    }

    let path = Path::new(&channel_id, &denom);
    RATE_LIMIT_TRACKERS.remove(deps.storage, path.into());
    Ok(Response::new()
        .add_attribute("method", "try_remove_channel")
        .add_attribute("denom", denom)
        .add_attribute("channel_id", channel_id))
}

// Reset specified quote_id for the given channel_id
pub fn try_reset_path_quota(
    deps: DepsMut,
    sender: Addr,
    channel_id: String,
    denom: String,
    quota_id: String,
    now: Timestamp,
) -> Result<Response, ContractError> {
    let gov_module = GOVMODULE.load(deps.storage)?;
    if sender != gov_module {
        return Err(ContractError::Unauthorized {});
    }

    let path = Path::new(&channel_id, &denom);
    RATE_LIMIT_TRACKERS.update(deps.storage, path.into(), |maybe_rate_limit| {
        match maybe_rate_limit {
            None => Err(ContractError::QuotaNotFound {
                quota_id,
                channel_id: channel_id.clone(),
                denom: denom.clone(),
            }),
            Some(mut limits) => {
                // Q: What happens here if quote_id not found? seems like we return ok?
                limits.iter_mut().for_each(|limit| {
                    if limit.quota.name == quota_id.as_ref() {
                        limit.flow.expire(now, limit.quota.duration)
                    }
                });
                Ok(limits)
            }
        }
    })?;

    Ok(Response::new()
        .add_attribute("method", "try_reset_channel")
        .add_attribute("channel_id", channel_id))
}

/// updates the bypass queue to allow the sender to send up to amount in a single transaction without
/// triggering rate limit evaluation
///
/// to "remove" an address from the bypass queue you can set `amount == 0`
pub fn bypass_update(
    deps: DepsMut,
    msg_invoker: Addr,
    sender: Addr,
    channel_id: String,
    denom: String,
    amount: Uint256,
) -> Result<Response, ContractError> {
    let ibc_module = IBCMODULE.load(deps.storage)?;
    let gov_module = GOVMODULE.load(deps.storage)?;
    if msg_invoker != ibc_module && msg_invoker != gov_module {
        return Err(ContractError::Unauthorized {});
    }
    let sender = sender.to_string();
    let path = &Path::new(channel_id, denom);
    let mut bypass_queue = TEMPORARY_RATE_LIMIT_BYPASS
        .may_load(deps.storage, path.into())?
        .unwrap_or_default();
    // stores whether or not the sender address is currently present in the bypass queue and was overriden
    let mut found = false;
    for s in bypass_queue.iter_mut() {
        if s.0.eq(&sender) {
            s.1 = amount;
            found = true;
            break;
        }
    }
    // address not found so update the bypass queue with a new entry
    if !found {
        bypass_queue.push((sender.clone(), amount));
    }

    TEMPORARY_RATE_LIMIT_BYPASS.save(deps.storage, path.into(), &bypass_queue)?;

    Ok(Response::new()
        .add_attribute("sender_bypass", sender.to_string())
        .add_attribute("amount", amount)
        .add_attribute("channel_id", path.channel.clone())
        .add_attribute("denom", path.denom.clone()))
}

pub fn submit_intent(
    deps: DepsMut,
    env: Env,
    msg_invoker: Addr,
    sender: Addr,
    channel_id: String,
    denom: String,
    amount: Uint256,
) -> Result<Response, ContractError> {
    let ibc_module = IBCMODULE.load(deps.storage)?;
    let gov_module = GOVMODULE.load(deps.storage)?;
    if msg_invoker != ibc_module && msg_invoker != gov_module {
        return Err(ContractError::Unauthorized {});
    }

    // unlock time is the current block time plus 24 hours (86400 seconds)
    let unlock_time = env.block.time.plus_seconds(86400);

    let path = &Path::new(channel_id, denom);

    if INTENT_QUEUE.has(
        deps.storage,
        (sender.to_string(), path.channel.clone(), path.denom.clone()),
    ) {
        return Err(ContractError::IntentAlreadyPresent);
    }
    let mut intent = INTENT_QUEUE
        .may_load(
            deps.storage,
            (sender.to_string(), path.channel.clone(), path.denom.clone()),
        )?
        .unwrap_or_default();

    intent.0 = amount;
    intent.1 = unlock_time;

    INTENT_QUEUE.save(
        deps.storage,
        (sender.to_string(), path.channel.clone(), path.denom.clone()),
        &intent,
    )?;

    Ok(Response::new()
        .add_attribute("submit_intent", sender.to_string())
        .add_attribute("amount", amount)
        .add_attribute("channel_id", path.channel.clone())
        .add_attribute("denom", path.denom.clone())
        .add_attribute("intent_action", "submit"))
}

/// removes an intent from the intent queue
pub fn remove_intent(
    deps: DepsMut,
    msg_invoker: Addr,
    sender: Addr,
    channel_id: String,
    denom: String,
) -> Result<Response, ContractError> {
    let ibc_module = IBCMODULE.load(deps.storage)?;
    let gov_module = GOVMODULE.load(deps.storage)?;
    if msg_invoker != ibc_module && msg_invoker != gov_module {
        return Err(ContractError::Unauthorized {});
    }
    
    let path = &Path::new(channel_id, denom);

    if INTENT_QUEUE.has(
        deps.storage,
        (sender.to_string(), path.channel.clone(), path.denom.clone()),
    ) {
        INTENT_QUEUE.remove(
            deps.storage,
            (sender.to_string(), path.channel.clone(), path.denom.clone()),
        );
    } else {
        return Err(ContractError::IntentNotPresent);
    }
    Ok(Response::new()
        .add_attribute("remove_intent", sender.to_string())
        .add_attribute("channel_id", path.channel.clone())
        .add_attribute("denom", path.denom.clone())
        .add_attribute("intent_action", "remove"))
}

/// returns whether or not the intent is ok to be consumed
pub fn intent_ok(intent: (Uint256, Timestamp), block_time: Timestamp, funds: Uint256) -> bool {
    block_time >= intent.1 && funds.eq(&intent.0)
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{from_binary, Addr, StdError, Timestamp, Uint256};

    use crate::contract::{execute, query};
    use crate::execute::intent_ok;
    use crate::helpers::tests::verify_query_response;
    use crate::msg::{ExecuteMsg, QueryMsg, QuotaMsg};
    use crate::state::{RateLimit, GOVMODULE, IBCMODULE};

    const IBC_ADDR: &str = "IBC_MODULE";
    const GOV_ADDR: &str = "GOV_MODULE";

    #[test] // Tests AddPath and RemovePath messages
    fn management_add_and_remove_path() {
        let mut deps = mock_dependencies();
        IBCMODULE
            .save(deps.as_mut().storage, &Addr::unchecked(IBC_ADDR))
            .unwrap();
        GOVMODULE
            .save(deps.as_mut().storage, &Addr::unchecked(GOV_ADDR))
            .unwrap();

        let msg = ExecuteMsg::AddPath {
            channel_id: format!("channel"),
            denom: format!("denom"),
            quotas: vec![QuotaMsg {
                name: "daily".to_string(),
                duration: 1600,
                send_recv: (3, 5),
            }],
        };
        let info = mock_info(IBC_ADDR, &vec![]);

        let env = mock_env();
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let query_msg = QueryMsg::GetQuotas {
            channel_id: format!("channel"),
            denom: format!("denom"),
        };

        let res = query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap();

        let value: Vec<RateLimit> = from_binary(&res).unwrap();
        verify_query_response(
            &value[0],
            "daily",
            (3, 5),
            1600,
            0_u32.into(),
            0_u32.into(),
            env.block.time.plus_seconds(1600),
        );

        assert_eq!(value.len(), 1);

        // Add another path
        let msg = ExecuteMsg::AddPath {
            channel_id: format!("channel2"),
            denom: format!("denom"),
            quotas: vec![QuotaMsg {
                name: "daily".to_string(),
                duration: 1600,
                send_recv: (3, 5),
            }],
        };
        let info = mock_info(IBC_ADDR, &vec![]);

        let env = mock_env();
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // remove the first one
        let msg = ExecuteMsg::RemovePath {
            channel_id: format!("channel"),
            denom: format!("denom"),
        };

        let info = mock_info(IBC_ADDR, &vec![]);
        let env = mock_env();
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // The channel is not there anymore
        let err = query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap_err();
        assert!(matches!(err, StdError::NotFound { .. }));

        // The second channel is still there
        let query_msg = QueryMsg::GetQuotas {
            channel_id: format!("channel2"),
            denom: format!("denom"),
        };
        let res = query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap();
        let value: Vec<RateLimit> = from_binary(&res).unwrap();
        assert_eq!(value.len(), 1);
        verify_query_response(
            &value[0],
            "daily",
            (3, 5),
            1600,
            0_u32.into(),
            0_u32.into(),
            env.block.time.plus_seconds(1600),
        );

        // Paths are overriden if they share a name and denom
        let msg = ExecuteMsg::AddPath {
            channel_id: format!("channel2"),
            denom: format!("denom"),
            quotas: vec![QuotaMsg {
                name: "different".to_string(),
                duration: 5000,
                send_recv: (50, 30),
            }],
        };
        let info = mock_info(IBC_ADDR, &vec![]);

        let env = mock_env();
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        let query_msg = QueryMsg::GetQuotas {
            channel_id: format!("channel2"),
            denom: format!("denom"),
        };
        let res = query(deps.as_ref(), mock_env(), query_msg.clone()).unwrap();
        let value: Vec<RateLimit> = from_binary(&res).unwrap();
        assert_eq!(value.len(), 1);

        verify_query_response(
            &value[0],
            "different",
            (50, 30),
            5000,
            0_u32.into(),
            0_u32.into(),
            env.block.time.plus_seconds(5000),
        );
    }
    #[test]
    fn test_intent_ok() {
        let now = Timestamp::from_seconds(1700383122);
        let unlock = now.plus_seconds(86400);
        let then = now.plus_seconds(200);
        let amount = Uint256::from_u128(1_000_000);

        // ensure that timestamp fails
        assert!(!intent_ok((amount, unlock), then, amount));
        
        let then = now.plus_seconds(86400);

        // ensure amount fails
        assert!(!intent_ok((amount, now), then, Uint256::from_u128(1)));

        // amount and timestamp ok so should return true
        assert!(intent_ok((amount, now), then, amount))

    }
}
