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

use crate::{DatedPrice, Price, Result, ScopeError};
use anchor_lang::prelude::*;
use pyth_client::{PriceStatus, PriceType};
use std::convert::{TryFrom, TryInto};

/// validate price confidence - confidence/price ratio should be less than 2%
const ORACLE_CONFIDENCE_FACTOR: u64 = 50; // 100% / 2%

pub fn get_price(pyth_price_info: &AccountInfo) -> Result<DatedPrice> {
    let pyth_price_data = &pyth_price_info.try_borrow_data()?;
    let pyth_price = pyth_client::cast::<pyth_client::Price>(pyth_price_data);
    let price = validate_valid_price(pyth_price)?;

    Ok(DatedPrice {
        price: Price {
            value: price,
            exp: pyth_price.expo.abs().try_into().unwrap(),
        },
        last_updated_slot: pyth_price.valid_slot,
        ..Default::default()
    })
}

fn validate_valid_price(pyth_price: &pyth_client::Price) -> Result<u64> {
    if option_env!("CLUSTER").unwrap_or("localnet") == "devnet" {
        return Ok(u64::try_from(pyth_price.agg.price).unwrap());
    }
    let is_trading = get_status(&pyth_price.agg.status);
    if !is_trading {
        return Err(ScopeError::PriceNotValid.into());
    }
    if pyth_price.num_qt < 3 {
        return Err(ScopeError::PriceNotValid.into());
    }

    let price = u64::try_from(pyth_price.agg.price).unwrap();
    if price == 0 {
        return Err(ScopeError::PriceNotValid.into());
    }
    let conf: u64 = pyth_price.agg.conf;
    let conf_50x: u64 = conf.checked_mul(ORACLE_CONFIDENCE_FACTOR).unwrap();
    if conf_50x > price {
        return Err(ScopeError::PriceNotValid.into());
    };
    Ok(price)
}

fn get_status(st: &PriceStatus) -> bool {
    matches!(st, PriceStatus::Trading)
}

pub fn validate_pyth_price(pyth_price: &pyth_client::Price) -> Result<()> {
    if pyth_price.magic != pyth_client::MAGIC {
        msg!("Pyth price account provided is not a valid Pyth account");
        return Err(ProgramError::InvalidArgument.into());
    }
    if !matches!(pyth_price.ptype, PriceType::Price) {
        msg!("Pyth price account provided has invalid price type");
        return Err(ProgramError::InvalidArgument.into());
    }
    if pyth_price.ver != pyth_client::VERSION_2 {
        msg!("Pyth price account provided has a different version than the Pyth client");
        return Err(ProgramError::InvalidArgument.into());
    }
    if !matches!(pyth_price.agg.status, PriceStatus::Trading) {
        msg!("Pyth price account provided is not active");
        return Err(ProgramError::InvalidArgument.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use anchor_lang::prelude::*;

    const PRICE_ACCT_SIZE: usize = 3312;

    const PRICE_MAGIC_OFFSET: usize = 0;
    const PRICE_VERSION_OFFSET: usize = 4;
    const PRICE_TYPE_OFFSET: usize = 16;
    const PRICE_STATUS_OFFSET: usize = 224;

    fn assert_err<T>(res: Result<T>, err: ProgramError) {
        match res {
            Ok(_) => panic!("Expect error {err} received Ok"),
            // Expected branch
            Err(Error::ProgramError(recv_e)) if recv_e.program_error == err => (),
            // Other errors
            Err(recv_e) => panic!("Expect error {err:?} received {recv_e:?}"),
        };
    }

    #[test]
    pub fn test_validate_price() {
        let buff = valid_price_bytes();
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert!(super::validate_pyth_price(price).err().is_none());
    }

    #[test]
    pub fn test_validate_price_magic_incorrect() {
        let incorrect_magic = 0xa1b2c3d3_u32.to_le_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_magic, PRICE_MAGIC_OFFSET);
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_err(
            super::validate_pyth_price(price),
            ProgramError::InvalidArgument,
        );
    }

    #[test]
    pub fn test_validate_price_price_type_incorrect() {
        let incorrect_price_type: &[u8] = &[0];
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, incorrect_price_type, PRICE_TYPE_OFFSET);
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_err(
            super::validate_pyth_price(price),
            ProgramError::InvalidArgument,
        );
    }

    #[test]
    pub fn test_validate_price_version_incorrect() {
        let incorrect_price_version = 1_u32.to_le_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_price_version, PRICE_VERSION_OFFSET);
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_err(
            super::validate_pyth_price(price),
            ProgramError::InvalidArgument,
        );
    }

    #[test]
    pub fn test_validate_price_status_incorrect() {
        let incorrect_price_status = 0_u32.to_be_bytes();
        let mut buff = valid_price_bytes();
        write_bytes(&mut buff, &incorrect_price_status, PRICE_STATUS_OFFSET);
        let price = pyth_client::cast::<pyth_client::Price>(&buff);
        assert_err(
            super::validate_pyth_price(price),
            ProgramError::InvalidArgument,
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
        buff[offset..(bytes.len() + offset)].clone_from_slice(bytes);
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
