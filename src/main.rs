fn main() {
    let mut glone_options = app::GloneOptions::new();
    logger::init_logger(&glone_options.log_path);
    glone_options.load();

    match glone_options.config {
        Some(config) => {
            for provider in config.get_providers().iter() {
                log::info!("Starting the cloning for {}", provider.url);

                git::download(provider)
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
        pub branch: String,
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
    use std::{io::Write, path::Path};

    use git2::{Cred, RemoteCallbacks, Repository};

    use crate::config::{AuthType, Provider};

    pub fn download(provider: &Provider) {
        let repo_path = Path::new(&provider.sync_dir);

        if repo_path.exists() && repo_path.is_dir() {
            pull(provider, repo_path);
        } else {
            clone(provider, repo_path);
        }
    }

    fn pull(provider: &Provider, repo_path: &Path) {
        let repo = match Repository::open(repo_path) {
            Ok(repo) => repo,
            Err(e) => {
                log::error!("Failed to open the repository {:?} {}", repo_path, e);
                panic!("Failed to open the repository {:?} {}", repo_path, e)
            }
        };

        let mut remote = repo.find_remote("origin").unwrap();
        let fetch_commit = fetch(&repo, &[&provider.branch], &mut remote).unwrap();

        let _ = merge(&repo, &provider.branch, fetch_commit);
    }

    fn clone(provider: &Provider, repo_path: &Path) {
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
        let _ = builder
            .branch(&provider.branch)
            .clone(&provider.url, repo_path);
    }

    fn fetch<'a>(
        repo: &'a git2::Repository,
        refs: &[&str],
        remote: &'a mut git2::Remote,
    ) -> Result<git2::AnnotatedCommit<'a>, git2::Error> {
        let mut cb = git2::RemoteCallbacks::new();

        // Print out our transfer progress.
        cb.transfer_progress(|stats| {
            if stats.received_objects() == stats.total_objects() {
                print!(
                    "Resolving deltas {}/{}\r",
                    stats.indexed_deltas(),
                    stats.total_deltas()
                );
            } else if stats.total_objects() > 0 {
                print!(
                    "Received {}/{} objects ({}) in {} bytes\r",
                    stats.received_objects(),
                    stats.total_objects(),
                    stats.indexed_objects(),
                    stats.received_bytes()
                );
            }
            std::io::stdout().flush().unwrap();
            true
        });

        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(cb);

        fo.download_tags(git2::AutotagOption::All);
        log::info!("Fetching {} for repo", remote.name().unwrap());
        remote.fetch(refs, Some(&mut fo), None)?;

        let stats = remote.stats();
        if stats.local_objects() > 0 {
            log::info!(
                "\rReceived {}/{} objects in {} bytes (used {} local \
                 objects)",
                stats.indexed_objects(),
                stats.total_objects(),
                stats.received_bytes(),
                stats.local_objects()
            );
        } else {
            log::info!(
                "\rReceived {}/{} objects in {} bytes",
                stats.indexed_objects(),
                stats.total_objects(),
                stats.received_bytes()
            );
        }

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        Ok(repo.reference_to_annotated_commit(&fetch_head)?)
    }

    fn fast_forward(
        repo: &Repository,
        lb: &mut git2::Reference,
        rc: &git2::AnnotatedCommit,
    ) -> Result<(), git2::Error> {
        let name = match lb.name() {
            Some(s) => s.to_string(),
            None => String::from_utf8_lossy(lb.name_bytes()).to_string(),
        };
        let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
        log::info!("{}", msg);
        lb.set_target(rc.id(), &msg)?;
        repo.set_head(&name)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        Ok(())
    }

    fn normal_merge(
        repo: &Repository,
        local: &git2::AnnotatedCommit,
        remote: &git2::AnnotatedCommit,
    ) -> Result<(), git2::Error> {
        let local_tree = repo.find_commit(local.id())?.tree()?;
        let remote_tree = repo.find_commit(remote.id())?.tree()?;
        let ancestor = repo
            .find_commit(repo.merge_base(local.id(), remote.id())?)?
            .tree()?;
        let mut idx = repo.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

        if idx.has_conflicts() {
            println!("Merge conflicts detected...");
            repo.checkout_index(Some(&mut idx), None)?;
            return Ok(());
        }
        let result_tree = repo.find_tree(idx.write_tree_to(repo)?)?;
        // now create the merge commit
        let msg = format!("Merge: {} into {}", remote.id(), local.id());
        let sig = repo.signature()?;
        let local_commit = repo.find_commit(local.id())?;
        let remote_commit = repo.find_commit(remote.id())?;
        // Do our merge commit and set current branch head to that commit.
        let _merge_commit = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &msg,
            &result_tree,
            &[&local_commit, &remote_commit],
        )?;
        // Set working tree to match head.
        repo.checkout_head(None)?;
        Ok(())
    }

    fn merge<'a>(
        repo: &'a Repository,
        remote_branch: &str,
        fetch_commit: git2::AnnotatedCommit<'a>,
    ) -> Result<(), git2::Error> {
        let analysis = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.0.is_fast_forward() {
            log::info!("Doing a fast forward");
            let refname = format!("refs/heads/{}", remote_branch);
            match repo.find_reference(&refname) {
                Ok(mut r) => {
                    fast_forward(repo, &mut r, &fetch_commit)?;
                }
                Err(_) => {
                    // The branch doesn't exist so just set the reference to the
                    // commit directly. Usually this is because you are pulling
                    // into an empty repository.
                    repo.reference(
                        &refname,
                        fetch_commit.id(),
                        true,
                        &format!("Setting {} to {}", remote_branch, fetch_commit.id()),
                    )?;
                    repo.set_head(&refname)?;
                    repo.checkout_head(Some(
                        git2::build::CheckoutBuilder::default()
                            .allow_conflicts(true)
                            .conflict_style_merge(true)
                            .force(),
                    ))?;
                }
            };
        } else if analysis.0.is_normal() {
            let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
            normal_merge(&repo, &head_commit, &fetch_commit)?;
        } else {
            log::info!("There is nothing to do...");
        }
        Ok(())
    }
}
