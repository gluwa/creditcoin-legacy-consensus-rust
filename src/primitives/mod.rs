mod hash;
pub use self::hash::*;
mod publishing_flag;
pub use publishing_flag::PublishingFlag;

pub(crate) type CCDifficulty = u32;
/// The current server time, in UTC seconds
pub(crate) type CCTimestamp = f64;
/// The proof-of-work nonce
pub(crate) type CCNonce = u64;
