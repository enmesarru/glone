fn main() {
    logger::init_logger();

    let mut glone_options = app::GloneOptions::new();
    glone_options.load();

    log::info!("Logger initialized");
}

pub mod app {
    use std::path::{Path, PathBuf};

    use dirs::config_dir;

    use crate::config::Config;

    pub struct GloneOptions {
        config_dir_path: PathBuf,
        config_path: PathBuf,
        config: Option<Config>,
    }

    impl GloneOptions {
        const CONFIG_PATH: &str = ".glone";
        const CONFIG_FILE: &str = "config.yaml";

        pub fn new() -> GloneOptions {
            let user_config_dir = config_dir().unwrap();
            let config_dir_path = Path::new(&user_config_dir).join(Self::CONFIG_PATH);
            let config_path = Path::new(&config_dir_path).join(Self::CONFIG_FILE);

            return GloneOptions {
                config_dir_path,
                config_path,
                config: None,
            };
        }

        pub fn load(&mut self) {
            self.create_config_dir();
            self.create_config_file();
            self.load_config();
        }

        fn create_config_dir(&self) {
            if !self.config_dir_path.exists() {
                match std::fs::create_dir_all(&self.config_dir_path) {
                    Ok(_) => {
                        log::info!("Creating an app config folder .");
                    }
                    Err(_) => {
                        log::error!("Error occured when creating app config folder");
                        panic!()
                    }
                }
            }
        }

        fn create_config_file(&self) {
            if self.config_dir_path.exists() {
                match std::fs::File::create(&self.config_path) {
                    Ok(_) => {
                        log::info!("Created an empty config file.");
                    }
                    Err(err) => {
                        log::error!("Error occured when creating empty config folder, {:?}", err);
                    }
                }
            }
        }

        fn load_config(&mut self) {
            let content = std::fs::read_to_string(&self.config_path);

            match content {
                Ok(c) => {
                    let config: Result<Config, serde_yaml::Error> = serde_yaml::from_str(&c);

                    match config {
                        Ok(conf) => self.config = Some(conf),
                        Err(error) => {
                            log::error!("Error occured when setting config {:?}", error);
                        }
                    }
                }
                Err(error) => {
                    log::error!("Error occured when loading config file. {:?}", error);
                }
            }
        }
    }
}

pub mod config {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Config {
        providers: Vec<Provider>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Provider {
        name: String,
        url: String,
        auth: Auth,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Auth {
        r#type: AuthType,
        username: String,
        password: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    enum AuthType {
        #[serde(rename = "token")]
        Token,
        #[serde(rename = "ssh")]
        Ssh,
    }
}

pub mod logger {
    use log::LevelFilter;
    use log4rs::append::console::ConsoleAppender;
    use log4rs::append::file::FileAppender;
    use log4rs::config::{Appender, Config as LogConfig, Root};
    use log4rs::encode::pattern::PatternEncoder;

    pub fn init_logger() {
        let stdout = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{d} [{l}] - {m}{n}")))
            .build();

        // Create a file appender
        let file = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{d} [{l}] - {m}{n}")))
            .build("log/app.log")
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
    }
}