use crate::*;
use serde::{Deserialize, Serialize};
use slint::ToSharedString;
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub cwd: PathBuf,
    pub server: Url,
    pub delete_mode: DeleteMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cwd: PathBuf::from("./"),
            server: Url::parse("http://127.0.0.1").unwrap(),
            delete_mode: Default::default(),
        }
    }
}

impl Config {
    pub fn to_view_model(&self) -> ConfigViewModel {
        ConfigViewModel {
            cwd: self.cwd.as_os_str().to_string_lossy().to_string().into(),
            server: self.server.to_shared_string(),
            delete_mode: self.delete_mode.into(),
        }
    }

    pub fn from_view_model(model: &ConfigViewModel) -> Result<Config, Box<dyn Error>> {
        Ok(Config {
            cwd: model.cwd.to_string().into(),
            server: Url::parse(&model.server)?,
            delete_mode: model.delete_mode.into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeleteMode {
    Rename,
    Delete,
}

impl Default for DeleteMode {
    fn default() -> Self {
        DeleteMode::Rename
    }
}

impl Into<DeleteModeViewModel> for DeleteMode {
    fn into(self) -> DeleteModeViewModel {
        match self {
            Self::Rename => DeleteModeViewModel::Rename,
            Self::Delete => DeleteModeViewModel::Delete,
        }
    }
}

impl From<DeleteModeViewModel> for DeleteMode {
    fn from(value: DeleteModeViewModel) -> Self {
        match value {
            DeleteModeViewModel::Rename => Self::Rename,
            DeleteModeViewModel::Delete => Self::Delete,
        }
    }
}
