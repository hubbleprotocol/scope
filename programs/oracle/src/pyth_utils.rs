#![allow(dead_code)]

use crate::Price;
use crate::Token;
use anchor_lang::prelude::*;
use pyth_client::{PriceStatus, PriceType};
use std::convert::{TryFrom, TryInto};

/// validate price confidence - confidence/price ratio should be less than 2%
const ORACLE_CONFIDENCE_FACTOR: u64 = 50; // 100% / 2%

pub fn get_price(pyth_price_info: &AccountInfo, token: Token) -> Result<Price> {
    let pyth_price_data = &pyth_price_info.try_borrow_data()?;
    let pyth_price = pyth_client::cast::<pyth_client::Price>(pyth_price_data);
    let price = validate_valid_price(pyth_price).map_err(|e| {
        msg!(
            "Invalid {:?} price on pyth account {}",
            token,
            pyth_price_info.key
        );
        e
    })?;

    Ok(Price {
        value: price,
        exp: pyth_price.expo.abs().try_into().unwrap(),
    })
}

fn validate_valid_price(pyth_price: &pyth_client::Price) -> Result<u64> {
    if cfg!(feature = "skip_price_validation") {
        return Ok(u64::try_from(pyth_price.agg.price).unwrap());
    }
    let is_trading = get_status(&pyth_price.agg.status);
    if !is_trading {
        return Err(BorrowError::PriceNotValid.into());
    }
    if pyth_price.num_qt < 3 {
        return Err(BorrowError::PriceNotValid.into());
    }

    let price = u64::try_from(pyth_price.agg.price).unwrap();
    if price == 0 {
        return Err(BorrowError::PriceNotValid.into());
    }
    let conf: u64 = pyth_price.agg.conf;
    let conf_50x: u64 = conf.checked_mul(ORACLE_CONFIDENCE_FACTOR).unwrap();
    if conf_50x > price {
        return Err(BorrowError::PriceNotValid.into());
    };
    Ok(price)
}

fn get_status(st: &PriceStatus) -> bool {
    matches!(st, PriceStatus::Trading)
}

pub fn validate_pyth_product(pyth_product: &pyth_client::Product) -> ProgramResult {
    if pyth_product.magic != pyth_client::MAGIC {
        msg!("Pyth product account provided is not a valid Pyth account");
        return Err(ProgramError::InvalidArgument);
    }
    if pyth_product.atype != pyth_client::AccountType::Product as u32 {
        msg!("Pyth product account provided is not a valid Pyth product account");
        return Err(ProgramError::InvalidArgument);
    }
    if pyth_product.ver != pyth_client::VERSION_2 {
        msg!("Pyth product account provided has a different version than the Pyth client");
        return Err(ProgramError::InvalidArgument);
    }
    if !pyth_product.px_acc.is_valid() {
        msg!("Pyth product price account is invalid");
        return Err(ProgramError::InvalidArgument);
    }

    Ok(())
}

pub fn validate_pyth_product_symbol(
    pyth_product: &pyth_client::Product,
    token: &Token,
) -> ProgramResult {
    match read_pyth_product_attribute(pyth_product, "symbol") {
        None => {
            msg!("Pyth product account does not contain symbol");
            return Err(ProgramError::InvalidArgument);
        }
        Some(product_symbol) => {
            let symbol_for_token = get_pyth_symbol_for_token(token);
            let symbol_for_token_dev = get_pyth_symbol_for_token_devnet(token);
            if product_symbol != symbol_for_token && product_symbol != symbol_for_token_dev {
                msg!("Pyth product account has invalid symbol. Expected: {} symbol for collateral token {:?}. Actual: {}", symbol_for_token, token, product_symbol);
                return Err(ProgramError::InvalidArgument);
            }
        }
    };
    Ok(())
}

pub fn validate_pyth_price_pubkey(
    pyth_product: &pyth_client::Product,
    pyth_price_pubkey: &Pubkey,
) -> ProgramResult {
    if pyth_product.px_acc.val[..] != pyth_price_pubkey.to_bytes() {
        msg!("Pyth product price account does not match the Pyth price account provided");
        return Err(ProgramError::InvalidArgument);
    }

    Ok(())
}

pub fn validate_pyth_price(pyth_price: &pyth_client::Price) -> ProgramResult {
    if pyth_price.magic != pyth_client::MAGIC {
        msg!("Pyth price account provided is not a valid Pyth account");
        return Err(ProgramError::InvalidArgument);
    }
    if !matches!(pyth_price.ptype, PriceType::Price) {
        msg!("Pyth price account provided has invalid price type");
        return Err(ProgramError::InvalidArgument);
    }
    if pyth_price.ver != pyth_client::VERSION_2 {
        msg!("Pyth price account provided has a different version than the Pyth client");
        return Err(ProgramError::InvalidArgument);
    }
    if !matches!(pyth_price.agg.status, PriceStatus::Trading) {
        msg!("Pyth price account provided is not active");
        return Err(ProgramError::InvalidArgument);
    }
    Ok(())
}

pub fn read_pyth_product_attribute(
    pyth_product: &pyth_client::Product,
    attribute: &str,
) -> Option<String> {
    let mut psz = pyth_product.size as usize - pyth_client::PROD_HDR_SIZE;
    let mut pit = (&pyth_product.attr[..]).iter();
    while psz > 0 {
        let key = get_attr_str(&mut pit);
        let val = get_attr_str(&mut pit);
        if key == attribute {
            return Some(val);
        }
        psz -= 2 + key.len() + val.len();
    }
    None
}

fn get_attr_str<'a, T>(ite: &mut T) -> String
where
    T: Iterator<Item = &'a u8>,
{
    let mut len = *ite.next().unwrap() as usize;
    let mut val = String::with_capacity(len);
    while len > 0 {
        val.push(*ite.next().unwrap() as char);
        len -= 1;
    }
    val
}

macro_rules! pyth_symbol {
    ($prefix: literal) => {
        format!("{}/USD", $prefix)
    };
}

pub fn get_pyth_symbol_for_token(token: &Token) -> String {
    match token {
        Token::SOL => pyth_symbol!("SOL"),
        Token::ETH => pyth_symbol!("ETH"),
        Token::BTC => pyth_symbol!("BTC"),
        Token::SRM => pyth_symbol!("SRM"),
        Token::RAY => pyth_symbol!("RAY"),
        Token::FTT => pyth_symbol!("FTT"),
        Token::MSOL => pyth_symbol!("MSOL"),
    }
}

pub fn get_pyth_symbol_for_token_devnet(token: &Token) -> String {
    match token {
        Token::SOL => pyth_symbol!("Crypto.SOL"),
        Token::ETH => pyth_symbol!("Crypto.ETH"),
        Token::BTC => pyth_symbol!("Crypto.BTC"),
        Token::SRM => pyth_symbol!("Crypto.SRM"),
        Token::RAY => pyth_symbol!("Crypto.RAY"),
        Token::FTT => pyth_symbol!("Crypto.FTT"),
        Token::MSOL => pyth_symbol!("Crypto.MSOL"),
    }
}

#[cfg(test)]
mod tests {
    use anchor_lang::prelude::{ProgramError, Pubkey};
    use pyth_client::AccKey;

    use super::utils::{new_product, new_product_attributes};
    use crate::Token;

    const PRICE_ACCT_SIZE: usize = 3312;

    const PRICE_MAGIC_OFFSET: usize = 0;
    const PRICE_VERSION_OFFSET: usize = 4;
    const PRICE_TYPE_OFFSET: usize = 16;
    const PRICE_STATUS_OFFSET: usize = 224;

    #[test]
    pub fn test_validate_product() {
        assert_eq!(
            super::validate_pyth_product(&new_product()).err().is_none(),
            true
        );
    }

    #[test]
    pub fn test_validate_product_magic_number() {
        let product = pyth_client::Product {
            magic: 0xa1b2c3d3, // incorrect magic
            ..new_product()
        };
        assert_eq!(
            super::validate_pyth_product(&product).err().unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn test_validate_product_account_type() {
        let product = pyth_client::Product {
            atype: pyth_client::AccountType::Mapping as u32, // incorrect atype
            ..new_product()
        };
        assert_eq!(
            super::validate_pyth_product(&product).err().unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn test_validate_product_version() {
        let product = pyth_client::Product {
            ver: 0_u32, // incorrect ver
            ..new_product()
        };
        assert_eq!(
            super::validate_pyth_product(&product).err().unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn test_validate_product_px_acc() {
        let product = pyth_client::Product {
            px_acc: pyth_client::AccKey {
                val: Pubkey::default().to_bytes(), // incorrect px_acc
            },
            ..new_product()
        };
        assert_eq!(
            super::validate_pyth_product(&product).err().unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn test_validate_product_symbol() {
        let product = pyth_client::Product {
            attr: new_product_attributes("symbol", "ETH/USD"),
            ..new_product()
        };
        assert_eq!(
            super::validate_pyth_product_symbol(&product, &Token::ETH)
                .err()
                .is_none(),
            true
        );
    }

    #[test]
    pub fn test_validate_product_symbol_incorrect() {
        let product = pyth_client::Product {
            attr: new_product_attributes("symbol", "BTC/USD"),
            ..new_product()
        };
        assert_eq!(
            super::validate_pyth_product_symbol(&product, &Token::ETH)
                .err()
                .unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn validate_pyth_price_pubkey() {
        let pubkey = Pubkey::new_unique();
        let product = pyth_client::Product {
            px_acc: AccKey {
                val: pubkey.to_bytes(),
            },
            ..new_product()
        };
        assert_eq!(
            super::validate_pyth_price_pubkey(&product, &pubkey)
                .err()
                .is_none(),
            true
        );
    }

    #[test]
    pub fn validate_pyth_price_pubkey_mismatch() {
        let pubkey = Pubkey::new_unique();
        let product = pyth_client::Product {
            px_acc: AccKey {
                val: pubkey.to_bytes(),
            },
            ..new_product()
        };
        assert_eq!(
            super::validate_pyth_price_pubkey(&product, &Pubkey::new_unique())
                .err()
                .unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn test_validate_price() {
        let buff = valid_price_bytes();
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_eq!(super::validate_pyth_price(price).err().is_none(), true);
    }

    #[test]
    pub fn test_validate_price_magic_incorrect() {
        let incorrect_magic = 0xa1b2c3d3_u32.to_le_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_magic, PRICE_MAGIC_OFFSET);
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_eq!(
            super::validate_pyth_price(price).err().unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn test_validate_price_price_type_incorrect() {
        let incorrect_price_type: &[u8] = &[0];
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_price_type, PRICE_TYPE_OFFSET);
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_eq!(
            super::validate_pyth_price(price).err().unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn test_validate_price_version_incorrect() {
        let incorrect_price_version = 1_u32.to_le_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_price_version, PRICE_VERSION_OFFSET);
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_eq!(
            super::validate_pyth_price(price).err().unwrap(),
            ProgramError::InvalidArgument
        );
    }

    #[test]
    pub fn test_validate_price_status_incorrect() {
        let incorrect_price_status = 0_u32.to_be_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_price_status, PRICE_STATUS_OFFSET);
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_eq!(
            super::validate_pyth_price(price).err().unwrap(),
            ProgramError::InvalidArgument
        );
    }

    fn valid_price_bytes() -> [u8; PRICE_ACCT_SIZE] {
        let mut buff = [0_u8; PRICE_ACCT_SIZE];
        write_bytes(
            &mut buff,
            &pyth_client::MAGIC.to_le_bytes(),
            PRICE_MAGIC_OFFSET,
        );
        write_bytes(
            &mut buff,
            &pyth_client::VERSION_2.to_le_bytes(),
            PRICE_VERSION_OFFSET,
        );
        write_bytes(&mut buff, &[1_u8], PRICE_TYPE_OFFSET); // price type = price
        write_bytes(&mut buff, &[1_u8], PRICE_STATUS_OFFSET); // price status = trading
        buff
    }

    fn write_bytes(buff: &mut [u8], bytes: &[u8], offset: usize) {
        for i in 0..bytes.len() {
            buff[i + offset] = bytes[i];
        }
    }
}

pub mod utils {

    use super::*;
    pub const PROD_ACCT_SIZE: usize = 512;
    pub const PROD_HDR_SIZE: usize = 48;
    pub const PROD_ATTR_SIZE: usize = PROD_ACCT_SIZE - PROD_HDR_SIZE;

    pub fn new_product() -> pyth_client::Product {
        pyth_client::Product {
            magic: pyth_client::MAGIC,
            ver: pyth_client::VERSION_2,
            atype: pyth_client::AccountType::Product as u32,
            size: u32::try_from(PROD_ACCT_SIZE).unwrap(),
            px_acc: pyth_client::AccKey {
                val: Pubkey::new_unique().to_bytes(),
            },
            attr: [0_u8; PROD_ATTR_SIZE],
        }
    }

    #[allow(clippy::same_item_push)]
    #[allow(clippy::integer_arithmetic)]
    pub fn new_product_attributes(key: &str, val: &str) -> [u8; PROD_ATTR_SIZE] {
        let key_bytes = key.as_bytes();
        let val_bytes = val.as_bytes();
        let mut zero_vec: Vec<u8> = Vec::with_capacity(PROD_ATTR_SIZE);
        // push the length discriminator
        zero_vec.push(key_bytes.len().try_into().unwrap());
        // push the value
        key_bytes.iter().for_each(|i| zero_vec.push(*i));
        // push the length discriminator
        zero_vec.push(val_bytes.len().try_into().unwrap());
        // push the value
        val_bytes.iter().for_each(|i| zero_vec.push(*i));
        // push zeroes

        for _ in 0..PROD_ATTR_SIZE - (1 + key_bytes.len() + 1 + val_bytes.len()) {
            zero_vec.push(0);
        }
        zero_vec.try_into().unwrap()
    }
}

#[error]
#[derive(PartialEq, Eq)]
pub enum BorrowError {
    #[msg("Insufficient collateral to cover debt")]
    NotEnoughCollateral,

    #[msg("Collateral not yet enabled")]
    CollateralNotEnabled,

    #[msg("Cannot deposit zero collateral amount")]
    CannotDepositZeroAmount,

    #[msg("Cannot withdraw zero collateral amount")]
    CannotWithdrawZeroAmount,

    #[msg("No outstanding debt")]
    NothingToRepay,

    #[msg("Could not generate seed")]
    CannotGenerateSeed,

    #[msg("Need to claim all rewards first")]
    NeedToClaimAllRewardsFirst,

    #[msg("Need to harvest all rewards first")]
    NeedToHarvestAllRewardsFirst,

    #[msg("Cannot stake or unstake 0 amount")]
    StakingZero,

    #[msg("Nothing to unstake")]
    NothingToUnstake,

    #[msg("No reward to withdraw")]
    NoRewardToWithdraw,

    #[msg("Cannot provide zero stability")]
    CannotProvideZeroStability,

    #[msg("Cannot withdraw zero stability")]
    CannotWithdrawZeroStability,

    #[msg("Nothing to withdraw")]
    NothingToWithdraw,

    // TODO: integrate this with the provide_stability function.
    // #[msg("Cannot provide stability more than user balance")]
    // CannotProvideStabilityMoreThanBalance,
    #[msg("Stability Pool is empty")]
    StabilityPoolIsEmpty,

    #[msg("Stability pool cannot offset this much debt")]
    NotEnoughStabilityInTheStabilityPool,

    #[msg("Mismatching next PDA reward address")]
    MismatchedNextPdaRewardAddress,

    #[msg("Mismatching next PDA reward seed")]
    MismatchedNextPdaRewardSeed,

    #[msg("Wrong next reward pda index")]
    MismatchedNextPdaIndex,

    #[msg("Next reward not ready yet")]
    NextRewardNotReadyYet,

    #[msg("Nothing staked, cannot collect any rewards")]
    NothingStaked,

    #[msg("Reward candidate mismatch from user's next pending reward")]
    NextRewardMismatchForUser,

    #[msg("User is well collateralized, cannot liquidate")]
    UserWellCollateralized,

    #[msg("Cannot liquidate the last user")]
    LastUser,

    #[msg("Integer overflow")]
    IntegerOverflow,

    #[msg("Conversion failure")]
    ConversionFailure,

    #[msg("Cannot harvest until liquidation gains are cleared")]
    CannotHarvestUntilLiquidationGainsCleared,

    #[msg("Redemptions queue is full, cannot add one more order")]
    RedemptionsQueueIsFull,

    #[msg("Redemptions queue is empty, nothing to process")]
    RedemptionsQueueIsEmpty,

    #[msg("Redemptions amount too small")]
    RedemptionsAmountTooSmall,

    #[msg("Redemptions amount too much")]
    CannotRedeemMoreThanMinted,

    #[msg("The program needs to finish processing the first outstanding order before moving on to others")]
    NeedToProcessFirstOrderBeforeOthers,

    #[msg("The bot submitted the clearing users in the wrong order")]
    RedemptionClearingOrderIsIncorrect,

    #[msg("Current redemption order is in clearing mode, cannot fill it until it's fully cleared")]
    CannotFillRedemptionOrderWhileInClearingMode,

    #[msg("Current redemption order is in filling mode, cannot clear it until it's filled")]
    CannotClearRedemptionOrderWhileInFillingMode,

    #[msg("Redemption order is inactive")]
    InvalidRedemptionOrder,

    #[msg("Redemption order is empty of candidates")]
    OrderDoesNotHaveCandidates,

    #[msg("Redemption user is not among the candidates")]
    WrongRedemptionUser,

    #[msg("Redemption user is not among the candidates")]
    RedemptionFillerNotFound,

    #[msg("Redeemer does not match with the order being redeemed")]
    InvalidRedeemer,

    #[msg("Duplicate account in fill order")]
    DuplicateAccountInFillOrder,

    #[msg("Redemption user is not among the candidates")]
    RedemptionUserNotFound,

    #[msg("Mathematical operation with overflow")]
    MathOverflow,

    #[msg("Price is not valid")]
    PriceNotValid,

    #[msg("Liquidation queue is full")]
    LiquidationsQueueFull,

    #[msg("Epoch to scale to sum deserialization failed")]
    CannotDeserializeEpochToScaleToSum,

    #[msg("Cannot borrow in Recovery mode")]
    CannotBorrowInRecoveryMode,

    #[msg("Cannot withdraw in Recovery mode")]
    CannotWithdrawInRecoveryMode,

    #[msg("Operation brings system to recovery mode")]
    OperationBringsSystemToRecoveryMode,

    #[msg("Borrowing is disabled")]
    BorrowingDisabled,

    #[msg("Cannot borrow zero amount")]
    CannotBorrowZeroAmount,

    #[msg("Cannot repay zero amount")]
    CannotRepayZeroAmount,

    #[msg("Cannot redeem during bootstrap period")]
    CannotRedeemDuringBootstrapPeriod,

    #[msg("Cannot borrow less than minimum")]
    CannotBorrowLessThanMinimum,

    #[msg("Cannot borrow more than maximum")]
    CannotBorrowMoreThanMaximum,

    #[msg("User debt is lower than the minimum")]
    UserDebtTooLow,

    #[msg("User debt is higher than the maximum")]
    UserDebtTooHigh,

    #[msg("Total debt is more than the maximum")]
    TotalDebtTooHigh,

    #[msg("Cannot redeem while being undercollateralized")]
    CannotRedeemWhenUndercollateralized,

    #[msg("Zero argument not allowed")]
    ZeroAmountInvalid,

    #[msg("Operation lowers system TCR")]
    OperationLowersSystemTCRInRecoveryMode,

    #[msg("Serum DEX variables inputted wrongly")]
    InvalidDexInputs,

    #[msg("Serum DEX transaction didn't execute the swap function")]
    ZeroSwap,

    #[msg("Key is not present in global config")]
    GlobalConfigKeyError,

    #[msg("Marinade deposit didn't go through")]
    MarinadeDepositError,

    #[msg("Cannot delegate inactive collateral")]
    CannotDelegateInactive,

    #[msg("User is either deposited or inactive, can't be both")]
    StatusMismatch,

    #[msg("Unexpected account in instruction")]
    UnexpectedAccount,

    #[msg("User is either deposited or inactive, can't be both")]
    OperationBringsPositionBelowMCR,

    #[msg("Operation forbidden")]
    OperationForbidden,

    #[msg("System is in emergency mode")]
    SystemInEmergencyMode,

    #[msg("Borrow is currently blocked")]
    BorrowBlocked,
    #[msg("Clear_liquidation_gains is currently blocked")]
    ClearLiquidationGainsBlocked,

    #[msg("Airdrop_HBB is currently blocked")]
    AirdropHBBBlocked,

    #[msg("Withdraw_collateral is currently blocked")]
    WithdrawCollateralBlocked,

    #[msg("Try_liquidate is currently blocked")]
    TryLiquidateBlocked,

    #[msg("deposit_and_borrow is currently blocked")]
    DepositAndBorrowBlocked,

    #[msg("harvest_liquidation_gains is currently blocked")]
    HarvestLiquidationGainsBlocked,

    #[msg("withdraw_stability is currently blocked")]
    WithdrawStabilityBlocked,

    #[msg("clear_redemption_order is currently blocked")]
    ClearRedemptionOrderBlocked,

    #[msg("User deposit is too high")]
    UserDepositTooHigh,

    #[msg("Total deposit is too high")]
    TotalDepositTooHigh,

    #[msg("Out of range integral conversion attempted")]
    OutOfRangeIntegralConversion,
}
