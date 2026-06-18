// src/device.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MediaType {
    HDD,
    SSD,
    NVMe,
    USB,
    Emmc,
    UFS,
    UNKNOWN,
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use MediaType::*;
        let s = match self {
            HDD => "HDD",
            SSD => "SSD",
            NVMe => "NVMe",
            USB => "USB",
            Emmc => "eMMC",
            UFS => "UFS",
            UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub path: String,
    pub size: String,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub tran: Option<String>,
    pub media: MediaType,
    pub source: String,
    pub removable: bool,
}

pub fn list_block_devices() -> Result<Vec<Device>> {
    let mut out = vec![];
    // local host devices
    match list_local_block_devices() {
        Ok(mut v) => out.append(&mut v),
        Err(e) => return Err(e.context("failed to list local block devices")),
    }
    // try Android devices (via adb) when adb exists
    if which::which("adb").is_ok() {
        match list_android_devices() {
            Ok(mut v) => out.append(&mut v),
            Err(e) => log::debug!("adb enumeration failed: {:?}", e),
        }
    }
    Ok(out)
}

#[cfg(unix)]
fn list_local_block_devices() -> Result<Vec<Device>> {
    let out = Command::new("lsblk")
        .arg("-J")
        .arg("-o")
        .arg("NAME,TYPE,SIZE,MODEL,SERIAL,ROTA,TRAN")
        .output()
        .context("failed to spawn lsblk")?;
    if !out.status.success() {
        anyhow::bail!("lsblk failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let txt = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&txt)?;
    let mut devices = vec![];
    if let Some(arr) = json.get("blockdevices").and_then(|v| v.as_array()) {
        for item in arr {
            if item.get("type").and_then(|v| v.as_str()) != Some("disk") {
                continue;
            }
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let path = format!("/dev/{}", name);
            let size = item.get("size").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let model = item.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
            let serial = item.get("serial").and_then(|v| v.as_str()).map(|s| s.to_string());
            let tran = item.get("tran").and_then(|v| v.as_str()).map(|s| s.to_string());
            let rota = item.get("rota").and_then(|v| v.as_str()).map(|s| s.to_string());
            let media = detect_media_unix(&name, &tran, rota.as_deref(), model.as_deref());

            let is_removable = tran.as_deref().map(|t| t.to_lowercase().contains("usb")).unwrap_or(false)
                || name.starts_with("mmcblk");

            devices.push(Device {
                name: name.clone(),
                path,
                size,
                model,
                serial,
                tran,
                media,
                source: "local".to_string(),
                removable: is_removable,
            });
        }
    }
    Ok(devices)
}

#[cfg(windows)]
fn list_local_block_devices() -> Result<Vec<Device>> {
    // wmic fallback
    let out = Command::new("wmic")
        .arg("diskdrive")
        .arg("get")
        .arg("DeviceID,Model,SerialNumber,Size,InterfaceType")
        .arg("/format:csv")
        .output()
        .context("failed to run wmic diskdrive")?;
    if !out.status.success() {
        anyhow::bail!("wmic failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let txt = String::from_utf8_lossy(&out.stdout);
    let mut devices = vec![];
    for line in txt.lines() {
        let s = line.trim();
        if s.is_empty() { continue; }
        if s.to_lowercase().starts_with("node,") { continue; }
        let parts: Vec<&str> = s.splitn(6, ',').collect();
        if parts.len() < 6 { continue; }
        let deviceid = parts[1].trim().to_string(); // \\.\PHYSICALDRIVE0
        let model = parts[2].trim();
        let serial = parts[3].trim();
        let size = parts[4].trim();
        let interface = parts[5].trim().to_string();
        let model_opt = if model.is_empty() { None } else { Some(model.to_string()) };
        let serial_opt = if serial.is_empty() { None } else { Some(serial.to_string()) };
        let mut dev_type = MediaType::UNKNOWN;
        if let Some(m) = &model_opt {
            let ml = m.to_lowercase();
            if ml.contains("nvme") { dev_type = MediaType::NVMe; }
            else if ml.contains("ssd") { dev_type = MediaType::SSD; }
            else if ml.contains("hdd") || ml.contains("disk") { dev_type = MediaType::HDD; }
        }
        if interface.to_lowercase().contains("usb") { dev_type = MediaType::USB; }
        let is_removable = interface.to_lowercase().contains("usb");
        let dev = Device {
            name: deviceid.rsplit('\\').next().unwrap_or(&deviceid).to_string(),
            path: deviceid.clone(),
            size: if size.is_empty() { "unknown".into() } else { size.to_string() },
            model: model_opt,
            serial: serial_opt,
            tran: Some(interface),
            media: dev_type,
            source: "windows".into(),
            removable: is_removable,
        };
        devices.push(dev);
    }
    Ok(devices)
}

fn detect_media_unix(name: &str, tran: &Option<String>, rota: Option<&str>, model: Option<&str>) -> MediaType {
    let name_l = name.to_lowercase();
    let tran_l = tran.clone().unwrap_or_default().to_lowercase();
    let model_l = model.unwrap_or("").to_lowercase();

    if name_l.starts_with("nvme") || tran_l.contains("nvme") || model_l.contains("nvme") {
        return MediaType::NVMe;
    }
    if tran_l.contains("usb") {
        return MediaType::USB;
    }
    if let Some(r) = rota {
        if r == "1" {
            return MediaType::HDD;
        } else if r == "0" {
            if model_l.contains("emmc") || name_l.starts_with("mmcblk") {
                return MediaType::Emmc;
            }
            if model_l.contains("ufs") {
                return MediaType::UFS;
            }
            return MediaType::SSD;
        }
    }
    if model_l.contains("emmc") { return MediaType::Emmc; }
    if model_l.contains("ufs") { return MediaType::UFS; }
    if model_l.contains("ssd") { return MediaType::SSD; }
    if model_l.contains("hdd") || model_l.contains("hard") { return MediaType::HDD; }
    MediaType::UNKNOWN
}

pub fn detect_hpa(dev: &Device) -> Result<Option<String>> {
    // On Unix try hdparm -N; on Windows not implemented
    if cfg!(windows) {
        return Ok(None);
    }
    if which::which("hdparm").is_err() {
        return Ok(None);
    }
    let out = Command::new("hdparm").arg("-N").arg(&dev.path).output()
        .context("failed to run hdparm -N")?;
    if !out.status.success() {
        return Ok(Some(String::from_utf8_lossy(&out.stderr).to_string()));
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).to_string()))
}

pub fn list_android_devices() -> Result<Vec<Device>> {
    if which::which("adb").is_err() {
        anyhow::bail!("adb not found");
    }
    let out = Command::new("adb").arg("devices").arg("-l").output().context("adb devices failed")?;
    if !out.status.success() {
        anyhow::bail!("adb devices failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let txt = String::from_utf8_lossy(&out.stdout);
    let mut devices = vec![];
    for line in txt.lines().skip(1) {
        let s = line.trim();
        if s.is_empty() { continue; }
        if s.contains("no permissions") { continue; }
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() == 0 { continue; }
        let serial = parts[0];
        if serial == "List" { continue; }
        let lsblk_cmd = "lsblk -J -o NAME,TYPE,SIZE,MODEL,SERIAL,ROTA,TRAN";
        let out2 = Command::new("adb").arg("-s").arg(serial).arg("shell").arg(lsblk_cmd).output();
        let out2 = match out2 {
            Ok(o) => o,
            Err(_) => {
                log::debug!("adb lsblk spawn failed for device {}", serial);
                continue;
            }
        };
        if !out2.status.success() {
            log::debug!("adb lsblk failed for {}: {}", serial, String::from_utf8_lossy(&out2.stderr));
            continue;
        }
        let js = String::from_utf8_lossy(&out2.stdout);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&js) {
            if let Some(arr) = json.get("blockdevices").and_then(|v| v.as_array()) {
                for item in arr {
                    if item.get("type").and_then(|v| v.as_str()) != Some("disk") {
                        continue;
                    }
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let blk_path = if name.starts_with("mmcblk") || name.starts_with("sda") || name.starts_with("nvme") {
                        format!("/dev/{}", name)
                    } else {
                        format!("/dev/block/{}", name)
                    };
                    let full_path = format!("adb:{}:{}", serial, blk_path);
                    let size = item.get("size").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let model = item.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let serial_num = item.get("serial").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let tran = item.get("tran").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let rota = item.get("rota").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let media = detect_media_unix(&name, &tran, rota.as_deref(), model.as_deref());
                    let is_removable = name.starts_with("mmcblk") || name.starts_with("sd");
                    devices.push(Device {
                        name: format!("{}@{}", name, serial),
                        path: full_path,
                        size,
                        model,
                        serial: serial_num,
                        tran,
                        media,
                        source: format!("android:{}", serial),
                        removable: is_removable,
                    });
                }
            }
        }
    }
    Ok(devices)
}
