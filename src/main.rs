use indicatif::MultiProgress;

mod config;
mod git;
mod logger;

fn main() {
    let mut glone_options = config::GloneOptions::new();
    logger::init_logger(&glone_options.log_path);
    glone_options.load();

    let m = MultiProgress::new();

    match glone_options.config {
        Some(config) => {
            for provider in config.get_providers().iter() {
                log::info!("Starting the cloning for {}", provider.url);

                git::download(provider, &m)
            }
        }
        None => todo!(),
    }
}
