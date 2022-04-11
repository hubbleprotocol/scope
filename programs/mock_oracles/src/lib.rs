use anchor_lang::prelude::*;
pub mod pc;

use pc::{Price, PriceStatus};
use switchboard_program::{
    mod_AggregatorState, AggregatorState, RoundResult, SwitchboardAccountType,
};

use borsh::{BorshDeserialize, BorshSerialize};
use quick_protobuf::deserialize_from_slice;
use quick_protobuf::serialize_into_slice;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array(include!(concat!(env!("OUT_DIR"), "/pubkey.rs")));

declare_id!(PROGRAM_ID);

#[program]
pub mod mock_oracles {

    use std::convert::TryInto;
    use std::ops::Div;

    use switchboard_v2::AggregatorAccountData;

    use super::*;
    use switchboard_v2::decimal::SwitchboardDecimal;
    pub fn initialize(ctx: Context<Initialize>, price: i64, expo: i32, conf: u64) -> ProgramResult {
        let oracle = &ctx.accounts.price;

        let mut price_oracle = Price::load(oracle).unwrap();

        price_oracle.agg.status = PriceStatus::Trading;
        price_oracle.agg.price = price;
        price_oracle.agg.conf = conf;
        price_oracle.twap.val = price;
        price_oracle.twac.val = conf.try_into().unwrap();
        price_oracle.expo = expo;
        price_oracle.ptype = pc::PriceType::Price;
        price_oracle.num_qt = 3;
        price_oracle.magic = 0xa1b2c3d4;
        price_oracle.ver = 2;

        let slot = ctx.accounts.clock.slot;
        price_oracle.valid_slot = slot;

        msg!(
            "Price {} initialized to {}, expo {}, conf {} at slot {}",
            oracle.key,
            price,
            expo,
            conf,
            slot
        );

        Ok(())
    }

    pub fn initialize_switchboard_v1(
        ctx: Context<Initialize>,
        mantissa: i128,
        scale: u32,
    ) -> ProgramResult {
        let mut account_data = ctx.accounts.price.data.borrow_mut();
        account_data[0] = SwitchboardAccountType::TYPE_AGGREGATOR as u8;

        let configs = Some(mod_AggregatorState::Configs {
            min_confirmations: Some(3),
            ..mod_AggregatorState::Configs::default()
        });
        let mantissa_f64 = mantissa as f64;
        let denominator = (10u128.pow(scale)) as f64;
        let price = mantissa_f64.div(denominator);
        let slot = ctx.accounts.clock.slot;
        let last_round_result = Some(RoundResult {
            num_success: Some(3),
            result: Some(price),
            round_open_slot: Some(slot),
            ..RoundResult::default()
        });
        let aggregator_state = AggregatorState {
            last_round_result,
            configs,
            ..AggregatorState::default()
        };
        serialize_into_slice(&aggregator_state, &mut account_data[1..]).unwrap();
        let key = &ctx.accounts.price.key.to_string();
        msg!("Switchboard V1 price {} initialized at slot {}", key, slot);

        Ok(())
    }

    pub fn initialize_switchboard_v2(
        ctx: Context<Initialize>,
        mantissa: i128,
        scale: u32,
    ) -> ProgramResult {
        let mut account_data = ctx.accounts.price.data.borrow_mut();
        const DISCRIMINATOR: [u8; 8] = [217, 230, 65, 101, 201, 162, 27, 125];
        account_data[..8].copy_from_slice(&DISCRIMINATOR);
        let aggregator_account_data: &mut AggregatorAccountData =
            bytemuck::from_bytes_mut(&mut account_data[8..]);
        aggregator_account_data.latest_confirmed_round.result =
            SwitchboardDecimal::new(mantissa, scale);
        let slot = ctx.accounts.clock.slot;
        aggregator_account_data
            .latest_confirmed_round
            .round_open_slot = slot;
        aggregator_account_data.latest_confirmed_round.num_success = 3;
        aggregator_account_data.min_oracle_results = 3;
        let key = &ctx.accounts.price.key.to_string();
        msg!("Switchboard V2 price {} initialized at slot {}", key, slot);
        Ok(())
    }

    pub fn set_price_pyth(ctx: Context<SetPrice>, price: i64) -> ProgramResult {
        let oracle = &ctx.accounts.price;

        let mut price_oracle = Price::load(oracle).unwrap();
        price_oracle.agg.price = price;

        let slot = ctx.accounts.clock.slot;
        price_oracle.valid_slot = slot;
        msg!("Price {} updated to {} at slot {}", oracle.key, price, slot);
        Ok(())
    }

    pub fn set_price_switchboard_v1(
        ctx: Context<SetPrice>,
        mantissa: i128,
        scale: u32,
    ) -> ProgramResult {
        let mut account_data = ctx.accounts.price.data.borrow_mut();
        let mut aggregator_state: AggregatorState =
            deserialize_from_slice(&account_data[1..]).unwrap();
        let mantissa_f64 = mantissa as f64;
        let denominator = (10u128.pow(scale)) as f64;
        let price = mantissa_f64.div(denominator);
        let mut last_round_result = aggregator_state.last_round_result.unwrap();
        last_round_result.result = Some(price);
        let slot = ctx.accounts.clock.slot;
        last_round_result.round_open_slot = Some(slot);
        aggregator_state.last_round_result = Some(last_round_result);
        serialize_into_slice(&aggregator_state, &mut account_data[1..]).unwrap();
        let key = &ctx.accounts.price.key.to_string();
        msg!("Switchboard V1 Price {} updated to at slot {}", key, slot);

        Ok(())
    }

    pub fn set_price_switchboard_v2(
        ctx: Context<SetPrice>,
        mantissa: i128,
        scale: u32,
    ) -> ProgramResult {
        let mut account_data = ctx.accounts.price.data.borrow_mut();
        let aggregator_account_data: &mut AggregatorAccountData =
            bytemuck::from_bytes_mut(&mut account_data[8..]);
        aggregator_account_data.latest_confirmed_round.result =
            SwitchboardDecimal::new(mantissa, scale);
        let slot = ctx.accounts.clock.slot;
        aggregator_account_data
            .latest_confirmed_round
            .round_open_slot = slot;
        let key = &ctx.accounts.price.key.to_string();
        msg!("Switchboard V2 Price {} updated at slot {}", key, slot);

        Ok(())
    }

    pub fn set_trading_pyth(ctx: Context<SetPrice>, status: u8) -> ProgramResult {
        let oracle = &ctx.accounts.price;
        let mut price_oracle = Price::load(oracle).unwrap();
        match status {
            0 => price_oracle.agg.status = PriceStatus::Unknown,
            1 => price_oracle.agg.status = PriceStatus::Trading,
            2 => price_oracle.agg.status = PriceStatus::Halted,
            3 => price_oracle.agg.status = PriceStatus::Auction,
            _ => {
                msg!("Unknown status: {}", status);
                return Err(ProgramError::Custom(1559));
            }
        }
        Ok(())
    }
    pub fn set_twap_pyth(ctx: Context<SetPrice>, value: u64) -> ProgramResult {
        let oracle = &ctx.accounts.price;
        let mut price_oracle = Price::load(oracle).unwrap();
        price_oracle.twap.val = value.try_into().unwrap();

        Ok(())
    }
    pub fn set_confidence_pyth(ctx: Context<SetPrice>, value: u64) -> ProgramResult {
        let oracle = &ctx.accounts.price;
        let mut price_oracle = Price::load(oracle).unwrap();
        price_oracle.agg.conf = value;

        Ok(())
    }
}
#[derive(Accounts)]
pub struct SetPrice<'info> {
    /// CHECK: Not safe but this is a test tool
    #[account(mut)]
    pub price: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}
#[derive(Accounts)]
pub struct Initialize<'info> {
    /// CHECK: Not safe but this is a test tool
    #[account(mut)]
    pub price: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}
