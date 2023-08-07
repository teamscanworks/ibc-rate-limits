use cosmwasm_std::{DepsMut, Env, Event, Response, StdError};
use cw2::{get_contract_version, set_contract_version, ContractVersion};

use crate::{contract::CONTRACT_NAME, msg::MigrateMsg, ContractError};

pub(crate) fn v1_migrate(
    stored_version: semver::Version,
    code_version: semver::Version,
    deps: DepsMut,
    env: Env,
    msg: MigrateMsg,
) -> Result<Response, ContractError> {
    Ok(Response::default().add_event(
        Event::new("migration_ok")
            .add_attribute("migration_version", "v1")
            .add_attribute("old_version", stored_version.to_string())
            .add_attribute("new_version", code_version.to_string()),
    ))
}
