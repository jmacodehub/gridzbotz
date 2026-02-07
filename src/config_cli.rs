use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "solana-grid-bot")]
pub struct CliArgs {
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[arg(short, long)]
    pub environment: Option<String>,

    #[arg(short = 'm', long)]
    pub mode: Option<String>,

    #[arg(short, long)]
    pub rpc: Option<String>,

    #[arg(short, long)]
    pub verbose: bool,
}

impl CliArgs {
    #[allow(dead_code)]
    pub fn get_config_path() -> PathBuf {
        let args = Self::parse();

        if let Some(path) = args.config {
            return path;
        }

        if let Ok(env_path) = std::env::var("CONFIG_PATH") {
            return PathBuf::from(env_path);
        }

        let env = std::env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string());

        match env.as_str() {
            "production" => PathBuf::from("config/production/mainnet.toml"),
            "staging" => PathBuf::from("config/staging/hornet.toml"),
            _ => PathBuf::from("config/development/master.toml"),
        }
    }
}
