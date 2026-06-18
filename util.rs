// src/util.rs
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::{Command, Stdio};

/// Read first `mb` MiB of the device and return SHA256 hex string.
/// Supports:
///  - Windows native path: r"\\.\PHYSICALDRIVE1"
///  - Unix native path: "/dev/sdb"
///  - Android path form: "adb:<serial>:/dev/block/mmcblk0"
pub fn sha256_of_device_sample(path: &str, mb: usize) -> Result<String> {
    let bytes_to_read = mb.checked_mul(1024*1024).unwrap_or(1024*1024);

    // Android adb path format: adb:<serial>:/dev/...
    if path.starts_with("adb:") {
        // expected format: adb:<serial>:/dev/...
        let parts: Vec<&str> = path.splitn(3, ':').collect();
        if parts.len() < 3 {
            anyhow::bail!("adb path format must be adb:<serial>:/dev/...");
        }
        let serial = parts[1];
        let devpath = parts[2];

        // Use adb exec-out dd if=<devpath> bs=1M count=<mb>
        // Read stdout and compute hash locally.
        let dd_arg = format!("dd if={} bs=1M count={}", devpath, mb);
        let mut child = Command::new("adb")
            .arg("-s")
            .arg(serial)
            .arg("exec-out")
            .arg(dd_arg)
            .stdout(Stdio::piped())
            .spawn()
            .context("failed to spawn adb exec-out dd")?;

        let mut stdout = child
            .stdout
            .take()
            .context("adb produced no stdout")?;

        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 8 * 1024];
        let mut total_read = 0usize;
        loop {
            let n = stdout.read(&mut buf)?;
            if n == 0 { break; }
            total_read += n;
            hasher.update(&buf[..n]);
            if total_read >= bytes_to_read {
                break;
            }
        }
        // Wait for child to exit; ignore exit code for now (we got data)
        let _ = child.wait();
        let result = hasher.finalize();
        let hex = result.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        return Ok(hex);
    }

    // Native device: open & read
    // On Windows this will be something like "\\.\PHYSICALDRIVE1" and requires admin.
    let mut f = File::open(path)
        .with_context(|| format!("failed to open device path '{}'", path))?;

    // Ensure at start
    let _ = f.seek(SeekFrom::Start(0));

    let mut buf = vec![0u8; bytes_to_read];
    let n = f.read(&mut buf)
        .with_context(|| format!("failed to read {} bytes from {}", bytes_to_read, path))?;
    let mut hasher = Sha256::new();
    hasher.update(&buf[..n]);
    let result = hasher.finalize();
    let hex = result.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    Ok(hex)
}
