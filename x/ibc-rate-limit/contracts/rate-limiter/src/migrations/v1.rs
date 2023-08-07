use cosmwasm_std::{DepsMut, Env, Event, Response, StdError};
use cw2::{get_contract_version, set_contract_version, ContractVersion};

use crate::{contract::CONTRACT_NAME, msg::MigrateMsg, ContractError, state::{RATE_LIMIT_TRACKERS, RateLimitV2, QuotaV2,  RateLimitType, RATE_LIMIT_TRACKERS_LEGACY}};

// migrates storage from RateLimit to RateLimitType, allowing for adding additional rate limit variants
// without more complex migrations
pub(crate) fn v1_migrate(
    stored_version: semver::Version,
    code_version: semver::Version,
    deps: DepsMut,
    env: Env,
    msg: MigrateMsg,
) -> Result<Response, ContractError> {


    // load the legacy format
    let rate_limit_rules = RATE_LIMIT_TRACKERS_LEGACY.range(deps.storage, None, None, cosmwasm_std::Order::Ascending).flatten().collect::<Vec<_>>();




    let mut rules_migrated = 0;
    rate_limit_rules.into_iter().for_each(|(key, rules)| {
        let rules = rules.into_iter().map(|rule| RateLimitType::from(rule)).collect::<Vec<_>>();
        rules_migrated += rules.len() as u64;
        // override the namespace entry with the new format
        RATE_LIMIT_TRACKERS.save(deps.storage, key, &rules).unwrap();
    });

    Ok(Response::default().add_event(
        Event::new("migration_ok")
            .add_attribute("migration_version", "v1")
            .add_attribute("old_version", stored_version.to_string())
            .add_attribute("rules_migrated", rules_migrated.to_string())
            .add_attribute("new_version", code_version.to_string()),
    ))
}
