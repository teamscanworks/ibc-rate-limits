use cosmwasm_std::{DepsMut, Env, Event, Response, StdError};
use cw2::{get_contract_version, set_contract_version, ContractVersion};

use crate::{contract::CONTRACT_NAME, msg::MigrateMsg, ContractError};

pub mod v1;

pub(crate) fn migrate_internal(
    deps: DepsMut,
    env: Env,
    msg: MigrateMsg,
) -> Result<Response, ContractError> {
    let c_version = get_contract_version(deps.storage)?;
    if &c_version.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same type").into());
    }
    let stored_version: semver::Version = c_version.version.parse()?;

    #[cfg(test)]
    let code_version = get_code_version(stored_version.clone());
    #[cfg(not(test))]
    let code_version = get_code_version();

    if stored_version < code_version {
        // update version
        set_contract_version(deps.storage, CONTRACT_NAME, code_version.to_string())?;

        if stored_version.major == 0 && stored_version.minor == 1 && stored_version.patch == 0 {
            v1::v1_migrate(stored_version, code_version, deps, env, msg)
        } else {
            return Err(StdError::generic_err("Missing migrate function").into());
        }
    } else {
        return Err(StdError::generic_err("Can't upgrade from a newer version").into());
    }
}

// intended for testing purposes only
#[cfg(test)]
fn get_code_version(stored_version: semver::Version) -> semver::Version {
    if stored_version.major == 0 && stored_version.minor == 1 && stored_version.patch == 0 {
        let code_version: semver::Version = "0.1.1".parse().unwrap();
        code_version
    } else {
        let code_version: semver::Version = "0.1.1".parse().unwrap();
        code_version
    }
}

// returns the version of the contract as defined in the codebase
#[cfg(not(test))]
fn get_code_version() -> semver::Version {
    use crate::contract::CONTRACT_VERSION;

    CONTRACT_VERSION.parse().unwrap()
}
