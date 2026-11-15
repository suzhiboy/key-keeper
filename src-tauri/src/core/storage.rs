use super::crypto::MAX_VAULT_SIZE;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use tempfile::NamedTempFile;

pub(super) fn read_encrypted(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| "无法打开保险库文件".to_owned())?;
    let mut bytes = Vec::new();
    file.take(MAX_VAULT_SIZE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取保险库文件".to_owned())?;
    if bytes.len() > MAX_VAULT_SIZE {
        return Err("保险库文件超过大小限制".to_owned());
    }
    Ok(bytes)
}

pub(super) fn write_encrypted(path: &Path, bytes: &[u8], replace: bool) -> Result<(), String> {
    atomic_write(path, bytes, replace).map_err(|_| "无法保存保险库文件".to_owned())
}

fn atomic_write(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut file = NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    // Same-directory persistence replaces the file atomically, including on Windows.
    if replace {
        file.persist(path)?;
    } else {
        file.persist_noclobber(path)?;
    }
    Ok(())
}
