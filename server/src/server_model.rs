use dashmap::DashMap;
use headers::Range;
use model::Manifest;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use uuid::Uuid;
use warp::{
    http::*,
    hyper::{body::Bytes, header::*, Body},
    reply::{Reply, Response},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub content_path: PathBuf,
    pub server_addr: SocketAddr,
    pub remove_ext: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            content_path: "./content".into(),
            server_addr: ([0, 0, 0, 0], 16342).into(),
            remove_ext: "del".into(),
        }
    }
}

#[derive(Debug)]
pub struct ManifestData {
    pub blob: Vec<u8>,
    pub data: Manifest,
}

pub struct ManifestReply(pub Arc<ManifestData>);

impl Reply for ManifestReply {
    fn into_response(self) -> warp::reply::Response {
        let response = warp::http::Response::builder()
            .header(CONTENT_TYPE, "application/msgpack")
            .header(CONTENT_LENGTH, self.0.blob.len());
        response.body(Body::from(Bytes::from_owner(self))).unwrap()
    }
}

impl AsRef<[u8]> for ManifestReply {
    fn as_ref(&self) -> &[u8] {
        self.0.blob.as_ref()
    }
}
