// 进度显示
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::sync::Mutex;

#[derive(Clone)]
pub struct ProgressTracker {
    progress_bar: ProgressBar,
    transferred_bytes: Arc<AtomicU64>,
    start_time: Arc<Instant>,
    last_update_time: Arc<Mutex<Instant>>,
    last_bytes: Arc<AtomicU64>,
}

impl ProgressTracker {
    pub fn new(total_size: u64, description: &str) -> Self {
        let progress_bar = ProgressBar::new(total_size);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        progress_bar.set_message(description.to_string());


        Self {
            progress_bar,
            transferred_bytes: Arc::new(AtomicU64::new(0)),
            start_time: Arc::new(Instant::now()),
            last_update_time: Arc::new(Mutex::new(Instant::now())),
            last_bytes: Arc::new(AtomicU64::new(0)),
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
        
        // 更新速度显示
        self.update_speed(new_total);
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
        self.progress_bar.finish_with_message(format!("Transfer completed (avg speed: {})", speed_str));
    }

    pub fn finish_with_error(&self, error: &str) {
        self.progress_bar.finish_with_message(format!("Transfer failed: {}", error));
    }
    
    // 私有方法：更新速度显示
    fn update_speed(&self, current_bytes: u64) {
        let now = Instant::now();
        
        // 获取上次更新的时间和字节数
        let mut last_time = self.last_update_time.lock().unwrap();
        let elapsed = now.duration_since(*last_time);
        
        // 每200ms更新一次速度，避免太频繁刷新
        if elapsed >= Duration::from_millis(200) {
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