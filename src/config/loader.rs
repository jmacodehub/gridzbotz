use super::*;
use config::{Config as ConfigBuilder, ConfigError, Environment, File};
use std::env;

pub struct ConfigLoader {
    builder: ConfigBuilder,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            builder: ConfigBuilder::builder()
                .set_default("bot.name", "GridBot")
                .unwrap()
                .build()
                .unwrap()
                .try_into()
                .unwrap(),
        }
    }
    
    pub fn with_defaults(mut self) -> Self {
        self.builder = ConfigBuilder::builder()
            .add_source(File::with_name("config/default"))
            .build()
            .unwrap();
        self
    }
    
    pub fn with_environment(mut self) -> Self {
        let env = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
        
        self.builder = ConfigBuilder::builder()
            .add_source(self.builder)
            .add_source(File::with_name(&format!("config/{}", env)).required(false))
            .build()
            .unwrap();
        self
    }
    
    pub fn with_env_file(mut self) -> Self {
        dotenv::dotenv().ok();
        
        self.builder = ConfigBuilder::builder()
            .add_source(self.builder)
            .add_source(Environment::default().separator("__"))
            .build()
            .unwrap();
        self
    }
    
    pub fn with_cli_overrides(self) -> Self {
        // CLI overrides handled by clap in main.rs
        self
    }
    
    pub fn build(self) -> Result<Config> {
        let config: Config = self.builder.try_deserialize()
            .context("Failed to parse configuration")?;
        
        config.validate()?;
        Ok(config)
    }
    
    pub fn from_file(path: PathBuf) -> Result<Config> {
        let builder = ConfigBuilder::builder()
            .add_source(File::from(path))
            .build()?;
        
        let config: Config = builder.try_deserialize()?;
        config.validate()?;
        Ok(config)
    }
}
