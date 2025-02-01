#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

use std::{error::Error, fs, path::PathBuf, str::FromStr};

mod boxed_ptr;
mod client_model;
mod utils;
mod winit_helper;
use boxed_ptr::*;
use client_model::*;
use url::Url;
use utils::*;
use winit_helper::*;

slint::include_modules!();

const CONFIG_PATH: &'static str = "syner.toml";

fn main() -> Result<(), Box<dyn Error>> {
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

fn setup_window(
    config_data_ptr: &BoxPtr<Config>,
    config_path: PathBuf,
) -> Result<bool, Box<dyn Error>> {
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

fn main_window(config_data_ptr: &BoxPtr<Config>) -> Result<(), Box<dyn Error>> {
    let ui_ptr = BoxPtr::new(AppWindow::new()?);

    let ui = ui_ptr.as_mut();

    let config_data = config_data_ptr.as_mut();
    ui.invoke_set_data(config_data.to_view_model());

    center_window(ui.window());
    ui.invoke_hide();
    ui.show()?;
    center_window(ui.window());
    ui.invoke_show();
    set_blur_tab(ui.window());
    ui.run()?;

    Ok(())
}
