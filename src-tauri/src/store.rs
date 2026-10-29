//! Stable identity and paired phones.
//!
//! The certificate is reused so its fingerprint, which the phone pins, does not change.
//! The private key stays in Application Support and is written with mode 0600.

use crate::protocol::{self, b64, fingerprint};
use anyhow::{Context, Result};
use rcgen::{CertificateParams, DnType, KeyPair};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use time::Duration;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct PhoneRecord {
    pub real_id: String,
    pub name: String,
    pub our_key: String,
    #[serde(default)]
    pub phone_key: Option<String>,
    #[serde(default)]
    pub user_disconnected: bool,
}

#[derive(Clone)]
pub struct Identity {
    pub real_id: String,
    pub real_bytes: Vec<u8>,
    pub sn: String,
    pub device_name: String,
    pub cert_pem: String,
    pub key_pem: String,
    pub fp_b64: String,
    pub fp_hex: String,
}

#[derive(Serialize, Deserialize)]
struct Disk {
    real_device_id: String,
    sn: String,
    device_name: String,
    cert_pem: String,
    key_pem: String,
    phones: Vec<PhoneRecord>,
}

pub fn load() -> Result<(Identity, Vec<PhoneRecord>)> {
    let path = store_path()?;
    if !path.exists() {
        let created = create_new(&computer_name())?;
        save(&created.0, &created.1)?;
        return Ok(created);
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    match decode(&text) {
        Ok(parsed) => Ok(parsed),
        Err(err) => {
            let backup = path.with_extension("json.bak");
            let _ = fs::rename(&path, &backup);
            eprintln!("identity file was unreadable ({err}); moved it aside and created a new one");
            let created = create_new(&computer_name())?;
            save(&created.0, &created.1)?;
            Ok(created)
        }
    }
}

pub fn save(identity: &Identity, phones: &[PhoneRecord]) -> Result<()> {
    let path = store_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let disk = Disk {
        real_device_id: identity.real_id.clone(),
        sn: identity.sn.clone(),
        device_name: identity.device_name.clone(),
        cert_pem: identity.cert_pem.clone(),
        key_pem: identity.key_pem.clone(),
        phones: phones.to_vec(),
    };
    let text = serde_json::to_string_pretty(&disk)?;
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path)?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

fn decode(text: &str) -> Result<(Identity, Vec<PhoneRecord>)> {
    let disk: Disk = serde_json::from_str(text)?;
    let real_bytes = protocol::b64_decode(&disk.real_device_id).context("real device id is not base64")?;
    let (fp_b64, fp_hex) = fingerprint_of_pem(&disk.cert_pem)?;
    let identity = Identity {
        real_id: disk.real_device_id,
        real_bytes,
        sn: disk.sn,
        device_name: disk.device_name,
        cert_pem: disk.cert_pem,
        key_pem: disk.key_pem,
        fp_b64,
        fp_hex,
    };
    Ok((identity, disk.phones))
}

pub fn create_new(device_name: &str) -> Result<(Identity, Vec<PhoneRecord>)> {
    let real_bytes = protocol::random_bytes::<16>().to_vec();
    let real_id = b64(&real_bytes);
    let sn = Uuid::new_v5(&Uuid::NAMESPACE_OID, &real_bytes).to_string();
    // The phone pins the certificate fingerprint and does not require RSA.
    // Ring can generate P-256, which rustls then presents on the listener.
    let key = KeyPair::generate()?;
    let mut params = CertificateParams::new(vec!["SmartConnect".to_string()])?;
    params.distinguished_name.push(DnType::CommonName, "Smart Connect");
    params.not_before = time::OffsetDateTime::now_utc() - Duration::days(1);
    params.not_after = time::OffsetDateTime::now_utc() + Duration::days(365 * 20);
    let cert = params.self_signed(&key)?;
    let cert_pem = cert.pem();
    let key_pem = key.serialize_pem();
    let (fp_b64, fp_hex) = fingerprint(cert.der());
    let identity = Identity {
        real_id,
        real_bytes,
        sn,
        device_name: device_name.to_string(),
        cert_pem,
        key_pem,
        fp_b64,
        fp_hex,
    };
    Ok((identity, Vec::new()))
}

fn fingerprint_of_pem(cert_pem: &str) -> Result<(String, String)> {
    let mut reader = std::io::Cursor::new(cert_pem.as_bytes());
    let der = rustls_pemfile::certs(&mut reader)
        .next()
        .context("certificate pem is empty")??;
    let digest = Sha256::digest(der.as_ref());
    Ok((b64(&digest), hex::encode(digest)))
}

pub fn store_path() -> Result<PathBuf> {
    let base = dirs::data_dir().context("no application support directory")?;
    Ok(base.join("MotoSmartConnect").join("identity.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_stable_certificate() {
        let (identity, phones) = create_new("Test Mac").expect("certificate");
        assert!(phones.is_empty());
        assert_eq!(identity.fp_hex.len(), 64);
        assert!(identity.cert_pem.contains("BEGIN CERTIFICATE"));
        crate::net::install_crypto();
        let (again, _) = create_new("Test Mac").expect("certificate after rustls init");
        assert_ne!(identity.fp_hex, again.fp_hex);
    }

    #[test]
    fn saves_identity_to_application_support() {
        let loaded = load().unwrap_or_else(|err| panic!("{err:#}"));
        assert!(!loaded.0.real_id.is_empty());
    }
}

fn computer_name() -> String {
    if let Ok(output) = std::process::Command::new("scutil").args(["--get", "ComputerName"]).output() {
        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Mac".to_string())
}
