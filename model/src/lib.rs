use std::sync::Arc;

use dashmap::DashMap;
use serde_bytes::ByteBuf;
use serde_repr::*;

pub type Manifest = Arc<DashMap<String, ManifestItem>>;

// len, path, hash (sha3 256)
pub type ManifestItem = (ItemOp, u64, ByteBuf);

#[repr(u8)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize_repr, Deserialize_repr,
)]
pub enum ItemOp {
    Sync,
    Remove,
}
