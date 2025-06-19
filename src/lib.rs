// 库文件，导出模块
pub mod cli;
pub mod config;
pub mod ssh;
pub mod transfer;
pub mod utils;
pub mod threadpool;

use anyhow::Result;
use config::Config;
use transfer::{download::Downloader, upload::Uploader};

pub async fn run_transfer(config: Config) -> Result<()> {
    match config.operation.clone() {
        config::Operation::Download { remote_path, local_path, recursive } => {
            let downloader = Downloader::new(config)?;
            downloader.download(&remote_path, &local_path, recursive).await
        }
        config::Operation::Upload { local_path, remote_path, recursive } => {
            let uploader = Uploader::new(config)?;
            uploader.upload(&local_path, &remote_path, recursive).await
        }
    }
}
