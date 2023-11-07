pub mod scope_chain;

use std::{
    cell::Ref,
    time::{SystemTime, UNIX_EPOCH},
};

use anchor_lang::{
    __private::bytemuck,
    prelude::{msg, AccountDeserialize, AccountInfo},
    Discriminator, Key,
};

use crate::{ScopeError, ScopeResult};

const SECONDS_IN_AN_HOUR: u64 = 60 * 60;

pub fn account_deserialize<T: AccountDeserialize + Discriminator>(
    account: &AccountInfo<'_>,
) -> ScopeResult<T> {
    let data = account.clone().data.borrow().to_owned();
    let discriminator = data.get(..8).ok_or_else(|| {
        msg!(
            "Account {:?} does not have enough bytes to be deserialized",
            account.key()
        );
        ScopeError::UnableToDeserializeAccount
    })?;
    if discriminator != T::discriminator() {
        msg!(
            "Expected discriminator for account {:?} ({:?}) is different from received {:?}",
            account.key(),
            T::discriminator(),
            discriminator
        );
        return Err(ScopeError::InvalidAccountDiscriminator);
    }

    let mut data: &[u8] = &data;
    let user: T = T::try_deserialize(&mut data).map_err(|_| {
        msg!("Account {:?} deserialization failed", account.key());
        ScopeError::UnableToDeserializeAccount
    })?;

    Ok(user)
}

pub fn zero_copy_deserialize<'info, T: bytemuck::AnyBitPattern + Discriminator>(
    account: &'info AccountInfo,
) -> ScopeResult<Ref<'info, T>> {
    let data = account.data.try_borrow().unwrap();

    let disc_bytes = data.get(..8).ok_or_else(|| {
        msg!(
            "Account {:?} does not have enough bytes to be deserialized",
            account.key()
        );
        ScopeError::UnableToDeserializeAccount
    })?;
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

fn get_current_timestamp() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs()
}

pub fn hours_since_timestamp(previous_timestamp: u64) -> u64 {
    let seconds_elapsed = get_current_timestamp().saturating_sub(previous_timestamp);
    seconds_elapsed / SECONDS_IN_AN_HOUR
}
