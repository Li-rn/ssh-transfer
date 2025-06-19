// 下载功能
use crate::config::Config;
use crate::ssh::SshSession;
use crate::transfer::progress::ProgressTracker;
use crate::utils::error::TransferError;
use anyhow::{Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use ssh2::Sftp;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::pin::Pin;

pub struct Downloader {
    session: SshSession,
    config: Arc<Config>,
    remote_home: String,
}

impl Downloader {
    pub fn new(config: Config) -> Result<Self> {
        let session = SshSession::new(config.clone())?;
        
        // 首先获取远程系统的家目录
        let remote_home = Self::detect_remote_home_dir(&session, &config.username)?;
        
        Ok(Self {
            session,
            config: Arc::new(config),
            remote_home,
        })
    }
    
    // 检测远程系统的家目录
    fn detect_remote_home_dir(session: &SshSession, username: &str) -> Result<String> {
        // 尝试通过SSH命令获取HOME目录
        match session.client.exec("echo $HOME") {
            Ok(output) => {
                let home = output.trim();
                if !home.is_empty() {
                    println!("Detected remote home directory: {}", home);
                    return Ok(home.to_string());
                }
            }
            Err(e) => {
                println!("Warning: Could not detect remote home directory: {}", e);
            }
        }
        
        // 回退到标准Linux路径
        let default_home = format!("/home/{}", username);
        println!("Using default remote home directory: {}", default_home);
        Ok(default_home)
    }

    pub async fn download<P: AsRef<Path>>(
        &self,
        remote_path_str: &str,
        local_path: P,
        recursive: bool,
    ) -> Result<()> {
        let local_path = local_path.as_ref();
        let sftp = self.session.client.sftp()?;

        // 检查常见路径错误 - 检测shell扩展的本地路径
        if remote_path_str.starts_with("/Users/") {
            return Err(anyhow::anyhow!(
                "错误: 远程路径 '{}' 看起来是本地 macOS 路径，而不是远程路径。\n\
                 要从远程主目录下载，请使用引号: '~'\n\
                 例如: ./ssh-transfer -H host -u user download '~/file.txt' .",
                remote_path_str
            ));
        }

        // 解析远程路径（处理 ~, . 等特殊情况）
        let remote_path = self.resolve_remote_path_str(remote_path_str);
        println!("Resolved remote path: {}", remote_path);

        // 检查远程文件是否存在
        let remote_stat = match sftp.stat(Path::new(&remote_path)) {
            Ok(stat) => stat,
            Err(e) => return Err(anyhow::anyhow!("Remote file does not exist: {}: {}", remote_path, e))
        };

        if remote_stat.is_dir() {
            if recursive {
                let file_name = Path::new(&remote_path)
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Cannot determine directory name from remote path"))?;
                let new_local_path = local_path.join(file_name);
                self.download_directory(&sftp, &remote_path, &new_local_path).await
            } else {
                Err(TransferError::DirectoryNotAllowed.into())
            }
        } else {
            let file_size = remote_stat.size.unwrap_or(0);
            // 确定目标文件路径
            let target_path = self.get_target_file_path(local_path, &remote_path)?;
            self.download_file(&sftp, &remote_path, &target_path, file_size).await
        }
    }

    // 解析远程路径字符串，处理波浪线等特殊字符
    fn resolve_remote_path_str(&self, remote_path: &str) -> String {
        // 检查路径是否看起来像本地扩展的主目录
        if remote_path.starts_with("/home/") && !remote_path.starts_with(&self.remote_home) {
            println!("警告: 远程路径看起来是本地路径。如果您想使用远程主目录，请使用引号: '~'");
        }

        if remote_path.starts_with("~/") || remote_path == "~" {
            // 处理 ~ 显式用于远程路径
            if remote_path.len() > 1 {
                format!("{}{}", self.remote_home, &remote_path[1..])
            } else {
                self.remote_home.clone()
            }
        } else if remote_path == "." || remote_path == "./" {
            self.remote_home.clone()
        } else {
            remote_path.to_string()
        }
    }

    // 获取本地目标文件路径
    fn get_target_file_path(&self, local_dir: &Path, remote_path: &str) -> Result<PathBuf> {
        if local_dir.exists() && local_dir.is_dir() {
            // 如果本地路径是目录，在该目录下创建与远程文件同名的文件
            let file_name = Path::new(remote_path)
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Cannot determine file name from remote path"))?;
            
            Ok(local_dir.join(file_name))
        } else if let Some(parent) = local_dir.parent() {
            // 检查父目录是否存在
            if parent.exists() || parent.as_os_str().is_empty() {
                Ok(local_dir.to_path_buf())
            } else {
                // 尝试创建父目录
                std::fs::create_dir_all(parent)?;
                Ok(local_dir.to_path_buf())
            }
        } else {
            // 没有父目录，直接使用本地路径
            Ok(local_dir.to_path_buf())
        }
    }

    // 实现带断点续传的文件下载
    async fn download_file(&self, sftp: &Sftp, remote_path: &str, local_path: &Path, file_size: u64) -> Result<()> {
        println!("Downloading file: {} -> {} ({} bytes)", remote_path, local_path.display(), file_size);

        // 断点续传逻辑：检查本地文件是否存在
        let mut offset = 0;
        let progress = ProgressTracker::new(file_size, &format!("Downloading {}", remote_path));
        
        if self.config.resume && local_path.exists() {
            // 获取本地文件大小
            let metadata = std::fs::metadata(local_path)?;
            let local_size = metadata.len();
            
            // 确保本地文件不大于远程文件
            if local_size <= file_size {
                offset = local_size;
                println!("Resuming download from offset: {} bytes", offset);
                progress.update(offset); // 更新进度条以显示已下载部分
            } else {
                println!("Local file is larger than remote file. Starting download from beginning.");
                // 本地文件异常，删除并重新开始
                std::fs::remove_file(local_path)?;
            }
        }

        // 创建或打开本地文件
        let mut local_file = if offset > 0 {
            // 以追加模式打开文件
            OpenOptions::new()
                .write(true)
                .append(true)
                .open(local_path)?
        } else {
            // 创建新文件
            File::create(local_path)?
        };

        // 打开远程文件并设置偏移量
        let mut remote_file = sftp.open(Path::new(remote_path))?;
        if offset > 0 {
            remote_file.seek(SeekFrom::Start(offset))?;
        }

        let mut buffer = vec![0u8; self.config.chunk_size];
        let mut total_transferred = offset;

        loop {
            match remote_file.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(bytes_read) => {
                    local_file.write_all(&buffer[..bytes_read])?;
                    total_transferred += bytes_read as u64;
                    progress.update(total_transferred);
                }
                Err(e) => {
                    progress.finish_with_error(&e.to_string());
                    return Err(e.into());
                }
            }
        }

        // 确保数据写入磁盘
        local_file.flush()?;

        progress.finish();
        println!("✅ Download completed: {}", local_path.display());
        Ok(())
    }

    async fn download_directory(&self, sftp: &Sftp, remote_dir: &str, local_dir: &Path) -> Result<()> {
        // 确保本地目录存在
        if !local_dir.exists() {
            std::fs::create_dir_all(local_dir)?;
        } else if !local_dir.is_dir() {
            return Err(anyhow::anyhow!("Local path exists but is not a directory: {}", local_dir.display()));
        }

        // 获取远程目录内容
        let entries = sftp.readdir(Path::new(remote_dir))?;
        
        // 计算总下载大小
        let mut total_size = 0u64;
        let mut files_to_download = Vec::new();
        
        for (path, stat) in entries {
            let path_str = path.to_string_lossy().to_string();
            let file_name = path.file_name()
                .ok_or_else(|| anyhow::anyhow!("Cannot determine file name"))?
                .to_string_lossy().to_string();
            let local_path = local_dir.join(&file_name);
            
            if stat.is_dir() {
                // 递归处理子目录 - 用 Box::pin 包装异步调用
                let remote_subdir = format!("{}/{}", remote_dir, file_name);
                // 创建目标子目录
                let file_name = Path::new(&remote_subdir)
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Cannot determine directory name from remote path"))?;
                let new_local_path = local_path.join(file_name);
                let future = self.download_directory(sftp, &remote_subdir, &new_local_path);
                Pin::from(Box::new(future)).await?;
            } else {
                // 添加文件到下载列表
                let size = stat.size.unwrap_or(0);
                
                // 如果启用断点续传，检查本地文件
                let mut effective_size = size;
                if self.config.resume && local_path.exists() {
                    if let Ok(metadata) = std::fs::metadata(&local_path) {
                        let local_size = metadata.len();
                        if local_size < size {
                            // 只下载剩余部分
                            effective_size = size - local_size;
                        } else if local_size == size {
                            // 文件已完成，跳过
                            println!("Skipping already downloaded file: {}", local_path.display());
                            continue;
                        }
                    }
                }
                
                total_size += effective_size;
                files_to_download.push((path_str, local_path, size));
            }
        }

        let progress = Arc::new(ProgressTracker::new(total_size, "Downloading files"));

        // 创建下载任务
        let (tx, rx): (Sender<DownloadTask>, Receiver<DownloadTask>) = bounded(100);

        // 启动工作线程
        let mut handles = Vec::new();
        for _ in 0..self.config.threads {
            let rx = rx.clone();
            let session = self.session.clone_session()?;
            let config = Arc::clone(&self.config);
            let progress = Arc::clone(&progress);

            let handle = thread::spawn(move || {
                let sftp = session.sftp().unwrap();
                while let Ok(task) = rx.recv() {
                    if let Err(e) = Self::download_file_worker(&sftp, &task, &config) {
                        eprintln!("Download error for {}: {}", task.remote_path, e);
                    } else {
                        progress.add_bytes(task.effective_size);
                    }
                }
            });
            handles.push(handle);
        }

        // 发送下载任务
        for (remote_path, local_path, size) in files_to_download {
            let mut offset = 0;
            let mut effective_size = size;
            
            // 如果启用断点续传，计算偏移量
            if self.config.resume && local_path.exists() {
                if let Ok(metadata) = std::fs::metadata(&local_path) {
                    let local_size = metadata.len();
                    if local_size < size {
                        offset = local_size;
                        effective_size = size - local_size;
                    }
                }
            }
            
            let task = DownloadTask {
                remote_path,
                local_path,
                offset,
                effective_size,
            };
            tx.send(task)?;
        }
        drop(tx);

        // 等待所有工作线程完成
        for handle in handles {
            handle.join().map_err(|_| TransferError::ThreadJoinError)?;
        }

        progress.finish();
        Ok(())
    }

    fn download_file_worker(sftp: &Sftp, task: &DownloadTask, config: &Config) -> Result<()> {
        // 准备本地文件
        let mut local_file = if task.offset > 0 {
            OpenOptions::new()
                .write(true)
                .append(true)
                .open(&task.local_path)?
        } else {
            File::create(&task.local_path)?
        };
        
        // 打开远程文件并设置偏移量
        let mut remote_file = sftp.open(Path::new(&task.remote_path))?;
        if task.offset > 0 {
            remote_file.seek(SeekFrom::Start(task.offset))?;
        }

        let mut buffer = vec![0u8; config.chunk_size];
        
        loop {
            match remote_file.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(bytes_read) => {
                    local_file.write_all(&buffer[..bytes_read])?;
                }
                Err(e) => return Err(e.into()),
            }
        }

        local_file.flush()?;
        Ok(())
    }

    // 删除未使用的方法和结构体
}

#[derive(Debug)]
struct DownloadTask {
    remote_path: String,
    local_path: PathBuf,
    offset: u64,         // 断点续传的起始位置
    effective_size: u64,  // 实际需要下载的大小
}