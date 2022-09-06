pub mod scope_chain;

use std::cell::Ref;

use crate::{ScopeError, ScopeResult};
use anchor_lang::__private::bytemuck;
use anchor_lang::prelude::{msg, AccountDeserialize, AccountInfo};
use anchor_lang::{Discriminator, Key};

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

    let disc_bytes = &data[..8];
    if disc_bytes != T::discriminator() {
        msg!(
            "Expected discriminator for account {:?} ({:?}) is different from received {:?}",
            account.key(),
            T::discriminator(),
            disc_bytes
        );
        return Err(ScopeError::InvalidAccountDiscriminator);
    }

    Ok(Ref::map(data, |data| bytemuck::from_bytes(&data[8..])))
}
