use anchor_lang::prelude::*;

use crate::UpdateOracleMappingMode;

use super::handler_update_mapping::UpdateOracleMapping;

pub fn process(
    ctx: Context<UpdateOracleMapping>,
    token: usize,
    mode: u16,
    value: u16,
    _: String,
) -> Result<()> {
    let mut oracle_mappings = ctx.accounts.oracle_mappings.load_mut()?;

    let mode: UpdateOracleMappingMode = mode.try_into().unwrap();
    match mode {
        UpdateOracleMappingMode::TwapSource => {
            oracle_mappings.twap_source[usize::from(token)] = value
        }
        UpdateOracleMappingMode::UseTwap => {
            oracle_mappings.twap_enabled[usize::from(token)] = value.try_into().unwrap()
        }
    }

    Ok(())
}
