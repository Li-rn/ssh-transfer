// 进度显示
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use std::path::Path;

// 使用全局MultiProgress实例来管理所有进度条
static MULTI_PROGRESS: Lazy<MultiProgress> = Lazy::new(|| MultiProgress::new());

#[derive(Clone)]
pub struct ProgressTracker {
    progress_bar: ProgressBar,
    transferred_bytes: Arc<AtomicU64>,
    start_time: Arc<Instant>,
    last_update_time: Arc<Mutex<Instant>>,
    last_bytes: Arc<AtomicU64>,
    current_file: Arc<Mutex<String>>,  // 当前文件名
}

impl ProgressTracker {
    pub fn new(total_size: u64, description: &str) -> Self {
        // 使用全局MultiProgress创建进度条
        let progress_bar = MULTI_PROGRESS.add(ProgressBar::new(total_size));
        
        // 设置样式 - 按照要求的格式，文件名在第一行，进度条在第二行
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}]{prefix}\n[{bar:40.cyan/blue}] {bytes}/{total_bytes} ({msg})")
                .unwrap()
                .progress_chars("#>-"),
        );
        
        // 提取并设置简化的前缀（只保留最后一部分名称）
        let simplified_prefix = extract_last_part(description);
        progress_bar.set_prefix(simplified_prefix);
        progress_bar.set_message("".to_string());

        Self {
            progress_bar,
            transferred_bytes: Arc::new(AtomicU64::new(0)),
            start_time: Arc::new(Instant::now()),
            last_update_time: Arc::new(Mutex::new(Instant::now())),
            last_bytes: Arc::new(AtomicU64::new(0)),
            current_file: Arc::new(Mutex::new(String::new())),
        }
    }

    // 设置当前正在传输的文件名
    pub fn set_current_file(&self, filename: &str) {
        if let Ok(mut current) = self.current_file.lock() {
            *current = filename.to_string();
            
            // 提取并设置简化的文件名（只保留最后一部分）
            let simplified_name = extract_last_part(filename);
            self.progress_bar.set_prefix(simplified_name);
        }
    }

    pub fn update(&self, bytes_transferred: u64) {
        self.transferred_bytes.store(bytes_transferred, Ordering::Relaxed);
        self.progress_bar.set_position(bytes_transferred);
        
        // 更新速度显示
        self.update_speed(bytes_transferred);
    }

    pub fn add_bytes(&self, bytes: u64) {
        let current = self.transferred_bytes.fetch_add(bytes, Ordering::Relaxed);
        let new_total = current + bytes;
        self.progress_bar.set_position(new_total);
        
        // 减少速度更新频率，只在添加较大数据量时更新
        if bytes >= 1024 * 64 {  // 每64KB更新一次
            self.update_speed(new_total);
        }
    }

    pub fn finish(&self) {
        // 计算平均速度
        let elapsed = self.start_time.elapsed();
        let total = self.transferred_bytes.load(Ordering::Relaxed);
        
        let avg_speed = if elapsed.as_secs() > 0 {
            total / elapsed.as_secs()
        } else if total > 0 {
            total
        } else {
            0
        };
        
        let speed_str = format_speed(avg_speed);
        
        // 完成时显示传输完成信息
        self.progress_bar.finish_with_message(format!("Transfer completed (avg speed: {})", speed_str));
    }

    pub fn finish_with_error(&self, error: &str) {
        self.progress_bar.finish_with_message(format!("Transfer failed: {}", error));
    }
    
    // 私有方法：更新速度显示
    fn update_speed(&self, current_bytes: u64) {
        let now = Instant::now();
        
        // 获取上次更新的时间和字节数
        if let Ok(mut last_time) = self.last_update_time.try_lock() {
            let elapsed = now.duration_since(*last_time);
            
            // 每500ms更新一次速度，避免太频繁刷新
            if elapsed >= Duration::from_millis(500) {
                let last_bytes = self.last_bytes.load(Ordering::Relaxed);
                let bytes_diff = current_bytes.saturating_sub(last_bytes);
                
                // 计算传输速度（字节/秒）
                let speed = if elapsed.as_secs_f64() > 0.0 {
                    (bytes_diff as f64 / elapsed.as_secs_f64()) as u64
                } else {
                    0
                };
                
                // 格式化并显示速度
                let speed_str = format_speed(speed);
                self.progress_bar.set_message(speed_str);
                
                // 更新记录的时间和字节数
                *last_time = now;
                self.last_bytes.store(current_bytes, Ordering::Relaxed);
            }
        }
    }
}

// 从路径或描述中提取最后一部分作为前缀
fn extract_last_part(path_or_description: &str) -> String {
    // 首先处理常见的前缀模式
    if path_or_description.starts_with("Uploading from ") {
        let path = &path_or_description["Uploading from ".len()..];
        return extract_last_part_from_path(path);
    } else if path_or_description.starts_with("Downloading from ") {
        let path = &path_or_description["Downloading from ".len()..];
        return extract_last_part_from_path(path);
    }
    
    // 否则直接当作路径处理
    extract_last_part_from_path(path_or_description)
}

// 从路径中提取最后一部分
fn extract_last_part_from_path(path: &str) -> String {
    let path_obj = Path::new(path);
    
    // 尝试获取文件名或目录名
    if let Some(file_name) = path_obj.file_name() {
        return file_name.to_string_lossy().to_string();
    }
    
    // 如果没有文件名，尝试从路径字符串中提取最后一部分
    if let Some(last_slash_pos) = path.rfind('/') {
        if last_slash_pos < path.len() - 1 {
            return path[(last_slash_pos + 1)..].to_string();
        }
    } else if let Some(last_slash_pos) = path.rfind('\\') {
        if last_slash_pos < path.len() - 1 {
            return path[(last_slash_pos + 1)..].to_string();
        }
    }
    
    // 如果无法提取，则返回原始字符串
    path.to_string()
}

// 格式化速度显示
fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec < 1024 {
        format!("{} B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        format!("{:.2} KB/s", bytes_per_sec as f64 / 1024.0)
    } else if bytes_per_sec < 1024 * 1024 * 1024 {
        format!("{:.2} MB/s", bytes_per_sec as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", bytes_per_sec as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}