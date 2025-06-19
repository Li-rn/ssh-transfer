// 断点续传
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
pub struct ResumeInfo {
    pub file_path: String,
    pub total_size: u64,
    pub transferred_size: u64,
    pub chunks: HashMap<usize, ChunkInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChunkInfo {
    pub start: u64,
    pub end: u64,
    pub completed: bool,
    pub checksum: Option<String>,
}

impl ResumeInfo {
    pub fn new(file_path: String, total_size: u64) -> Self {
        Self {
            file_path,
            total_size,
            transferred_size: 0,
            chunks: HashMap::new(),
        }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize resume info")?;
        fs::write(path, json)
            .context("Failed to write resume file")?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)
            .context("Failed to read resume file")?;
        let resume_info: ResumeInfo = serde_json::from_str(&content)
            .context("Failed to parse resume file")?;
        Ok(resume_info)
    }

    pub fn get_incomplete_chunks(&self) -> Vec<(usize, &ChunkInfo)> {
        self.chunks
            .iter()
            .filter(|(_, chunk)| !chunk.completed)
            .map(|(id, chunk)| (*id, chunk))
            .collect()
    }

    pub fn mark_chunk_completed(&mut self, chunk_id: usize, checksum: Option<String>) {
        if let Some(chunk) = self.chunks.get_mut(&chunk_id) {
            chunk.completed = true;
            chunk.checksum = checksum;
            self.transferred_size += chunk.end - chunk.start + 1;
        }
    }

    pub fn add_chunk(&mut self, chunk_id: usize, start: u64, end: u64) {
        self.chunks.insert(chunk_id, ChunkInfo {
            start,
            end,
            completed: false,
            checksum: None,
        });
    }

    pub fn resume_file_path<P: AsRef<Path>>(file_path: P) -> std::path::PathBuf {
        let path = file_path.as_ref();
        let mut resume_path = path.to_path_buf();
        resume_path.set_extension(format!("{}.resume", 
            path.extension().and_then(|s| s.to_str()).unwrap_or("tmp")));
        resume_path
    }
}