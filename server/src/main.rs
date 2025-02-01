#![allow(dead_code)]
#![allow(unused_imports)]

use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::abort;
use std::str::FromStr;
use std::sync::Arc;

use dashmap::DashMap;
use headers::{Header, Range};
use hyper::header::HeaderValue;
use log::info;
use model::{ItemOp, Manifest};
use serde_bytes::ByteBuf;
use sha3::{Digest, Sha3_256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;
use ulid::Ulid;
use url::Url;
use warp::http::StatusCode;
use warp::reject::Rejection;
use warp::Filter;

mod client_ip;
mod init_log;
mod print;
mod server_model;
mod utils;

use client_ip::*;
use init_log::*;
use print::*;
use server_model::*;
use utils::*;

const CONFIG_PATH: &'static str = "./syner_server.toml";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = if !tokio::fs::try_exists(CONFIG_PATH).await? {
        first_config(&PathBuf::from(CONFIG_PATH)).await?
    } else {
        let config_str = tokio::fs::read_to_string(CONFIG_PATH).await?;
        toml::from_str(&config_str)?
    };
    let config = Arc::new(config);

    init_logger()?;

    let content_path = Arc::new(config.content_path.clone());
    let backup_path = Arc::new(PathBuf::from_str("./.c")?);

    tokio::fs::create_dir_all(&*content_path).await?;
    tokio::fs::create_dir_all(&*backup_path).await?;

    let manifest = Arc::new(tokio::sync::RwLock::new(
        collect_manifest(config.clone(), content_path.clone()).await?,
    ));
    ready_for_back_up(backup_path.clone()).await?;
    backup_conten(
        config.clone(),
        content_path.clone(),
        backup_path.clone(),
        &*content_path,
    )
    .await?;

    let mut set = JoinSet::new();
    set.spawn(server_thread(config.clone(), manifest.clone()));
    set.spawn(input_watch_thread(
        config,
        content_path,
        backup_path,
        manifest,
    ));

    sprintln!()?;
    print_help()?;
    sprintln!()?;

    while let Some(res) = set.join_next().await {
        res??;
    }

    Ok(())
}

async fn first_config(config_path: &Path) -> anyhow::Result<Config> {
    let mut config = Config::default();
    sprintln!("不存在配置文件，开始初次配置")?;

    loop {
        sprint!("内容文件夹（默认 ./content）：")?;
        let str = read_line()?;
        if str.is_empty() {
            break;
        }
        if !is_valid_path(&str) {
            sprintln!("路径格式错误, 请重新输入")?;
            continue;
        }
        match PathBuf::from_str(&str) {
            Ok(path) => {
                config.content_path = path;
                break;
            }
            Err(_) => {
                sprintln!("路径格式错误, 请重新输入")?;
            }
        }
    }

    loop {
        sprint!("服务器监听地址（默认 0.0.0.0:16342）：")?;
        let str = read_line()?;
        if str.is_empty() {
            break;
        }
        match SocketAddr::from_str(&str) {
            Ok(addr) => {
                config.server_addr = addr;
                break;
            }
            Err(_) => {
                sprintln!("地址格式错误, 请重新输入")?;
            }
        }
    }

    loop {
        sprint!("标记删除的文件的后缀 （默认 del）：")?;
        let str = read_line()?;
        if str.is_empty() {
            break;
        }
        config.remove_ext = str;
        break;
    }

    let config_str = toml::to_string_pretty(&config)?;
    tokio::fs::write(config_path, &config_str).await?;

    Ok(config)
}

async fn input_watch_thread(
    config: Arc<Config>,
    contnet_path: Arc<PathBuf>,
    backup_path: Arc<PathBuf>,
    manifest: Arc<tokio::sync::RwLock<Arc<ManifestData>>>,
) -> anyhow::Result<()> {
    loop {
        let str = tokio::task::spawn_blocking(read_line).await??;
        match &*str {
            "?" | "h" | "help" => print_help()?,
            "q" | "quit" | "exit" | "stop" => std::process::exit(0),
            "r" | "reload" => {
                re_collect_manifest(
                    config.clone(),
                    contnet_path.clone(),
                    backup_path.clone(),
                    manifest.clone(),
                )
                .await?
            }
            _ => {
                sprintln!("未知指令")?;
                print_help()?;
            }
        }
    }
}

fn print_help() -> anyhow::Result<()> {
    sprintln!(
        r#"? | h | help 				=> 显示此帮助信息
q | quit | exit | stop 			=> 退出进程
r | reload 				=> 重新加载文件，重新生成清单"#
    )?;
    Ok(())
}

async fn collect_manifest(
    config: Arc<Config>,
    content_path: Arc<PathBuf>,
) -> anyhow::Result<Arc<ManifestData>> {
    let map = Arc::new(DashMap::new());
    collect_manifest_files(config, content_path.clone(), content_path, map.clone()).await?;
    Ok(Arc::new(ManifestData {
        blob: rmp_serde::to_vec(&*map)?,
        data: map,
    }))
}

async fn collect_manifest_files(
    config: Arc<Config>,
    root_path: Arc<PathBuf>,
    dir: Arc<PathBuf>,
    map: Manifest,
) -> anyhow::Result<()> {
    let mut read_dir = tokio::fs::read_dir(&*dir).await?;

    let mut set = JoinSet::<anyhow::Result<()>>::new();

    while let Some(entry) = read_dir.next_entry().await? {
        let config = config.clone();
        let map = map.clone();
        let root_path = root_path.clone();
        set.spawn(async move {
            let entry = entry;
            let map = map;
            let ft = entry.file_type().await?;
            let path = Arc::new(entry.path());

            if ft.is_dir() {
                let root_path = root_path.clone();
                let path = path.clone();
                let map = map.clone();
                fn f(config:Arc<Config>,root_path: Arc<PathBuf>, path: Arc<PathBuf>, map: Manifest) -> impl Future<Output = anyhow::Result<()>> + Send {
                    collect_manifest_files(config,root_path, path, map)
                }
                tokio::task::spawn(f(config.clone(), root_path, path, map)).await??;
                return Ok(());
            }

            if !ft.is_file() {
                return Ok(());
            }

            let op = if let Some(true) = path.extension().map(|s| s.to_string_lossy() == config.remove_ext) {
                ItemOp::Remove
            } else {
                ItemOp::Sync
            };

            let mut file = tokio::fs::File::open(&*path).await?;
            let meta = file.metadata().await?;
            let len = meta.len();

            let mut hasher = Sha3_256::new();
            {
                let mut buffer = Vec::with_capacity(1024);
                unsafe { buffer.set_len(1024) };
                loop {
                    let count = file.read(&mut buffer).await?;
                    if count == 0 {
                        break;
                    }
                    hasher.update(&buffer[..count]);
                }
            };
            let hash = hasher.finalize();

            let rel = pathdiff::diff_paths(&*path, &*root_path).unwrap();
            let parts: Vec<_> = rel
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();

            let parts = parts.join("/");

            let item = (op, len,  hash.to_vec().into());
            log::info!(target: "manifest", "Loaded {:?} {{ op = {:?}, len = {}, hash = {:?} }}", parts, item.0, item.1, base16ct::lower::encode_string(&hash));
            map.insert(parts, item);

            Ok(())
        });
    }

    while let Some(res) = set.join_next().await {
        res??;
    }

    Ok(())
}

async fn re_collect_manifest(
    config: Arc<Config>,
    content_path: Arc<PathBuf>,
    backup_path: Arc<PathBuf>,
    manifest: Arc<tokio::sync::RwLock<Arc<ManifestData>>>,
) -> anyhow::Result<()> {
    sprintln!("正在重新加载清单")?;
    tokio::fs::create_dir_all(&*content_path).await?;
    tokio::fs::create_dir_all(&*backup_path).await?;
    let new = collect_manifest(config.clone(), content_path.clone()).await?;
    let mut manifest = manifest.write().await;
    ready_for_back_up(backup_path.clone()).await?;
    backup_conten(config, content_path.clone(), backup_path, &*content_path).await?;
    *manifest = new;
    sprintln!("清单加载完成")?;
    Ok(())
}

async fn ready_for_back_up(backup_path: Arc<PathBuf>) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || remove_dir_all::ensure_empty_dir(&*backup_path)).await??;
    Ok(())
}

async fn backup_conten(
    config: Arc<Config>,
    content_path: Arc<PathBuf>,
    backup_path: Arc<PathBuf>,
    cur_dir: &Path,
) -> anyhow::Result<()> {
    let mut read_dir = tokio::fs::read_dir(&*cur_dir).await?;
    let mut set = JoinSet::<anyhow::Result<()>>::new();

    while let Some(entry) = read_dir.next_entry().await? {
        let config = config.clone();
        let content_path = content_path.clone();
        let backup_path = backup_path.clone();
        set.spawn(async move {
            let path = entry.path();
            let rel = pathdiff::diff_paths(&path, &*content_path).unwrap();
            let mut dst = (*backup_path).clone();
            dst.push(rel);

            let meta = entry.metadata().await?;
            if meta.is_dir() {
                fn f<'a>(
                    config: Arc<Config>,
                    content_path: Arc<PathBuf>,
                    backup_path: Arc<PathBuf>,
                    cur_dir: PathBuf,
                ) -> impl Future<Output = anyhow::Result<()>> + Send + 'a {
                    async move { backup_conten(config, content_path, backup_path, &cur_dir).await }
                }
                tokio::spawn(f(
                    config.clone(),
                    content_path.clone(),
                    backup_path.clone(),
                    path,
                ))
                .await??;

                return Ok(());
            }

            if !meta.is_file() {
                return Ok(());
            }

            if let Some(true) = path
                .extension()
                .map(|s| s.to_string_lossy() == config.remove_ext)
            {
                return Ok(());
            }

            log::info!(target: "manifest", "Backup {:?} => {:?}", path, dst);

            let mut dst_dir = dst.clone();
            dst_dir.pop();
            tokio::fs::create_dir_all(&*dst_dir).await?;

            tokio::fs::hard_link(path, dst).await?;

            Ok(())
        });
    }

    while let Some(res) = set.join_next().await {
        res??;
    }

    Ok(())
}

async fn server_thread(
    config: Arc<Config>,
    manifest: Arc<tokio::sync::RwLock<Arc<ManifestData>>>,
) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(&config.content_path).await?;

    let manifest = warp::get()
        .and(warp::path("manifest"))
        .and(log_req(true))
        .and_then(move || get_manifest(manifest.clone()));
    let contents = warp::path("content")
        .and(warp::get().or(warp::head()))
        .unify()
        .and(log_req(true))
        .and(warp::fs::dir("./.c/"));
    let fallback = warp::any()
        .and(log_req(false))
        .map(|| StatusCode::IM_A_TEAPOT);

    let routes = manifest.or(contents).or(fallback);
    warp::serve(routes).run(config.server_addr).await;

    Ok(())
}

async fn get_manifest(
    manifest: Arc<tokio::sync::RwLock<Arc<ManifestData>>>,
) -> Result<impl warp::Reply, Rejection> {
    let manifest = manifest.read().await;
    Ok(ManifestReply(manifest.clone()))
}
