// SSH客户端封装
use crate::config::{AuthMethod, Config};
use crate::utils::error::TransferError;
use anyhow::{Context, Result};
use ssh2::Session;
use std::io::prelude::*;
use std::net::TcpStream;

pub struct SshClient {
    pub session: Session,
}

impl SshClient {
    pub fn connect(config: &Config) -> Result<Self> {
        // println!("Connecting to {}:{}...", config.host, config.port);
        
        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))
            .context("Failed to connect to SSH server")?;
        
        let mut session = Session::new().context("Failed to create SSH session")?;
        session.set_tcp_stream(tcp);
        session.handshake().context("SSH handshake failed")?;

        // println!("SSH handshake completed. Authenticating...");

        // Authentication
        match &config.auth {
            AuthMethod::Password(password) => {
                // println!("Authenticating with password...");
                session
                    .userauth_password(&config.username, password)
                    .context("Password authentication failed")?;
            }
            AuthMethod::PublicKey(key_path) => {
                // println!("Authenticating with SSH key: {}", key_path.display());
                session
                    .userauth_pubkey_file(&config.username, None, key_path, None)
                    .context("Public key authentication failed")?;
            }
            AuthMethod::Agent => {
                // println!("Authenticating with SSH agent...");
                session
                    .userauth_agent(&config.username)
                    .context("SSH agent authentication failed")?;
            }
        }

        if !session.authenticated() {
            return Err(TransferError::AuthenticationFailed.into());
        }

        // println!("Authentication successful!");
        Ok(SshClient { session })
    }

    pub fn sftp(&self) -> Result<ssh2::Sftp> {
        self.session.sftp().context("Failed to create SFTP session")
    }

    pub fn exec(&self, command: &str) -> Result<String> {
        let mut channel = self.session.channel_session()
            .context("Failed to create SSH channel")?;
        
        channel.exec(command)
            .context("Failed to execute command")?;
        
        let mut output = String::new();
        channel.read_to_string(&mut output)
            .context("Failed to read command output")?;
        
        channel.wait_close()
            .context("Failed to close channel")?;
        
        Ok(output)
    }
}