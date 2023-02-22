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
        num_qt: 3,
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

    #[derive(PartialEq, Eq, Debug, Copy, Clone)]
    #[repr(C)]
    pub enum PriceStatus {
        Unknown = 0,
        Trading = 1,
        Halted = 2,
        Auction = 3,
    }

    impl Default for PriceStatus {
        fn default() -> Self {
            PriceStatus::Trading
        }
    }

    #[derive(Copy, Clone)]
    #[repr(C)]
    pub enum CorpAction {
        NoCorpAct,
    }

    impl Default for CorpAction {
        fn default() -> Self {
            CorpAction::NoCorpAct
        }
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

    #[derive(Copy, Clone)]
    #[repr(C)]
    pub enum PriceType {
        Unknown,
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

    impl Default for PriceType {
        fn default() -> Self {
            PriceType::Price
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
        pub magic: u32,            // Pyth magic number
        pub ver: u32,              // Program version
        pub atype: u32,            // Account type
        pub size: u32,             // Price account size
        pub ptype: PriceType,      // Price or calculation type
        pub expo: i32,             // Price exponent
        pub num: u32,              // Number of component prices
        pub num_qt: u32,           // Number of quoters that make up aggregate
        pub last_slot: u64,        // Slot of last valid (not unknown) aggregate price
        pub valid_slot: u64,       // Valid slot-time of agg. price
        pub twap: Ema,             // Time-weighted average price
        pub twac: Ema,             // Time-weighted average confidence interval
        pub drv1: i64,             // Space for future derived values
        pub drv2: i64,             // Space for future derived values
        pub prod: AccKey,          // Product account key
        pub next: AccKey,          // Next Price account in linked list
        pub prev_slot: u64,        // Valid slot of previous update
        pub prev_price: i64,       // Aggregate price of previous update
        pub prev_conf: u64,        // Confidence interval of previous update
        pub drv3: i64,             // Space for future derived values
        pub agg: PriceInfo,        // Aggregate price info
        pub comp: [PriceComp; 32], // Price components one per quoter
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
