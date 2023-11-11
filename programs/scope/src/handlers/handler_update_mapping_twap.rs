use anchor_lang::prelude::*;

use crate::{
    oracles::{check_context, validate_oracle_account, OracleType},
    OracleMappings, ScopeError, UpdateOracleMappingMode,
};

use super::handler_update_mapping::UpdateOracleMapping;

pub fn process(
    ctx: Context<UpdateOracleMapping>,
    token_index: u16,
    mode: u16,
    value: u16,
) -> Result<()> {
    let mut oracle_mappings = ctx.accounts.oracle_mappings.load_mut()?;

    let mode: UpdateOracleMappingMode = mode.try_into().unwrap();
    match mode {
        UpdateOracleMappingMode::TwapSource => {
            oracle_mappings.twap_source[usize::from(token_index)] = value
        }
        UpdateOracleMappingMode::UseTwap => {
            oracle_mappings.use_twap[usize::from(token_index)] = value.try_into().unwrap()
        }
    }

    Ok(())
}
