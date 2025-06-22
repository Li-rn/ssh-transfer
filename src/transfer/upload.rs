// 上传功能
use crate::config::Config;
use crate::ssh::SshSession;
use crate::transfer::progress::ProgressTracker;
use crate::utils::error::TransferError;
use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use ssh2::{Sftp, OpenType};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::future::Future;
use std::pin::Pin;

pub struct Uploader {
    session: SshSession,
    config: Arc<Config>,
    remote_home: String,
}

impl Uploader {
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

    pub async fn upload<P: AsRef<Path>>(
        &self,
        local_path: P,
        remote_path_str: &str,
        recursive: bool,
    ) -> Result<()> {
        let local_path = local_path.as_ref();
        let sftp = self.session.client.sftp()?;

        // 确保本地文件存在
        if !local_path.exists() {
            return Err(anyhow::anyhow!("Local file or directory does not exist: {}", local_path.display()));
        }

        // 检查常见路径错误 - 检测shell扩展的本地路径
        if remote_path_str.starts_with("/Users/") {
            return Err(anyhow::anyhow!(
                "错误: 远程路径 '{}' 看起来是本地 macOS 路径，而不是远程路径。\n\
                要上传到远程主目录，请使用引号: '~'\n\
                例如: ./ssh-transfer -H host -u user upload file.txt '~'",
                remote_path_str
            ));
        }

        // 解析远程路径（处理 ~, . 等特殊情况）
        let remote_path = self.resolve_remote_path_str(remote_path_str);
        println!("Resolved remote path: {}", remote_path);
        
        if local_path.is_dir() {
            if recursive {
                let file_name = local_path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Cannot determine directory name from local path"))?;
                let new_remote_path = format!("{}/{}", remote_path.trim_end_matches('/'), file_name.to_string_lossy());
                self.upload_directory(&sftp, local_path, &new_remote_path).await
            } else {
                Err(TransferError::DirectoryNotAllowed.into())
            }
        } else {
            // 确定目标文件路径
            let target_path = self.get_target_file_path(&sftp, &remote_path, local_path)?;
            println!("Target file path: {}", target_path);
            self.upload_file(&sftp, local_path, &target_path).await
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

    fn get_target_file_path(&self, sftp: &Sftp, remote_path: &str, local_file: &Path) -> Result<String> {
        // 检查远程路径是否存在且是目录
        match sftp.stat(Path::new(remote_path)) {
            Ok(stat) => {
                if stat.is_dir() {
                    // 如果是目录，添加文件名
                    let file_name = local_file.file_name()
                        .ok_or_else(|| anyhow::anyhow!("Cannot determine file name"))?
                        .to_string_lossy();
                    Ok(format!("{}/{}", remote_path.trim_end_matches('/'), file_name))
                } else {// 如果是文件，直接使用
                    Ok(remote_path.to_string())
                }
            }
            Err(_) => {
                // 路径不存在，检查它是否有父目录
                if let Some(parent) = Path::new(remote_path).parent() {
                    if !parent.as_os_str().is_empty() {  // 不是根目录
                        // 检查父目录是否存在
                        match sftp.stat(parent) {
                            Ok(stat) if stat.is_dir() => {
                                // 父目录存在，使用原路径
                                Ok(remote_path.to_string())
                            }
                            _ => {
                                // 尝试创建父目录路径（如果不存在）
                                self.ensure_remote_directory(sftp, parent)?;
                                Ok(remote_path.to_string())
                            }
                        }
                    } else {
                        // 直接在根目录创建文件
                        Ok(remote_path.to_string())
                    }
                } else {
                    // 没有父目录（根目录的情况）
                    Ok(remote_path.to_string())
                }
            }
        }
    }
    
    // 确保远程目录存在，尝试创建必要的目录
    fn ensure_remote_directory(&self, sftp: &Sftp, dir_path: &Path) -> Result<()> {
        let path_str = dir_path.to_string_lossy();
        
        // 系统目录列表 - 不尝试创建这些
        let system_dirs = ["/", "/usr", "/var", "/etc", "/bin", "/sbin", "/lib", "/opt", "/Users"];
        if system_dirs.contains(&path_str.as_ref()) {
            return Err(anyhow::anyhow!(
                "Cannot create system directory: {}. Please specify a valid path within your home directory.", 
                path_str
            ));
        }
        
        // 检查目录是否已存在
        match sftp.stat(dir_path) {
            Ok(stat) if stat.is_dir() => return Ok(()),
            Ok(_) => return Err(anyhow::anyhow!("Path exists but is not a directory: {}", dir_path.display())),
            Err(_) => {}
        }
        
        // 需要创建的目录路径列表（从最上层到最底层）
        let mut dirs_to_create = Vec::new();
        let mut current = dir_path;
        
        // 查找第一个存在的父目录
        loop {
            dirs_to_create.push(current.to_path_buf());
            
            if let Some(parent) = current.parent() {
                if parent.as_os_str().is_empty() || parent == Path::new("/") {
                    // 到达根目录
                    break;
                }
                
                match sftp.stat(parent) {
                    Ok(stat) if stat.is_dir() => break, // 找到存在的父目录
                    _ => current = parent,
                }
            } else {
                break;
            }
        }
        
        // 按从最高级到最低级的顺序创建目录
        for dir in dirs_to_create.iter().rev() {
            println!("Creating remote directory: {}", dir.display());
            match sftp.mkdir(dir, 0o755) {
                Ok(_) => {},
                Err(e) => {
                    // 再次检查，可能是由于竞争条件目录已被创建
                    match sftp.stat(dir) {
                        Ok(stat) if stat.is_dir() => {}, // 目录现在存在了
                        _ => return Err(anyhow::anyhow!("Failed to create directory {}: {}", dir.display(), e))
                    }
                }
            }
        }
        
        Ok(())
    }

    // 使用Pin<Box<dyn Future>> 返回类型来处理异步递归
    fn upload_directory<'a>(
        &'a self, 
        sftp: &'a Sftp, 
        local_dir: &'a Path, 
        remote_dir: &'a str
    ) -> Pin<Box<dyn Future<Output = Result<()>> + 'a>> {
        Box::pin(async move {
            // 确保远程目录存在
            self.ensure_remote_directory(sftp, Path::new(remote_dir))?;

            // 收集要上传的文件和子目录
            let mut files_to_upload = Vec::new();
            let mut dirs_to_upload = Vec::new();
            let mut total_size: u64 = 0;

            // 使用std::fs::read_dir收集文件但不立即递归处理子目录
            for entry in std::fs::read_dir(local_dir)? {
                let entry = entry?;
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                let remote_path = format!("{}/{}", remote_dir.trim_end_matches('/'), file_name);

                if path.is_dir() {
                    // 收集子目录，稍后处理
                    dirs_to_upload.push((path.to_path_buf(), remote_path));
                } else if path.is_file() {
                    // 添加文件到上传列表
                    let metadata = std::fs::metadata(&path)?;
                    let size = metadata.len();
                    
                    // 如果启用断点续传，检查远程文件
                    let mut effective_size = size;
                    let mut offset = 0;
                    if self.config.resume {
                        match sftp.stat(Path::new(&remote_path)) {
                            Ok(stat) => {
                                let remote_size = stat.size.unwrap_or(0);
                                if remote_size < size {
                                    // 只上传剩余部分
                                    offset = remote_size;
                                    effective_size = size - remote_size;
                                } else if remote_size == size {
                                    // 文件已完成，跳过
                                    println!("Skipping already uploaded file: {}", path.display());
                                    continue;
                                }
                            }
                            Err(_) => {} // 文件不存在，从头开始上传
                        }
                    }
                    
                    total_size += effective_size;
                    files_to_upload.push((path, remote_path, offset, effective_size));
                }
            }

            // 为当前目录创建单个总进度条
            let progress = Arc::new(ProgressTracker::new(
                total_size, 
                &format!("Uploading from {}", local_dir.display())
            ));

            // 创建上传任务
            let (tx, rx): (Sender<UploadTask>, Receiver<UploadTask>) = bounded(100);

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
                        if let Err(e) = Self::upload_file_worker(&sftp, &task, &config) {
                            eprintln!("Upload error for {}: {}", task.local_path.display(), e);
                        } else {
                            progress.add_bytes(task.effective_size);
                        }
                    }
                });
                handles.push(handle);
            }

            // 发送上传任务
            for (local_path, remote_path, offset, effective_size) in files_to_upload {
                let task = UploadTask {
                    local_path,
                    remote_path,
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
            
            // 当前目录处理完毕后，顺序处理子目录
            // 这样避免同时创建太多进度条
            for (local_subdir, remote_subdir) in dirs_to_upload {
                self.upload_directory(sftp, &local_subdir, &remote_subdir).await?;
            }

            Ok(())
        })
    }

    async fn upload_file(&self, sftp: &Sftp, local_path: &Path, remote_path: &str) -> Result<()> {
        let metadata = std::fs::metadata(local_path)?;
        let file_size = metadata.len();

        println!("Uploading file: {} -> {} ({} bytes)", 
                 local_path.display(), remote_path, file_size);

        // 断点续传逻辑：检查远程文件是否存在
        let mut offset = 0;
        let progress = ProgressTracker::new(file_size, &format!("Uploading {}", local_path.display()));
        
        if self.config.resume {
            // 检查远程文件是否存在
            match sftp.stat(Path::new(remote_path)) {
                Ok(stat) => {
                    let remote_size = stat.size.unwrap_or(0);
                    if remote_size <= file_size {
                        offset = remote_size;
                        println!("Resuming upload from offset: {} bytes", offset);
                        progress.update(offset); // 更新进度条以显示已上传部分
                    } else {
                        println!("Remote file is larger than local file. Starting upload from beginning.");
                    }
                }
                Err(_) => {
                    // 远程文件不存在，从头开始上传
                }
            }
        }

        // 确保远程目录存在
        if let Some(parent) = Path::new(remote_path).parent() {
            if !parent.as_os_str().is_empty() {  // 不是根目录
                self.ensure_remote_directory(sftp, parent)?;
            }
        }

        let mut local_file = File::open(local_path)
            .with_context(|| format!("Failed to open local file: {}", local_path.display()))?;
        
        // 如果断点续传，先定位到偏移位置
        if offset > 0 {
            local_file.seek(SeekFrom::Start(offset))?;
        }
        
        // 创建或打开远程文件
        let mut remote_file = if offset > 0 {
            // 以追加模式打开文件
            sftp.open_mode(
                Path::new(remote_path), 
                ssh2::OpenFlags::WRITE | ssh2::OpenFlags::APPEND, 
                0o644,
                OpenType::File
            )?
        } else {
            // 创建新文件
            sftp.create(Path::new(remote_path))?
        };

        let mut buffer = vec![0u8; self.config.chunk_size];
        let mut total_transferred = offset;

        loop {
            match local_file.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(bytes_read) => {
                    remote_file.write_all(&buffer[..bytes_read])
                        .with_context(|| "Failed to write to remote file")?;
                    total_transferred += bytes_read as u64;
                    progress.update(total_transferred);
                }
                Err(e) => {
                    progress.finish_with_error(&e.to_string());
                    return Err(e.into());
                }
            }
        }

        // 确保数据写入完成
        remote_file.fsync().ok(); // 忽略fsync错误，某些服务器可能不支持

        progress.finish();
        println!("✅ Upload completed: {}", remote_path);
        Ok(())
    }

    fn upload_file_worker(sftp: &Sftp, task: &UploadTask, config: &Config) -> Result<()> {
        let mut local_file = File::open(&task.local_path)?;
        
        // 设置偏移量
        if task.offset > 0 {
            local_file.seek(SeekFrom::Start(task.offset))?;
        }
        
        // 创建或打开远程文件
        let mut remote_file = if task.offset > 0 {
            sftp.open_mode(
                Path::new(&task.remote_path),
                ssh2::OpenFlags::WRITE | ssh2::OpenFlags::APPEND,
                0o644,
                OpenType::File
            )?
        } else {
            // 确保父目录存在
            if let Some(parent) = Path::new(&task.remote_path).parent() {
                if !parent.as_os_str().is_empty() {
                    match sftp.stat(parent) {
                        Ok(stat) if stat.is_dir() => {}, // 目录已存在
                        _ => {
                            // 尝试创建目录
                            sftp.mkdir(parent, 0o755).ok();
                        }
                    }
                }
            }
            sftp.create(Path::new(&task.remote_path))?
        };
    
        // 对于大文件使用更大的缓冲区
        let buffer_size = if task.effective_size > 10 * 1024 * 1024 {
            // 大文件使用8MB缓冲区
            8 * 1024 * 1024
        } else {
            config.chunk_size
        };
        
        let mut buffer = vec![0u8; buffer_size];
        
        // 添加进度反馈
        let mut bytes_uploaded = 0;
        let update_frequency = if task.effective_size > 10 * 1024 * 1024 {
            // 大文件每1MB反馈一次
            1024 * 1024
        } else {
            // 小文件每128KB反馈一次
            128 * 1024
        };
        
        loop {
            match local_file.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(bytes_read) => {
                    remote_file.write_all(&buffer[..bytes_read])?;
                    bytes_uploaded += bytes_read as u64;
                    
                    // 减少进度更新频率
                    if bytes_uploaded >= update_frequency {
                        // 我们不在这里更新进度，而是返回后一次性更新
                        bytes_uploaded = 0;
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
    
        // 确保数据写入完成
        remote_file.fsync().ok();
        Ok(())
    }
}

#[derive(Debug)]
struct UploadTask {
    local_path: PathBuf,
    remote_path: String,
    offset: u64,         // 断点续传的起始位置
    effective_size: u64,  // 实际需要上传的大小
}