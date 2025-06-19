// SSH会话管理
use crate::config::Config;
use crate::ssh::SshClient;
use anyhow::Result;
use std::sync::Arc;

pub struct SshSession {
    pub client: Arc<SshClient>,
    pub config: Arc<Config>,
}

impl SshSession {
    pub fn new(config: Config) -> Result<Self> {
        let client = Arc::new(SshClient::connect(&config)?);
        let config = Arc::new(config);
        
        Ok(SshSession { client, config })
    }

    pub fn clone_session(&self) -> Result<SshClient> {
        SshClient::connect(&self.config)
    }
}