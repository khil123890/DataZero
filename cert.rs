use anyhow::{Context, Result};
use chrono::Utc;
use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::pkey::PKey;
use openssl::sign::Signer;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::device::Device;
use crate::ops::WipeResult;

#[derive(Serialize, Deserialize)]
struct Cert {
    tool: String,
    version: String,
    device: Device,
    wipe: WipeResult,
    operator: Option<String>,
    timestamp: String,
    signature: Option<String>,
}

pub fn generate_certificate<P: AsRef<Path>>(
    out_dir: P,
    device: &Device,
    method: &str,               // we will use this value to ensure the cert records the chosen method
    wipe_result: &WipeResult,
    operator: Option<&str>,
) -> Result<std::path::PathBuf> {
    let key_path = Path::new("datazero_key.pem");
    // create key if missing (P-256)
    if !key_path.exists() {
        println!("Generating ECDSA P-256 private key (datazero_key.pem)...");
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;
        let ec_key = EcKey::generate(&group)?;
        let pem = ec_key.private_key_to_pem()?;
        fs::write(key_path, pem)?;
        // write public
        let pub_pem = ec_key.public_key_to_pem()?;
        fs::write("datazero_key.pem.pub", pub_pem)?;
    }
    let key_pem = fs::read(key_path).context("read key")?;
    let ec_key = EcKey::private_key_from_pem(&key_pem)?;
    let pkey = PKey::from_ec_key(ec_key)?;

    // ensure the wipe method in the certificate matches the provided `method` string:
    let mut wipe_for_cert = wipe_result.clone();
    wipe_for_cert.method = method.to_string();

    // make certificate content
    let cert = Cert {
        tool: "DataZero-Rust".to_string(),
        version: "0.1.0".to_string(),
        device: device.clone(),
        wipe: wipe_for_cert,
        operator: operator.map(|s| s.to_string()),
        timestamp: Utc::now().to_rfc3339(),
        signature: None,
    };

    let payload = serde_json::to_vec_pretty(&cert)?;
    // sign payload
    let mut signer = Signer::new_without_digest(&pkey)?;
    signer.update(&payload)?;
    let signature = signer.sign_to_vec()?;
    let sig_hex = hex::encode(&signature);

    // attach signature and write final JSON
    let mut signed_cert = cert;
    signed_cert.signature = Some(sig_hex);
    let out = serde_json::to_vec_pretty(&signed_cert)?;
    let filename = format!(
        "{}/wipe_cert_{}_{}.json",
        out_dir.as_ref().display(),
        device.name,
        Utc::now().timestamp()
    );
    fs::write(&filename, out)?;
    Ok(Path::new(&filename).to_path_buf())
}
