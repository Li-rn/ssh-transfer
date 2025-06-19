use anyhow::Result;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub fn calculate_md5<P: AsRef<Path>>(file_path: P) -> Result<String> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = md5::Context::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.consume(&buffer[..bytes_read]);
    }

    let digest = hasher.compute();
    Ok(format!("{:x}", digest))
}

pub fn ensure_parent_dir<P: AsRef<Path>>(file_path: P) -> Result<()> {
    if let Some(parent) = file_path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}