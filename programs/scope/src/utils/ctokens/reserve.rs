use super::last_update::LastUpdate;
use super::math::{Decimal, Rate, TryAdd, TryDiv, TryMul, WAD};
use anchor_lang::solana_program::{
    clock::Slot,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
};

use super::*;

use std::result::Result;

/// Collateral tokens are initially valued at a ratio of 5:1 (collateral:liquidity)
// @FIXME: restore to 5
pub const INITIAL_COLLATERAL_RATIO: u64 = 1;
const INITIAL_COLLATERAL_RATE: u64 = INITIAL_COLLATERAL_RATIO * WAD;

/// Current version of the program and all new accounts created
pub const PROGRAM_VERSION: u8 = 1;

/// Accounts are created with data zeroed out, so uninitialized state instances
/// will have the version set to 0.
pub const UNINITIALIZED_VERSION: u8 = 0;

/// Lending market reserve state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reserve {
    /// Version of the struct
    pub version: u8,
    /// Last slot when supply and rates updated
    pub last_update: LastUpdate,
    /// Lending market address
    pub lending_market: Pubkey,
    /// Reserve liquidity
    pub liquidity: ReserveLiquidity,
    /// Reserve collateral
    pub collateral: ReserveCollateral,
    /// Reserve configuration values
    pub config: ReserveConfig,
}

impl Reserve {
    /// Collateral exchange rate
    pub fn collateral_exchange_rate(&self) -> Result<CollateralExchangeRate, ProgramError> {
        let total_liquidity = self.liquidity.total_supply()?;
        self.collateral.exchange_rate(total_liquidity)
    }
}

/// Calculate borrow result
#[derive(Debug)]
pub struct CalculateBorrowResult {
    /// Total amount of borrow including fees
    pub borrow_amount: Decimal,
    /// Borrow amount portion of total amount
    pub receive_amount: u64,
    /// Loan origination fee
    pub borrow_fee: u64,
    /// Host fee portion of origination fee
    pub host_fee: u64,
}

/// Calculate repay result
#[derive(Debug)]
pub struct CalculateRepayResult {
    /// Amount of liquidity that is settled from the obligation.
    pub settle_amount: Decimal,
    /// Amount that will be repaid as u64
    pub repay_amount: u64,
}

/// Calculate liquidation result
#[derive(Debug)]
pub struct CalculateLiquidationResult {
    /// Amount of liquidity that is settled from the obligation. It includes
    /// the amount of loan that was defaulted if collateral is depleted.
    pub settle_amount: Decimal,
    /// Amount that will be repaid as u64
    pub repay_amount: u64,
    /// Amount of collateral to withdraw in exchange for repay amount
    pub withdraw_amount: u64,
}

/// Reserve liquidity
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReserveLiquidity {
    /// Reserve liquidity mint address
    pub mint_pubkey: Pubkey,
    /// Reserve liquidity mint decimals
    pub mint_decimals: u8,
    /// Reserve liquidity supply address
    pub supply_pubkey: Pubkey,
    /// Reserve liquidity pyth oracle account
    pub pyth_oracle_pubkey: Pubkey,
    /// Reserve liquidity switchboard oracle account
    pub switchboard_oracle_pubkey: Pubkey,
    /// Reserve liquidity available
    pub available_amount: u64,
    /// Reserve liquidity borrowed
    pub borrowed_amount_wads: Decimal,
    /// Reserve liquidity cumulative borrow rate
    pub cumulative_borrow_rate_wads: Decimal,
    /// Reserve liquidity market price in quote currency
    pub market_price: Decimal,
}

impl ReserveLiquidity {
    /// Calculate the total reserve supply including active loans
    pub fn total_supply(&self) -> Result<Decimal, ProgramError> {
        Decimal::from(self.available_amount).try_add(self.borrowed_amount_wads)
    }
}

/// Reserve collateral
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReserveCollateral {
    /// Reserve collateral mint address
    pub mint_pubkey: Pubkey,
    /// Reserve collateral mint supply, used for exchange rate
    pub mint_total_supply: u64,
    /// Reserve collateral supply address
    pub supply_pubkey: Pubkey,
}

impl ReserveCollateral {
    /// Return the current collateral exchange rate.
    fn exchange_rate(
        &self,
        total_liquidity: Decimal,
    ) -> Result<CollateralExchangeRate, ProgramError> {
        let rate = if self.mint_total_supply == 0 || total_liquidity == Decimal::zero() {
            Rate::from_scaled_val(INITIAL_COLLATERAL_RATE)
        } else {
            let mint_total_supply = Decimal::from(self.mint_total_supply);
            Rate::try_from(mint_total_supply.try_div(total_liquidity)?)?
        };

        Ok(CollateralExchangeRate(rate))
    }
}

/// Collateral exchange rate
#[derive(Clone, Copy, Debug)]
pub struct CollateralExchangeRate(Rate);

impl CollateralExchangeRate {
    /// Convert reserve collateral to liquidity
    pub fn collateral_to_liquidity(&self, collateral_amount: u64) -> Result<u64, ProgramError> {
        self.decimal_collateral_to_liquidity(collateral_amount.into())?
            .try_floor_u64()
    }

    /// Convert reserve collateral to liquidity
    pub fn decimal_collateral_to_liquidity(
        &self,
        collateral_amount: Decimal,
    ) -> Result<Decimal, ProgramError> {
        collateral_amount.try_div(self.0)
    }

    /// Convert reserve liquidity to collateral
    pub fn liquidity_to_collateral(&self, liquidity_amount: u64) -> Result<u64, ProgramError> {
        self.decimal_liquidity_to_collateral(liquidity_amount.into())?
            .try_floor_u64()
    }

    /// Convert reserve liquidity to collateral
    pub fn decimal_liquidity_to_collateral(
        &self,
        liquidity_amount: Decimal,
    ) -> Result<Decimal, ProgramError> {
        liquidity_amount.try_mul(self.0)
    }
}

impl From<CollateralExchangeRate> for Rate {
    fn from(exchange_rate: CollateralExchangeRate) -> Self {
        exchange_rate.0
    }
}

/// Reserve configuration values
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReserveConfig {
    /// Optimal utilization rate, as a percentage
    pub optimal_utilization_rate: u8,
    /// Target ratio of the value of borrows to deposits, as a percentage
    /// 0 if use as collateral is disabled
    pub loan_to_value_ratio: u8,
    /// Bonus a liquidator gets when repaying part of an unhealthy obligation, as a percentage
    pub liquidation_bonus: u8,
    /// Loan to value ratio at which an obligation can be liquidated, as a percentage
    pub liquidation_threshold: u8,
    /// Min borrow APY
    pub min_borrow_rate: u8,
    /// Optimal (utilization) borrow APY
    pub optimal_borrow_rate: u8,
    /// Max borrow APY
    pub max_borrow_rate: u8,
    /// Program owner fees assessed, separate from gains due to interest accrual
    pub fees: ReserveFees,
    /// Maximum deposit limit of liquidity in native units, u64::MAX for inf
    pub deposit_limit: u64,
    /// Borrows disabled
    pub borrow_limit: u64,
    /// Reserve liquidity fee receiver address
    pub fee_receiver: Pubkey,
    /// Cut of the liquidation bonus that the protocol receives, as a percentage
    pub protocol_liquidation_fee: u8,
}

/// Additional fee information on a reserve
///
/// These exist separately from interest accrual fees, and are specifically for the program owner
/// and frontend host. The fees are paid out as a percentage of liquidity token amounts during
/// repayments and liquidations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReserveFees {
    /// Fee assessed on `BorrowObligationLiquidity`, expressed as a Wad.
    /// Must be between 0 and 10^18, such that 10^18 = 1.  A few examples for
    /// clarity:
    /// 1% = 10_000_000_000_000_000
    /// 0.01% (1 basis point) = 100_000_000_000_000
    /// 0.00001% (Aave borrow fee) = 100_000_000_000
    pub borrow_fee_wad: u64,
    /// Fee for flash loan, expressed as a Wad.
    /// 0.3% (Aave flash loan fee) = 3_000_000_000_000_000
    pub flash_loan_fee_wad: u64,
    /// Amount of fee going to host account, if provided in liquidate and repay
    pub host_fee_percentage: u8,
}

impl Sealed for Reserve {}
impl IsInitialized for Reserve {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const RESERVE_LEN: usize = 619; // 1 + 8 + 1 + 32 + 32 + 1 + 32 + 32 + 32 + 8 + 16 + 16 + 16 + 32 + 8 + 32 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 8 + 8 + 1 + 8 + 8 + 32 + 1 + 247
impl Pack for Reserve {
    const LEN: usize = RESERVE_LEN;

    // @TODO: break this up by reserve / liquidity / collateral / config https://git.io/JOCca
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update_slot,
            last_update_stale,
            lending_market,
            liquidity_mint_pubkey,
            liquidity_mint_decimals,
            liquidity_supply_pubkey,
            liquidity_pyth_oracle_pubkey,
            liquidity_switchboard_oracle_pubkey,
            liquidity_available_amount,
            liquidity_borrowed_amount_wads,
            liquidity_cumulative_borrow_rate_wads,
            liquidity_market_price,
            collateral_mint_pubkey,
            collateral_mint_total_supply,
            collateral_supply_pubkey,
            config_optimal_utilization_rate,
            config_loan_to_value_ratio,
            config_liquidation_bonus,
            config_liquidation_threshold,
            config_min_borrow_rate,
            config_optimal_borrow_rate,
            config_max_borrow_rate,
            config_fees_borrow_fee_wad,
            config_fees_flash_loan_fee_wad,
            config_fees_host_fee_percentage,
            config_deposit_limit,
            config_borrow_limit,
            config_fee_receiver,
            config_protocol_liquidation_fee,
            _padding,
        ) = mut_array_refs![
            output,
            1,
            8,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            16,
            16,
            16,
            PUBKEY_BYTES,
            8,
            PUBKEY_BYTES,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            8,
            8,
            1,
            8,
            8,
            PUBKEY_BYTES,
            1,
            247
        ];

        // reserve
        *version = self.version.to_le_bytes();
        *last_update_slot = self.last_update.slot.to_le_bytes();
        pack_bool(self.last_update.stale, last_update_stale);
        lending_market.copy_from_slice(self.lending_market.as_ref());

        // liquidity
        liquidity_mint_pubkey.copy_from_slice(self.liquidity.mint_pubkey.as_ref());
        *liquidity_mint_decimals = self.liquidity.mint_decimals.to_le_bytes();
        liquidity_supply_pubkey.copy_from_slice(self.liquidity.supply_pubkey.as_ref());
        liquidity_pyth_oracle_pubkey.copy_from_slice(self.liquidity.pyth_oracle_pubkey.as_ref());
        liquidity_switchboard_oracle_pubkey
            .copy_from_slice(self.liquidity.switchboard_oracle_pubkey.as_ref());
        *liquidity_available_amount = self.liquidity.available_amount.to_le_bytes();
        pack_decimal(
            self.liquidity.borrowed_amount_wads,
            liquidity_borrowed_amount_wads,
        );
        pack_decimal(
            self.liquidity.cumulative_borrow_rate_wads,
            liquidity_cumulative_borrow_rate_wads,
        );
        pack_decimal(self.liquidity.market_price, liquidity_market_price);

        // collateral
        collateral_mint_pubkey.copy_from_slice(self.collateral.mint_pubkey.as_ref());
        *collateral_mint_total_supply = self.collateral.mint_total_supply.to_le_bytes();
        collateral_supply_pubkey.copy_from_slice(self.collateral.supply_pubkey.as_ref());

        // config
        *config_optimal_utilization_rate = self.config.optimal_utilization_rate.to_le_bytes();
        *config_loan_to_value_ratio = self.config.loan_to_value_ratio.to_le_bytes();
        *config_liquidation_bonus = self.config.liquidation_bonus.to_le_bytes();
        *config_liquidation_threshold = self.config.liquidation_threshold.to_le_bytes();
        *config_min_borrow_rate = self.config.min_borrow_rate.to_le_bytes();
        *config_optimal_borrow_rate = self.config.optimal_borrow_rate.to_le_bytes();
        *config_max_borrow_rate = self.config.max_borrow_rate.to_le_bytes();
        *config_fees_borrow_fee_wad = self.config.fees.borrow_fee_wad.to_le_bytes();
        *config_fees_flash_loan_fee_wad = self.config.fees.flash_loan_fee_wad.to_le_bytes();
        *config_fees_host_fee_percentage = self.config.fees.host_fee_percentage.to_le_bytes();
        *config_deposit_limit = self.config.deposit_limit.to_le_bytes();
        *config_borrow_limit = self.config.borrow_limit.to_le_bytes();
        config_fee_receiver.copy_from_slice(self.config.fee_receiver.as_ref());
        *config_protocol_liquidation_fee = self.config.protocol_liquidation_fee.to_le_bytes();
    }

    /// Unpacks a byte buffer into a [ReserveInfo](struct.ReserveInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update_slot,
            last_update_stale,
            lending_market,
            liquidity_mint_pubkey,
            liquidity_mint_decimals,
            liquidity_supply_pubkey,
            liquidity_pyth_oracle_pubkey,
            liquidity_switchboard_oracle_pubkey,
            liquidity_available_amount,
            liquidity_borrowed_amount_wads,
            liquidity_cumulative_borrow_rate_wads,
            liquidity_market_price,
            collateral_mint_pubkey,
            collateral_mint_total_supply,
            collateral_supply_pubkey,
            config_optimal_utilization_rate,
            config_loan_to_value_ratio,
            config_liquidation_bonus,
            config_liquidation_threshold,
            config_min_borrow_rate,
            config_optimal_borrow_rate,
            config_max_borrow_rate,
            config_fees_borrow_fee_wad,
            config_fees_flash_loan_fee_wad,
            config_fees_host_fee_percentage,
            config_deposit_limit,
            config_borrow_limit,
            config_fee_receiver,
            config_protocol_liquidation_fee,
            _padding,
        ) = array_refs![
            input,
            1,
            8,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            16,
            16,
            16,
            PUBKEY_BYTES,
            8,
            PUBKEY_BYTES,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            8,
            8,
            1,
            8,
            8,
            PUBKEY_BYTES,
            1,
            247
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("Reserve version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            version,
            last_update: LastUpdate {
                slot: u64::from_le_bytes(*last_update_slot),
                stale: unpack_bool(last_update_stale)?,
            },
            lending_market: Pubkey::new_from_array(*lending_market),
            liquidity: ReserveLiquidity {
                mint_pubkey: Pubkey::new_from_array(*liquidity_mint_pubkey),
                mint_decimals: u8::from_le_bytes(*liquidity_mint_decimals),
                supply_pubkey: Pubkey::new_from_array(*liquidity_supply_pubkey),
                pyth_oracle_pubkey: Pubkey::new_from_array(*liquidity_pyth_oracle_pubkey),
                switchboard_oracle_pubkey: Pubkey::new_from_array(
                    *liquidity_switchboard_oracle_pubkey,
                ),
                available_amount: u64::from_le_bytes(*liquidity_available_amount),
                borrowed_amount_wads: unpack_decimal(liquidity_borrowed_amount_wads),
                cumulative_borrow_rate_wads: unpack_decimal(liquidity_cumulative_borrow_rate_wads),
                market_price: unpack_decimal(liquidity_market_price),
            },
            collateral: ReserveCollateral {
                mint_pubkey: Pubkey::new_from_array(*collateral_mint_pubkey),
                mint_total_supply: u64::from_le_bytes(*collateral_mint_total_supply),
                supply_pubkey: Pubkey::new_from_array(*collateral_supply_pubkey),
            },
            config: ReserveConfig {
                optimal_utilization_rate: u8::from_le_bytes(*config_optimal_utilization_rate),
                loan_to_value_ratio: u8::from_le_bytes(*config_loan_to_value_ratio),
                liquidation_bonus: u8::from_le_bytes(*config_liquidation_bonus),
                liquidation_threshold: u8::from_le_bytes(*config_liquidation_threshold),
                min_borrow_rate: u8::from_le_bytes(*config_min_borrow_rate),
                optimal_borrow_rate: u8::from_le_bytes(*config_optimal_borrow_rate),
                max_borrow_rate: u8::from_le_bytes(*config_max_borrow_rate),
                fees: ReserveFees {
                    borrow_fee_wad: u64::from_le_bytes(*config_fees_borrow_fee_wad),
                    flash_loan_fee_wad: u64::from_le_bytes(*config_fees_flash_loan_fee_wad),
                    host_fee_percentage: u8::from_le_bytes(*config_fees_host_fee_percentage),
                },
                deposit_limit: u64::from_le_bytes(*config_deposit_limit),
                borrow_limit: u64::from_le_bytes(*config_borrow_limit),
                fee_receiver: Pubkey::new_from_array(*config_fee_receiver),
                protocol_liquidation_fee: u8::from_le_bytes(*config_protocol_liquidation_fee),
            },
        })
    }
}
