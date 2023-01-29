//! Toolings to retrieve pyth prices and validate them
//!
//! Validation partially follows [pyth best practices](https://docs.pyth.network/consumers/best-practices)
//!
//! 1. Some checks in [`validate_pyth_price`] are performed on the pyth price account upon registration in
//!    the oracle mapping. However some information present only in the associated pyth product account are
//!    expected to be checked by the admin to ensure the product has the expected quality prior the mapping
//!    update.
//! 2. Upon usage the current price state is checked in [`validate_valid_price`]
//! 3. The confidence interval is also checked in this same function with [`ORACLE_CONFIDENCE_FACTOR`]

use std::convert::{TryFrom, TryInto};

use anchor_lang::prelude::*;
use pyth_client::PriceType;
use pyth_sdk_solana::state as pyth_client;

use crate::{DatedPrice, Price, Result, ScopeError};

/// validate price confidence - confidence/price ratio should be less than 2%
const ORACLE_CONFIDENCE_FACTOR: u64 = 50; // 100% / 2%

pub fn get_price(price_info: &AccountInfo) -> Result<DatedPrice> {
    let data = price_info.try_borrow_data()?;
    let price_account = pyth_client::load_price_account(data.as_ref())
        .map_err(|_| error!(ScopeError::PriceNotValid))?;

    let pyth_raw = price_account.to_price_feed(price_info.key);

    let pyth_price = if cfg!(feature = "skip_price_validation") {
        // Don't validate price in tests
        pyth_raw.get_current_price_unchecked()
    } else if let Some(pyth_price) = pyth_raw.get_current_price() {
        // Or use the current valid price if available
        pyth_price
    } else {
        msg!("No valid price in pyth account {}", price_info.key);
        return err!(ScopeError::PriceNotValid);
    };

    let price = validate_valid_price(&pyth_price, ORACLE_CONFIDENCE_FACTOR).map_err(|e| {
        msg!("Invalid price on pyth account {}", price_info.key);
        e
    })?;

    Ok(DatedPrice {
        price: Price {
            value: price,
            exp: pyth_price.expo.abs().try_into().unwrap(),
        },
        last_updated_slot: price_account.valid_slot,
        unix_timestamp: u64::try_from(price_account.timestamp).unwrap(),
        ..Default::default()
    })
}

pub fn validate_valid_price(
    pyth_price: &pyth_client::Price,
    oracle_confidence_factor: u64,
) -> Result<u64> {
    if cfg!(feature = "skip_price_validation") {
        return Ok(u64::try_from(pyth_price.price).unwrap());
    }

    let price = u64::try_from(pyth_price.price).unwrap();
    if price == 0 {
        return err!(ScopeError::PriceNotValid);
    }
    let conf: u64 = pyth_price.conf;
    let conf_50x: u64 = conf.checked_mul(oracle_confidence_factor).unwrap();
    if conf_50x > price {
        return err!(ScopeError::PriceNotValid);
    };
    Ok(price)
}

fn validate_pyth_price(pyth_price: &pyth_client::PriceAccount) -> Result<()> {
    if pyth_price.magic != pyth_client::MAGIC {
        msg!("Pyth price account provided is not a valid Pyth account");
        return err!(ScopeError::PriceNotValid);
    }
    if !matches!(pyth_price.ptype, PriceType::Price) {
        msg!("Pyth price account provided has invalid price type");
        return err!(ScopeError::PriceNotValid);
    }
    if pyth_price.ver != pyth_client::VERSION_2 {
        msg!("Pyth price account provided has a different version than the Pyth client");
        return err!(ScopeError::PriceNotValid);
    }
    if !matches!(pyth_price.agg.status, pyth_client::PriceStatus::Trading) {
        msg!("Pyth price account provided is not active");
        return err!(ScopeError::PriceNotValid);
    }
    Ok(())
}

pub fn validate_pyth_price_info(pyth_price_info: &AccountInfo) -> Result<()> {
    if cfg!(feature = "skip_price_validation") {
        return Ok(());
    }
    let pyth_price_data = pyth_price_info.try_borrow_data()?;
    let pyth_price = pyth_client::load_price_account(&pyth_price_data).unwrap();

    validate_pyth_price(pyth_price)
}

#[cfg(test)]
mod tests {
    use super::*;
    const PRICE_ACCT_SIZE: usize = 3312;

    const PRICE_MAGIC_OFFSET: usize = 0;
    const PRICE_VERSION_OFFSET: usize = 4;
    const PRICE_ACCOUNT_TYPE_OFFSET: usize = 8;
    const PRICE_TYPE_OFFSET: usize = 16;
    const PRICE_STATUS_OFFSET: usize = 224;

    fn assert_err<T>(res: Result<T>, err: ScopeError) {
        match res {
            Ok(_) => panic!("Expect error {err} received Ok"),
            // Expected branch
            Err(Error::ProgramError(recv_e)) => panic!("Expect error {err:?} received {recv_e:?}"),
            // Other errors
            Err(recv_e) => assert_eq!(recv_e, error!(err)),
        };
    }

    #[test]
    pub fn test_validate_price() {
        let buff = valid_price_bytes();
        let price = pyth_client::load_price_account(&buff).unwrap();
        assert!(super::validate_pyth_price(price).is_ok());
    }

    #[test]
    pub fn test_validate_price_magic_incorrect() {
        let incorrect_magic = 0xa1b2c3d3_u32.to_le_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_magic, PRICE_MAGIC_OFFSET);
        assert!(pyth_client::load_price_account(&buff).is_err());
    }

    #[test]
    pub fn test_validate_price_price_type_incorrect() {
        let incorrect_price_type: &[u8] = &[0];
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, incorrect_price_type, PRICE_TYPE_OFFSET);
        let price = pyth_client::load_price_account(&buff).unwrap();
        assert_err(super::validate_pyth_price(price), ScopeError::PriceNotValid);
    }

    #[test]
    pub fn test_validate_price_version_incorrect() {
        let incorrect_price_version = 1_u32.to_le_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_price_version, PRICE_VERSION_OFFSET);
        // Error detected directly by pyth crate
        assert!(pyth_client::load_price_account(&buff).is_err());
    }

    #[test]
    pub fn test_validate_price_status_incorrect() {
        let incorrect_price_status = 0_u32.to_be_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_price_status, PRICE_STATUS_OFFSET);
        let price = pyth_client::load_price_account(&buff).unwrap();
        assert_err(super::validate_pyth_price(price), ScopeError::PriceNotValid);
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
        write_bytes(&mut buff, &[3_u8], PRICE_ACCOUNT_TYPE_OFFSET); // account type = price
        write_bytes(&mut buff, &[1_u8], PRICE_STATUS_OFFSET); // price status = trading
        buff
    }

    fn write_bytes(buff: &mut [u8], bytes: &[u8], offset: usize) {
        buff[offset..(bytes.len() + offset)].clone_from_slice(bytes);
    }
}
