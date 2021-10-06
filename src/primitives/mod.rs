mod hash;
pub use self::hash::*;

pub(crate) type CCDifficulty = u32;
/// The current server time, in UTC seconds
pub(crate) type CCTimestamp = f64;
/// The proof-of-work nonce
pub(crate) type CCNonce = u64;

pub use std::sync::{atomic::AtomicBool, Arc};
pub type AtomicFlag = Arc<AtomicBool>;
