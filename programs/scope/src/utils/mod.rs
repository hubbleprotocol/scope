pub mod scope_chain;

use std::cell::Ref;

use crate::{ScopeError, ScopeResult};
use anchor_lang::__private::bytemuck;
use anchor_lang::prelude::{AccountDeserialize, AccountInfo};
use anchor_lang::Discriminator;

pub fn account_deserialize<T: AccountDeserialize + Discriminator>(
    account: &AccountInfo<'_>,
) -> ScopeResult<T> {
    let data = account.clone().data.borrow().to_owned();
    let discriminator = &data[..8];
    if discriminator != T::discriminator() {
        return Err(ScopeError::InvalidAccountDiscriminator);
    }

    let mut data: &[u8] = &data;
    let user: T =
        T::try_deserialize(&mut data).map_err(|_| ScopeError::UnableToDeserializeAccount)?;

    Ok(user)
}

pub fn zero_copy_deserialize<'info, T: bytemuck::AnyBitPattern + Discriminator>(
    account: &'info AccountInfo,
) -> ScopeResult<Ref<'info, T>> {
    let data = account.data.try_borrow().unwrap();

    let mut disc_bytes = [0u8; 8];
    disc_bytes.copy_from_slice(&data[..8]);
    if disc_bytes != T::discriminator() {
        return Err(ScopeError::InvalidAccountDiscriminator);
    }

    Ok(Ref::map(data, |data| bytemuck::from_bytes(&data[8..])))
}
