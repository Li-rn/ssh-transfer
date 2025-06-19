// 配置管理
use crate::cli::{Cli, Commands};
use anyhow::{Context, Result};
use dialoguer::{Confirm, Password};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
    pub threads: usize,
    pub chunk_size: usize,
    pub resume: bool,
    pub verbose: bool,
    pub operation: Operation,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    Password(String),
    PublicKey(PathBuf),
    Agent,
}

#[derive(Debug, Clone)]
pub enum Operation {
    Download {
        remote_path: String,
        local_path: PathBuf,
        recursive: bool,
    },
    Upload {
        local_path: PathBuf,
        remote_path: String,
        recursive: bool,
    },
}

impl Config {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let auth = Self::determine_auth_method(cli)?;

        let operation = match &cli.command {
            Commands::Download { remote_path, local_path, recursive } => {
                Operation::Download {
                    remote_path: remote_path.clone(),
                    local_path: local_path.clone(),
                    recursive: *recursive,
                }
            }
            Commands::Upload { local_path, remote_path, recursive } => {
                Operation::Upload {
                    local_path: local_path.clone(),
                    remote_path: remote_path.clone(),
                    recursive: *recursive,
                }
            }
        };

        Ok(Config {
            host: cli.host.clone(),
            port: cli.port,
            username: cli.username.clone(),
            auth,
            threads: cli.threads,
            chunk_size: cli.chunk_size,
            resume: cli.resume,
            verbose: cli.verbose,
            operation,
        })
    }

    fn determine_auth_method(cli: &Cli) -> Result<AuthMethod> {
        // 如果命令行提供了密码，直接使用
        if let Some(password) = &cli.password {
            return Ok(AuthMethod::Password(password.clone()));
        }

        // 如果指定了使用 SSH Agent
        if cli.use_agent {
            return Ok(AuthMethod::Agent);
        }

        // 如果提供了密钥文件路径
        if let Some(key_file) = &cli.key_file {
            return Ok(AuthMethod::PublicKey(key_file.clone()));
        }

        // 尝试查找默认的SSH密钥
        let home = home::home_dir().context("Cannot determine home directory")?;
        let ssh_dir = home.join(".ssh");
        
        // 检查常见的SSH密钥文件
        let key_files = ["id_rsa", "id_ed25519", "id_ecdsa"];
        for key_name in &key_files {
            let key_path = ssh_dir.join(key_name);
            if key_path.exists() {
                println!("Found SSH key: {}", key_path.display());
                let use_key = Confirm::new()
                    .with_prompt(format!("Use SSH key {} for authentication?", key_path.display()))
                    .default(true)
                    .interact()?;
                
                if use_key {
                    return Ok(AuthMethod::PublicKey(key_path));
                }
            }
        }

        // 如果没有找到密钥或用户不想使用密钥，提示输入密码
        println!("No SSH key found or selected.");
        let password = Password::new()
            .with_prompt(format!("Enter password for {}@{}", cli.username, cli.host))
            .interact()?;

        Ok(AuthMethod::Password(password))
    }
}