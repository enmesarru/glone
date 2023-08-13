fn main() {
    let mut glone_options = app::GloneOptions::new();
    logger::init_logger(&glone_options.log_path);
    glone_options.load();

    match glone_options.config {
        Some(config) => {
            for provider in config.get_providers().iter() {
                log::info!("Starting the cloning for {}", provider.url);

                git::clone(provider)
            }
        }
        None => todo!(),
    }
}

pub mod app {
    use std::path::{Path, PathBuf};

    use dirs::config_dir;

    use crate::config::Config;

    pub struct GloneOptions {
        config_dir_path: PathBuf,
        config_path: PathBuf,
        pub log_path: PathBuf,
        pub config: Option<Config>,
    }

    impl GloneOptions {
        const CONFIG_PATH: &str = ".glone";
        const CONFIG_FILE: &str = "config.yaml";
        const LOG_FILE: &str = "app.log";

        pub fn new() -> GloneOptions {
            let user_config_dir = config_dir().unwrap();
            let config_dir_path = Path::new(&user_config_dir).join(Self::CONFIG_PATH);
            let config_path = Path::new(&config_dir_path).join(Self::CONFIG_FILE);
            let log_path = Path::new(&config_dir_path).join(Self::LOG_FILE);

            return GloneOptions {
                config_dir_path,
                config_path,
                log_path,
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
                        log::info!("Creating an app config folder.");
                    }
                    Err(_) => {
                        log::error!("Error occured when creating app config folder");
                        panic!()
                    }
                }
            }
        }

        fn create_config_file(&self) {
            if self.config_dir_path.exists() && !self.config_path.exists() {
                match std::fs::File::create(&self.config_path) {
                    Ok(_) => {
                        log::info!("Created an empty config file. {:?}", self.config_path);
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
                    if c.len() == 0 {
                        log::error!("Error: The config file is empty.");
                        return;
                    }

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
    use std::env;

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Config {
        providers: Vec<Provider>,
    }

    impl Config {
        pub fn get_providers(&self) -> &Vec<Provider> {
            self.providers.as_ref()
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Provider {
        pub name: String,
        pub url: String,
        pub sync_dir: String,
        pub auth: Auth,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Auth {
        pub r#type: AuthType,
        pub username: Option<String>,
        pub password: Option<String>,
        pub path: Option<String>,
    }

    impl Auth {
        pub fn is_valid_cred(&self) -> bool {
            self.username.as_ref().unwrap().starts_with('_')
                && self.password.as_ref().unwrap().starts_with('_')
        }

        pub fn get_username(&self) -> String {
            env::var(self.username.as_ref().unwrap()).unwrap()
        }

        pub fn get_password(&self) -> String {
            env::var(self.password.as_ref().unwrap()).unwrap()
        }

        pub fn get_ssh_path(&self) -> &String {
            self.password.as_ref().unwrap()
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum AuthType {
        #[serde(rename = "token")]
        Token,
        #[serde(rename = "ssh")]
        Ssh,
        #[serde(rename = "public")]
        Public,
    }
}

pub mod logger {
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
}

pub mod git {
    use std::path::Path;

    use git2::{Cred, RemoteCallbacks};

    use crate::config::{AuthType, Provider};

    pub fn clone(provider: &Provider) {
        let mut callbacks = RemoteCallbacks::new();

        match provider.auth.r#type {
            AuthType::Token => {
                if provider.auth.is_valid_cred() {
                    callbacks.credentials(|_url, _username_from_url, _allowed_types| {
                        Cred::userpass_plaintext(
                            &provider.auth.get_username(),
                            &provider.auth.get_password(),
                        )
                    });
                }
            }
            AuthType::Ssh => {
                callbacks.credentials(|_url, username_from_url, _allowed_types| {
                    Cred::ssh_key(
                        username_from_url.unwrap(),
                        None,
                        Path::new(provider.auth.get_ssh_path()),
                        None,
                    )
                });
            }
            AuthType::Public => {}
        }

        // Prepare fetch options.
        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);

        // Prepare builder.
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        // Clone the project.
        let _ = builder.clone(&provider.url, Path::new(&provider.sync_dir));
    }
}
