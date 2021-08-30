use std::sync::{atomic::AtomicBool, Arc};

pub type PublishingFlag = Arc<AtomicBool>;
