#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, Event, MessageInfo, Response, StdError, StdResult};
use cw2::{get_contract_version, set_contract_version, ContractVersion};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SudoMsg};
use crate::state::{FlowType, GOVMODULE, IBCMODULE};
use crate::{execute, query, sudo};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:rate-limiter";

const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // set to lowest possible version number for testing
    #[cfg(test)]
    set_contract_version(deps.storage, CONTRACT_NAME, "0.0.1")?;

    #[cfg(not(test))]
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    IBCMODULE.save(deps.storage, &msg.ibc_module)?;
    GOVMODULE.save(deps.storage, &msg.gov_module)?;

    execute::add_new_paths(deps, msg.paths, env.block.time)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("ibc_module", msg.ibc_module.to_string())
        .add_attribute("gov_module", msg.gov_module.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddPath {
            channel_id,
            denom,
            quotas,
        } => execute::try_add_path(deps, info.sender, channel_id, denom, quotas, env.block.time),
        ExecuteMsg::RemovePath { channel_id, denom } => {
            execute::try_remove_path(deps, info.sender, channel_id, denom)
        }
        ExecuteMsg::ResetPathQuota {
            channel_id,
            denom,
            quota_id,
        } => execute::try_reset_path_quota(
            deps,
            info.sender,
            channel_id,
            denom,
            quota_id,
            env.block.time,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::SendPacket {
            packet,
            #[cfg(test)]
            channel_value_mock,
        } => sudo::process_packet(
            deps,
            packet,
            FlowType::Out,
            env.block.time,
            #[cfg(test)]
            channel_value_mock,
        ),
        SudoMsg::RecvPacket {
            packet,
            #[cfg(test)]
            channel_value_mock,
        } => sudo::process_packet(
            deps,
            packet,
            FlowType::In,
            env.block.time,
            #[cfg(test)]
            channel_value_mock,
        ),
        SudoMsg::UndoSend { packet } => sudo::undo_send(deps, packet),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetQuotas { channel_id, denom } => query::get_quotas(deps, channel_id, denom),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let c_version = get_contract_version(deps.storage)?;
    if &c_version.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same type").into());
    }
    let stored_version: semver::Version = c_version.version.parse()?;

    // logic that should only run in testing (handle valid migration)
    #[cfg(test)]
    let code_version =
        if stored_version.major == 0 && stored_version.minor == 0 && stored_version.patch == 1 {
            let code_version: semver::Version = "0.0.2".parse()?;
            code_version
        } else {
            let code_version: semver::Version = "0.0.2".parse()?;
            code_version
        };

    // when not testing, derive code version from the global variable
    #[cfg(not(test))]
    let code_version: semver::Version = CONTRACT_VERSION.parse()?;

    if stored_version < code_version {
        // update contract version
        set_contract_version(deps.storage, CONTRACT_NAME, code_version.to_string())?;
        // handle storage migrations
        Ok(Response::default().add_event(
            Event::new("migration_ok")
            .add_attribute("old_version", stored_version.to_string())
            .add_attribute("new_version", code_version.to_string()),
        ))
    } else {
        return Err(StdError::generic_err("Can't upgrade from a newer version").into());
    }
}
