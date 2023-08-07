use cosmwasm_std::{DepsMut, Env, Event, Response, StdError};
use cw2::{get_contract_version, set_contract_version, ContractVersion};

use crate::{contract::CONTRACT_NAME, msg::MigrateMsg, ContractError, state::{RATE_LIMIT_TRACKERS, RateLimitV2, QuotaV2, RATE_LIMIT_TRACKERS_V2}};

pub(crate) fn v1_migrate(
    stored_version: semver::Version,
    code_version: semver::Version,
    deps: DepsMut,
    env: Env,
    msg: MigrateMsg,
) -> Result<Response, ContractError> {


    let rate_limit_rules = RATE_LIMIT_TRACKERS.keys(deps.storage, None, None, cosmwasm_std::Order::Ascending).flat_map(|k| {
        if let Ok(k) = k {
            if let Ok(res) = RATE_LIMIT_TRACKERS.load(deps.storage, k.clone()) {
                Some((k, res))
            } else {
                None
            }
        } else {
            None
        }
    }).collect::<Vec<_>>();

    // nuke the old data
    RATE_LIMIT_TRACKERS.clear(deps.storage);

    let mut rules_migrated = 0;

    for (k, rules) in rate_limit_rules {
        let rules = rules.into_iter().map(|rule| RateLimitV2 {
            quota: QuotaV2 {
                max_percentage_recv: rule.quota.max_percentage_recv,
                max_percentage_send: rule.quota.max_percentage_send,
                channel_value: rule.quota.channel_value,
            },
            flow: rule.flow
        }).collect::<Vec<RateLimitV2>>();
        rules_migrated += rules.len() as u64;
        RATE_LIMIT_TRACKERS_V2.save(deps.storage, k, &rules)?;
    }

    Ok(Response::default().add_event(
        Event::new("migration_ok")
            .add_attribute("migration_version", "v1")
            .add_attribute("old_version", stored_version.to_string())
            .add_attribute("rules_migrated", rules_migrated.to_string())
            .add_attribute("new_version", code_version.to_string()),
    ))
}
