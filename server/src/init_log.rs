use chrono::prelude::*;
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use std::time::SystemTime;

pub fn init_logger() -> anyhow::Result<()> {
    let now = Local::now().format("%Y-%m-%d-%H-%M-%S");

    let pattern = Box::new(PatternEncoder::new(
        "[{d(%Y-%m-%d %H:%M:%S%.6f)}][{h({l})}][{t}] {m}{n}",
    ));
    let stdout = ConsoleAppender::builder().encoder(pattern.clone()).build();

    let file = FileAppender::builder()
        .encoder(pattern)
        .build(format!("./logs/{now}.log"))?;
    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .build(
            Root::builder()
                .appenders(["stdout", "file"])
                .build(LevelFilter::Info),
        )?;

    log4rs::init_config(config)?;
    Ok(())
}
