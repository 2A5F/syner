use std::sync::Arc;

use dashmap::DashMap;
use serde_bytes::ByteBuf;
use serde_repr::*;
use sha3::{Digest, Sha3_256};
use tokio::io::AsyncReadExt;

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

pub async fn calc_hash(mut file: tokio::fs::File) -> anyhow::Result<Vec<u8>> {
    tokio::spawn(async move {
        let mut hasher = Sha3_256::new();
        {
            let mut buffer: [u8; 4096] = [0; 4096];
            loop {
                let count = file.read(&mut buffer).await?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
        };
        let hash = hasher.finalize();
        Ok(hash.to_vec())
    })
    .await?
}
