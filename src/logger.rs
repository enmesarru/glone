use std::path::PathBuf;

use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config as LogConfig, Root};
use log4rs::encode::pattern::PatternEncoder;

pub fn init_logger(log_path: &PathBuf) {
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} [{l}] - {m}{n}")))
        .build();

    // Create a file appender
    let file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} [{l}] - {m}{n}")))
        .build(log_path)
        .unwrap();

    let root = Root::builder()
        .appender("console")
        .appender("file")
        .build(LevelFilter::Info);

    let config = LogConfig::builder()
        .appender(Appender::builder().build("console", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .build(root)
        .unwrap();

    log4rs::init_config(config).unwrap();

    log::info!("Logger initialized");
}
