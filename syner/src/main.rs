#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]
#![allow(unused_variables)]

use std::{collections::HashMap, error::Error, fs, path::PathBuf, str::FromStr, time::Instant};

mod boxed_ptr;
mod client_model;
mod utils;
mod winit_helper;
use anyhow::anyhow;
use boxed_ptr::*;
use client_model::*;
use futures_lite::AsyncReadExt;
use model::ManifestItem;
use sha3::{Digest, Sha3_256};
use slint::{ModelRc, SharedString, ToSharedString, VecModel, Weak};
use tokio::{io::AsyncWriteExt, task::JoinSet};
use url::Url;
use utils::*;
use winit_helper::*;

slint::include_modules!();

const CONFIG_PATH: &'static str = "syner.toml";

fn main() -> anyhow::Result<()> {
    let mut config_path = std::env::current_exe()?;
    config_path.pop();
    config_path.push(CONFIG_PATH);

    let config_data_ptr = BoxPtr::new(Config::default());
    let config_data = config_data_ptr.as_mut();

    if !fs::exists(&config_path)? {
        if !setup_window(&config_data_ptr, config_path)? {
            return Ok(());
        }
    } else {
        let config_str = fs::read_to_string(config_path)?;
        *config_data = toml::from_str(&config_str)?;
    }

    main_window(&config_data_ptr)?;

    drop(config_data_ptr);
    Ok(())
}

fn setup_window(config_data_ptr: &BoxPtr<Config>, config_path: PathBuf) -> anyhow::Result<bool> {
    let r_ptr = BoxPtr::new(false);
    let ui_ptr = BoxPtr::new(SetupWindow::new()?);

    let ui = ui_ptr.as_mut();

    let config_data = config_data_ptr.as_mut();
    ui.invoke_set_data(config_data.to_view_model());

    let config_data = config_data_ptr.as_mut();
    let ui2 = ui_ptr.as_mut();
    ui.on_check_model(move |config| {
        let mut success = true;
        if !is_valid_path(config.cwd.as_str()) {
            ui2.invoke_set_cwd_error("不正确的路径".into());
            success = false
        } else {
            match PathBuf::from_str(config.cwd.as_str()) {
                Ok(cwd) => {
                    ui2.invoke_set_cwd_error("".into());
                    config_data.cwd = cwd;
                }
                Err(_) => {
                    ui2.invoke_set_cwd_error("不正确的路径".into());
                    success = false
                }
            }
        }
        match Url::parse(config.server.as_str()) {
            Ok(server) => {
                ui2.invoke_set_server_error("".into());
                config_data.server = server;
            }
            Err(e) => {
                ui2.invoke_set_server_error(format!("不正确的地址: {e}").into());
                success = false
            }
        }
        success
    });
    let config_data = config_data_ptr.as_mut();
    let ui2 = ui_ptr.as_mut();
    let r = r_ptr.as_mut();
    ui.on_save_config(move || {
        if let Err(e) = (|| -> Result<(), Box<dyn Error>> {
            let config = toml::to_string_pretty(config_data)?;
            fs::write(&config_path, config)?;
            Ok(())
        })() {
            ui2.invoke_set_result_error(format!("错误：{e}").into());
        } else {
            *r = true;
            ui2.hide().unwrap();
        }
    });

    center_window(ui.window());
    ui.invoke_hide();
    ui.show()?;
    center_window(ui.window());
    ui.invoke_show();
    set_blur_tab(ui.window());
    ui.run()?;

    let r = *r_ptr.as_ref();
    drop(ui_ptr);
    drop(r_ptr);
    Ok(r)
}

fn main_window(config_data_ptr: &BoxPtr<Config>) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let ui_ptr = BoxPtr::new(AppWindow::new()?);
    let manifest_ptr = BoxPtr::<ClientManifest>::new(vec![]);

    let ui = ui_ptr.as_mut();

    let config_data = config_data_ptr.as_mut();
    ui.invoke_set_data(config_data.to_view_model());

    center_window(ui.window());
    ui.invoke_hide();
    ui.show()?;
    center_window(ui.window());
    ui.invoke_show();
    set_blur_tab(ui.window());

    {
        let config = config_data_ptr.ptr();
        let ui = ui_ptr.as_ref().as_weak();
        let ui2 = ui_ptr.as_ref().as_weak();
        let manifest_ptr = manifest_ptr.ptr();
        rt.spawn(async move {
            let r = req_manifest(config.as_ref(), manifest_ptr.as_mut()).await;
            match r {
                Ok(_) => {
                    let model: VecModel<_> = manifest_ptr
                        .as_ref()
                        .iter()
                        .enumerate()
                        .map(|(i, a)| ModelManifestItem {
                            index: i as i32,
                            op: match a.op {
                                model::ItemOp::Sync => ModelItemOp::Sync,
                                model::ItemOp::Remove => ModelItemOp::Remove,
                            },
                            path: a.path.1.clone(),
                            cur: "".into(),
                            len: "".into(),
                            progress: 0f32,
                            progress_name: "".into(),
                            state: ModelItemState::Pending,
                        })
                        .collect();
                    let model = SendT(ModelRc::new(model));
                    {
                        let ui = ui.clone();
                        tokio::task::spawn_blocking(move || {
                            ui.upgrade_in_event_loop(move |ui| {
                                let model = model;
                                ui.invoke_set_manifest_ok(model.0);
                            })
                            .unwrap();
                        })
                        .await
                        .unwrap();
                    }
                    match do_sync(ui.clone(), config.as_ref(), manifest_ptr.as_ref()).await {
                        Ok(_) => {
                            tokio::task::spawn_blocking(move || {
                                ui.upgrade_in_event_loop(move |ui| {
                                    ui.invoke_set_sync_ok();
                                })
                                .unwrap()
                            })
                            .await
                            .unwrap();
                        }
                        Err(e) => {
                            println!("{e:?}");
                            let msg = SharedString::from(format!("{e:?}"));
                            tokio::task::spawn_blocking(move || {
                                ui.upgrade_in_event_loop(move |ui| {
                                    ui.invoke_set_sync_err(msg);
                                })
                                .unwrap()
                            })
                            .await
                            .unwrap();
                        }
                    };
                }
                Err(e) => {
                    println!("{e:?}");
                    let msg = SharedString::from(format!("{e:?}"));
                    tokio::task::spawn_blocking(move || {
                        ui.upgrade_in_event_loop(move |ui| {
                            ui.invoke_set_manifest_err(msg);
                        })
                        .unwrap()
                    })
                    .await
                    .unwrap();
                }
            }
            loop {
                let ui = ui2.clone();
                tokio::task::spawn_blocking(move || {
                    ui.upgrade_in_event_loop(move |ui| {
                        ui.window().request_redraw();
                    })
                    .unwrap()
                })
                .await
                .unwrap();
            }
        });
    }

    ui.run()?;

    drop(rt);
    drop(ui_ptr);
    drop(manifest_ptr);

    Ok(())
}

async fn req_manifest(config: &Config, manifest_ptr: &mut ClientManifest) -> anyhow::Result<()> {
    let server = &config.server;

    let mut res = surf::get(server.join("manifest")?)
        .await
        .map_err(|e| anyhow!(e))?;
    let manifest_bytes = res.body_bytes().await.map_err(|e| anyhow!(e))?;
    let manifest: HashMap<String, ManifestItem> = rmp_serde::from_slice(&manifest_bytes)?;

    *manifest_ptr = manifest
        .into_iter()
        .map(|a| ClientManifestItem {
            path: ((&a.0).into(), (&a.0).into(), a.0),
            op: a.1 .0,
            len: a.1 .1,
            hash: a.1 .2,
        })
        .collect();

    Ok(())
}

async fn do_sync(
    ui: Weak<AppWindow>,
    config: &'static Config,
    manifest_ptr: &'static ClientManifest,
) -> anyhow::Result<()> {
    let mut js = JoinSet::new();

    {
        let ui = ui.clone();
        tokio::task::spawn_blocking(move || {
            ui.upgrade_in_event_loop(move |ui| {
                ui.invoke_set_total_len(manifest_ptr.len().to_shared_string());
            })
        })
        .await??;
    }

    for index in 0..manifest_ptr.len() {
        let ui = ui.clone();
        js.spawn(async move {
            let manifest = &manifest_ptr[index];

            match do_sync_item(index, ui.clone(), config, manifest).await {
                Ok(_) => {
                    println!("Finish {index}");
                    tokio::task::spawn_blocking(move || {
                        ui.upgrade_in_event_loop(move |ui| {
                            ui.invoke_set_manifest_item_state(index as i32, ModelItemState::Finish);
                        })
                        .unwrap()
                    })
                    .await
                    .unwrap();
                }
                Err(e) => {
                    println!("Error {index} {e:?}");
                    tokio::task::spawn_blocking(move || {
                        ui.upgrade_in_event_loop(move |ui| {
                            ui.invoke_set_manifest_item_state(index as i32, ModelItemState::Error);
                        })
                        .unwrap()
                    })
                    .await
                    .unwrap();
                }
            };
        });
    }

    let mut count = 0usize;

    while let Some(r) = js.join_next().await {
        r?;

        {
            count += 1;
            let ui = ui.clone();
            let p = (count as f64 / manifest_ptr.len() as f64) as f32;
            let pp = p * 100f32;
            tokio::task::spawn_blocking(move || {
                ui.upgrade_in_event_loop(move |ui| {
                    ui.invoke_set_total_cur(count.to_shared_string(), p, format!("{pp:.2}").into());
                })
            })
            .await??;
        }
    }

    Ok(())
}

async fn do_sync_item(
    index: usize,
    ui: Weak<AppWindow>,
    config: &Config,
    item: &ClientManifestItem,
) -> anyhow::Result<()> {
    let server = &config.server;

    let mut path = config.cwd.clone();
    path.push(&item.path.0);
    let mut dir = path.clone();
    dir.pop();
    tokio::fs::create_dir_all(&dir).await?;

    match item.op {
        model::ItemOp::Sync => {
            if tokio::fs::try_exists(&path).await? {
                {
                    let ui = ui.clone();
                    tokio::task::spawn_blocking(move || {
                        ui.upgrade_in_event_loop(move |ui| {
                            ui.invoke_set_manifest_item_state(index as i32, ModelItemState::Hash);
                        })
                    })
                    .await??;
                }

                let mut file = tokio::fs::File::open(&path).await?;
                let total_size = file.metadata().await?.len();
                if total_size == item.len {
                    let hash = {
                        let ui = ui.clone();
                        {
                            let ui = ui.clone();
                            tokio::task::spawn_blocking(move || {
                                ui.upgrade_in_event_loop(move |ui| {
                                    ui.invoke_set_manifest_item_len(
                                        index as i32,
                                        total_size.to_shared_string(),
                                    );
                                })
                            })
                            .await??;
                        }
                        tokio::spawn(async move {
                            let mut hasher = Sha3_256::new();
                            let mut size = 0;
                            let mut start = Instant::now();
                            {
                                let mut buffer: [u8; 4096] = [0; 4096];
                                loop {
                                    use tokio::io::AsyncReadExt;

                                    let count = file.read(&mut buffer).await?;
                                    if count == 0 {
                                        break;
                                    }
                                    hasher.update(&buffer[..count]);
                                    size += count;

                                    let now = Instant::now();
                                    if (now - start).as_micros() > 500 {
                                        start = now;
                                        let p = (size as f64 / total_size as f64) as f32;
                                        let pp = p * 100f32;
                                        println!("Hash {index} : {pp:.2}% ; {size} / {total_size}");
                                        let ui = ui.clone();
                                        tokio::task::spawn_blocking(move || {
                                            ui.upgrade_in_event_loop(move |ui| {
                                                ui.invoke_set_manifest_item_cur(
                                                    index as i32,
                                                    size.to_shared_string(),
                                                    p,
                                                    format!("{pp:.2}").into(),
                                                );
                                            })
                                        })
                                        .await??;
                                    }
                                }
                            };
                            let hash = hasher.finalize();
                            anyhow::Result::<_>::Ok(hash.to_vec())
                        })
                        .await??
                    };

                    if item.hash == hash {
                        tokio::task::spawn_blocking(move || {
                            ui.upgrade_in_event_loop(move |ui| {
                                ui.invoke_set_manifest_item_state(
                                    index as i32,
                                    ModelItemState::NoOp,
                                );
                            })
                        })
                        .await??;
                        return Ok(());
                    }
                }
            }

            {
                let ui = ui.clone();
                tokio::task::spawn_blocking(move || {
                    ui.upgrade_in_event_loop(move |ui| {
                        ui.invoke_set_manifest_item_state(index as i32, ModelItemState::Sync);
                    })
                })
                .await??;
            }

            let api = server.join(&format!("content/{}", item.path.2))?;
            let mut res = surf::get(api).await.map_err(|e| anyhow!(e))?;
            let content_length: Option<usize> = res
                .header("Content-Length")
                .and_then(|a| a.as_str().parse().ok());
            let mut body = res.take_body();
            let total_size = content_length.or(body.len());
            if let Some(total_size) = total_size {
                let ui = ui.clone();
                tokio::task::spawn_blocking(move || {
                    ui.upgrade_in_event_loop(move |ui| {
                        ui.invoke_set_manifest_item_len(
                            index as i32,
                            total_size.to_shared_string(),
                        );
                    })
                })
                .await??;
            }
            let mut file = tokio::fs::File::create(&path).await?;
            tokio::spawn(async move {
                let mut buffer: [u8; 4096] = [0; 4096];
                let mut size = 0;
                let mut start = Instant::now();
                loop {
                    let len = body.read(&mut buffer).await?;
                    if len == 0 {
                        break;
                    }
                    file.write(&mut buffer[..len]).await?;
                    size += len;
                    if let Some(total_size) = total_size {
                        let now = Instant::now();
                        if (now - start).as_micros() > 500 {
                            start = now;
                            let p = (size as f64 / total_size as f64) as f32;
                            let pp = p * 100f32;
                            println!("Sync {index} : {pp:.2}% ; {size} / {total_size}");
                            let ui = ui.clone();
                            tokio::task::spawn_blocking(move || {
                                ui.upgrade_in_event_loop(move |ui| {
                                    ui.invoke_set_manifest_item_cur(
                                        index as i32,
                                        size.to_shared_string(),
                                        p,
                                        format!("{pp:.2}").into(),
                                    );
                                })
                            })
                            .await??;
                        }
                    }
                }
                println!("Sync {index} finish");
                return anyhow::Result::<()>::Ok(());
            })
            .await??;
            return Ok(());
        }
        model::ItemOp::Remove => {
            if !tokio::fs::try_exists(&path).await? {
                tokio::task::spawn_blocking(move || {
                    ui.upgrade_in_event_loop(move |ui| {
                        ui.invoke_set_manifest_item_state(index as i32, ModelItemState::NoOp);
                    })
                })
                .await??;
                return Ok(());
            }

            match config.delete_mode {
                DeleteMode::Rename => {
                    let mut dst = path.clone();
                    dst.set_extension(format!(
                        "{}.{}",
                        dst.extension()
                            .map(|a| a.to_string_lossy())
                            .as_deref()
                            .unwrap_or(""),
                        "del" // todo config
                    ));
                    // println!("{dst:?}");
                    tokio::fs::rename(&path, dst).await?;
                    return Ok(());
                }
                DeleteMode::Delete => {
                    tokio::fs::remove_file(&path).await?;
                    return Ok(());
                }
            }
        }
    }
}
