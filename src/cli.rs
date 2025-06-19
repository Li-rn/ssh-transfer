// 命令行参数解析
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ssh-transfer")]
#[command(about = "A multi-threaded SSH file transfer tool")]
#[command(version = "0.1.0")]
pub struct Cli {
    /// SSH server hostname or IP address
    #[arg(short = 'H', long)]
    pub host: String,

    /// SSH server port
    #[arg(short, long, default_value = "22")]
    pub port: u16,

    /// SSH username
    #[arg(short, long)]
    pub username: String,

    /// SSH password (if not provided, will prompt for input)
    #[arg(short = 'P', long)]
    pub password: Option<String>,

    /// SSH private key file path
    #[arg(short, long)]
    pub key_file: Option<PathBuf>,

    /// Use SSH agent for authentication
    #[arg(long)]
    pub use_agent: bool,

    /// Number of parallel threads
    #[arg(short, long, default_value = "4")]
    pub threads: usize,

    /// Chunk size in bytes
    #[arg(short, long, default_value = "1048576")]
    pub chunk_size: usize,

    /// Enable resume capability
    #[arg(short, long)]
    pub resume: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
#[derive(Debug)]
pub enum Commands {
    /// Download files from remote server
    Download {
        /// Remote file or directory path
        remote_path: String,
        /// Local destination path
        local_path: PathBuf,
        /// Recursively download directories
        #[arg(short, long)]
        recursive: bool,
    },
    /// Upload files to remote server
    Upload {
        /// Local file or directory path
        local_path: PathBuf,
        /// Remote destination path
        remote_path: String,
        /// Recursively upload directories
        #[arg(short, long)]
        recursive: bool,
    },
}