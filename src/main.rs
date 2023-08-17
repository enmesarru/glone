use clap::Parser;
use indicatif::MultiProgress;

mod config;
mod git;
mod logger;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short = 'i', long)]
    info: bool,
}

fn main() {
    let args = Args::parse();

    let mut glone_options = config::GloneOptions::new();
    logger::init_logger(&glone_options.log_path);
    glone_options.load();

    if args.info {
        println!("Config path: {}", glone_options.config_dir_path.display());
        return;
    }

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
