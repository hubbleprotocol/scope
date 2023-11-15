use anchor_lang::prelude::{Clock, Pubkey};
use scope::Price;
use solana_sdk::pubkey;

pub const fn id() -> Pubkey {
    // It does not matter what the pubkey is
    pubkey!("Pyth111111111111111111111111111111111111111")
}

pub fn get_account_data_for_price(price: &Price, clock: &Clock) -> Vec<u8> {
    let int_price = price.value as i64;
    let expo = -(price.exp as i32);
    pyth_tools::Price {
        magic: 0xa1b2c3d4,
        ver: 2,
        atype: 3,
        ptype: pyth_tools::PriceType::Price,
        expo,
        valid_slot: clock.slot,
        last_slot: clock.slot,
        timestamp: clock.unix_timestamp,
        num_qt: 3,
        ema_price: pyth_tools::Ema {
            val: int_price,
            numer: int_price,
            denom: 1,
        },
        ema_conf: pyth_tools::Ema {
            val: 0,
            numer: 0,
            denom: 1,
        },
        agg: pyth_tools::PriceInfo {
            price: int_price,
            conf: 0,
            status: pyth_tools::PriceStatus::Trading,
            corp_act: pyth_tools::CorpAction::NoCorpAct,
            pub_slot: clock.slot,
        },
        ..Default::default()
    }
    .as_bytes()
}

mod pyth_tools {
    use bytemuck::{bytes_of, Pod, Zeroable};

    #[derive(Default, Copy, Clone)]
    #[repr(C)]
    pub struct AccKey {
        pub val: [u8; 32],
    }

    #[derive(PartialEq, Eq, Debug, Copy, Clone, Default)]
    #[repr(C)]
    pub enum PriceStatus {
        Unknown = 0,
        #[default]
        Trading = 1,
        Halted = 2,
        Auction = 3,
    }

    #[derive(Copy, Clone, Default)]
    #[repr(C)]
    pub enum CorpAction {
        #[default]
        NoCorpAct,
    }

    #[derive(Default, Copy, Clone)]
    #[repr(C)]
    pub struct PriceInfo {
        pub price: i64,
        pub conf: u64,
        pub status: PriceStatus,
        pub corp_act: CorpAction,
        pub pub_slot: u64,
    }
    #[derive(Default, Copy, Clone)]
    #[repr(C)]
    pub struct PriceComp {
        publisher: AccKey,
        agg: PriceInfo,
        latest: PriceInfo,
    }

    #[derive(Copy, Clone, Default)]
    #[repr(C)]
    pub enum PriceType {
        Unknown,
        #[default]
        Price,
        Twap,
        Volatility,
    }

    impl From<PriceStatus> for u8 {
        fn from(price: PriceStatus) -> Self {
            match price {
                PriceStatus::Unknown => 0,
                PriceStatus::Trading => 1,
                PriceStatus::Halted => 2,
                PriceStatus::Auction => 3,
            }
        }
    }

    #[derive(Default, Copy, Clone)]
    #[repr(C)]
    pub struct Ema {
        pub val: i64,   // Current value of ema
        pub numer: i64, // Numerator state for next update
        pub denom: i64, // Denominator state for next update
    }
    #[derive(Default, Copy, Clone)]
    #[repr(C)]
    pub struct Price {
        /// pyth magic number
        pub magic: u32,
        /// program version
        pub ver: u32,
        /// account type
        pub atype: u32,
        /// price account size
        pub size: u32,
        /// price or calculation type
        pub ptype: PriceType,
        /// price exponent
        pub expo: i32,
        /// number of component prices
        pub num: u32,
        /// number of quoters that make up aggregate
        pub num_qt: u32,
        /// slot of last valid (not unknown) aggregate price
        pub last_slot: u64,
        /// valid slot-time of agg. price
        pub valid_slot: u64,
        /// exponentially moving average price
        pub ema_price: Ema,
        /// exponentially moving average confidence interval
        pub ema_conf: Ema,
        /// unix timestamp of aggregate price
        pub timestamp: i64,
        /// min publishers for valid price
        pub min_pub: u8,
        /// space for future derived values
        pub drv2: u8,
        /// space for future derived values
        pub drv3: u16,
        /// space for future derived values
        pub drv4: u32,
        /// product account key
        pub prod: AccKey,
        /// next Price account in linked list
        pub next: AccKey,
        /// valid slot of previous update
        pub prev_slot: u64,
        /// aggregate price of previous update with TRADING status
        pub prev_price: i64,
        /// confidence interval of previous update with TRADING status
        pub prev_conf: u64,
        /// unix timestamp of previous aggregate with TRADING status
        pub prev_timestamp: i64,
        /// aggregate price info
        pub agg: PriceInfo,
        /// price components one per quoter
        pub comp: [PriceComp; 32],
    }
    impl Price {
        pub fn as_bytes(&self) -> Vec<u8> {
            bytes_of(self).to_vec()
        }
    }

    #[cfg(target_endian = "little")]
    unsafe impl Zeroable for Price {}

    #[cfg(target_endian = "little")]
    unsafe impl Pod for Price {}
}
