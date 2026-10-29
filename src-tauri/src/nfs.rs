//! Read-only NFSv3 client for the phone's folders.
//!
//! The phone serves its storage after a service 168 cmd 1001 handshake. The
//! connection is mutual TLS: we present the identity certificate named in that
//! handshake and pin the certificate fingerprint the phone sends back. The
//! procedures used are MOUNT MNT, LOOKUP, READDIRPLUS and READ (RFC 1813).

use anyhow::{bail, Context, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

const MOUNT: u32 = 100005;
const NFS: u32 = 100003;
const MAX_READ: usize = 128 * 1024 * 1024;
const CHUNK: u32 = 128 * 1024;

#[derive(Clone, Debug)]
pub struct PhoneShare {
    pub nfs_port: u16,
    /// Portmap port from the reply. Listing uses `nfs_port`, which also accepts MOUNT.
    #[allow(dead_code)]
    pub mount_port: u16,
    pub export_path: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneEntry {
    pub name: String,
    pub directory: bool,
    pub size: u64,
}

pub fn clean_path(path: &str) -> Result<Vec<String>> {
    if path.len() > 400 {
        bail!("That folder name is too long.");
    }
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains('\0') || part.contains('\\') {
            bail!("That folder is not on the phone.");
        }
        if part.len() > 180 {
            bail!("That folder name is too long.");
        }
        parts.push(part.to_string());
    }
    if parts.len() > 16 {
        bail!("That folder is nested too deeply.");
    }
    Ok(parts)
}

pub async fn list_dir(ip: IpAddr, share: &PhoneShare, cert_pem: &str, key_pem: &str, parts: &[String]) -> Result<Vec<PhoneEntry>> {
    let mut rpc = connect(ip, share, cert_pem, key_pem).await?;
    let root = mount_root(&mut rpc, share).await?;
    let dir = walk(&mut rpc, &root, parts).await?;
    read_directory(&mut rpc, &dir).await
}

pub async fn read_file(ip: IpAddr, share: &PhoneShare, cert_pem: &str, key_pem: &str, parts: &[String]) -> Result<Vec<u8>> {
    if parts.is_empty() {
        bail!("Choose a file.");
    }
    let mut rpc = connect(ip, share, cert_pem, key_pem).await?;
    let root = mount_root(&mut rpc, share).await?;
    let parent = walk(&mut rpc, &root, &parts[..parts.len() - 1]).await?;
    let (handle, kind, size) = lookup(&mut rpc, &parent, parts.last().unwrap()).await?;
    if kind == 2 {
        bail!("That is a folder.");
    }
    if size > MAX_READ as u64 {
        bail!("That file is too large to copy.");
    }
    let mut out = Vec::new();
    let mut offset = 0u64;
    loop {
        let (chunk, eof) = read_at(&mut rpc, &handle, offset, CHUNK).await?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len() as u64;
        out.extend_from_slice(&chunk);
        if out.len() > MAX_READ || eof {
            break;
        }
    }
    if out.len() > MAX_READ {
        bail!("That file is too large to copy.");
    }
    Ok(out)
}

struct Rpc {
    stream: tokio_rustls::client::TlsStream<TcpStream>,
    xid: u32,
}

async fn connect(ip: IpAddr, share: &PhoneShare, cert_pem: &str, key_pem: &str) -> Result<Rpc> {
    let pin = fingerprint_bytes(&share.fingerprint).context("the phone's folder certificate was not a fingerprint")?;
    let config = client_config(pin, cert_pem, key_pem)?;
    let connector = TlsConnector::from(Arc::new(config));
    let tcp = tokio::time::timeout(Duration::from_secs(8), TcpStream::connect((ip, share.nfs_port)))
        .await
        .context("the phone's folders did not answer")?
        .context("the phone's folders did not answer")?;
    tcp.set_nodelay(true).ok();
    let name = ServerName::try_from("phone.local").context("tls name")?;
    let stream = tokio::time::timeout(Duration::from_secs(8), connector.connect(name, tcp))
        .await
        .context("the phone's folder certificate check timed out")?
        .context("the phone rejected the folder connection")?;
    Ok(Rpc { stream, xid: 1 })
}

async fn mount_root(rpc: &mut Rpc, share: &PhoneShare) -> Result<Vec<u8>> {
    let mut args = XdrWriter::default();
    args.string(&share.export_path);
    let body = rpc.call(MOUNT, 3, 1, &args.buf).await?;
    let mut reader = XdrReader::new(&body);
    let status = reader.u32().context("mount reply was short")?;
    if status != 0 {
        bail!("The phone refused its folder ({status}).");
    }
    let handle = reader.opaque().context("the phone did not name its folder")?;
    if handle.is_empty() || handle.len() > 64 {
        bail!("The phone sent an unusable folder handle.");
    }
    Ok(handle)
}

async fn walk(rpc: &mut Rpc, root: &[u8], parts: &[String]) -> Result<Vec<u8>> {
    let mut handle = root.to_vec();
    for part in parts {
        let (next, kind, _) = lookup(rpc, &handle, part).await?;
        if kind != 2 {
            bail!("That is not a folder.");
        }
        handle = next;
    }
    Ok(handle)
}

async fn lookup(rpc: &mut Rpc, dir: &[u8], name: &str) -> Result<(Vec<u8>, u32, u64)> {
    let mut args = XdrWriter::default();
    args.opaque_bytes(dir);
    args.string(name);
    let body = rpc.call(NFS, 3, 3, &args.buf).await?;
    let mut reader = XdrReader::new(&body);
    let status = reader.u32().context("lookup reply was short")?;
    if status != 0 {
        bail!("The phone has no folder by that name.");
    }
    let (kind, size) = reader.attr().context("lookup attributes were short")?;
    let _dir_attr = reader.attr().context("lookup directory attributes were short")?;
    let handle = reader.opaque().context("lookup handle was short")?;
    if handle.is_empty() || handle.len() > 64 {
        bail!("The phone sent an unusable file handle.");
    }
    Ok((handle, kind, size))
}

async fn read_directory(rpc: &mut Rpc, dir: &[u8]) -> Result<Vec<PhoneEntry>> {
    let mut cookie = 0u64;
    let mut verifier = [0u8; 8];
    let mut entries = Vec::new();
    for _ in 0..32 {
        let mut args = XdrWriter::default();
        args.opaque_bytes(dir);
        args.u64(cookie);
        args.bytes(&verifier);
        args.u32(8192);
        args.u32(65536);
        let body = rpc.call(NFS, 3, 17, &args.buf).await?;
        let mut reader = XdrReader::new(&body);
        let status = reader.u32().context("the folder list was short")?;
        if status != 0 {
            bail!("The phone could not list that folder.");
        }
        let _dir_attr = reader.attr().context("the folder list was short")?;
        verifier = reader.fixed::<8>().context("the folder list was short")?;
        let mut page = 0u32;
        loop {
            let more = reader.u32().context("the folder list was cut off")?;
            if more == 0 {
                break;
            }
            let _file_id = reader.u64().context("a folder entry was short")?;
            let name = reader.string().context("a folder entry had no name")?;
            cookie = reader.u64().context("a folder entry was short")?;
            let (kind, size) = reader.attr().context("a folder entry was short")?;
            let _handle = reader.post_handle().context("a folder entry was short")?;
            if name != "." && name != ".." && !name.contains('/') && !name.contains('\0') {
                entries.push(PhoneEntry { name, directory: kind == 2, size });
            }
            page += 1;
            if page > 5000 || entries.len() > 2000 {
                break;
            }
        }
        let eof = reader.u32().unwrap_or(1) != 0;
        if eof || page == 0 || entries.len() > 2000 {
            break;
        }
    }
    entries.sort_by(|left, right| {
        right.directory.cmp(&left.directory).then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(entries)
}

async fn read_at(rpc: &mut Rpc, handle: &[u8], offset: u64, count: u32) -> Result<(Vec<u8>, bool)> {
    let mut args = XdrWriter::default();
    args.opaque_bytes(handle);
    args.u64(offset);
    args.u32(count);
    let body = rpc.call(NFS, 3, 6, &args.buf).await?;
    let mut reader = XdrReader::new(&body);
    let status = reader.u32().context("the file read was short")?;
    if status != 0 {
        bail!("The phone could not read that file.");
    }
    let _attr = reader.attr().context("the file read was short")?;
    let _count = reader.u32().context("the file read was short")?;
    let eof = reader.u32().context("the file read was short")? != 0;
    let data = reader.opaque().context("the file read was short")?;
    Ok((data, eof))
}

impl Rpc {
    async fn call(&mut self, program: u32, version: u32, procedure: u32, args: &[u8]) -> Result<Vec<u8>> {
        match self.call_auth(program, version, procedure, args, true).await {
            Ok(body) => Ok(body),
            Err(err) if err.to_string() == "denied" => self.call_auth(program, version, procedure, args, false).await,
            Err(err) => Err(err),
        }
    }

    async fn call_auth(&mut self, program: u32, version: u32, procedure: u32, args: &[u8], unix: bool) -> Result<Vec<u8>> {
        self.xid = self.xid.wrapping_add(1);
        let mut call = XdrWriter::default();
        call.u32(self.xid);
        call.u32(0);
        call.u32(2);
        call.u32(program);
        call.u32(version);
        call.u32(procedure);
        if unix {
            call.auth_unix();
        } else {
            call.auth_none();
        }
        call.auth_none();
        call.bytes(args);
        self.write_record(&call.buf).await?;
        let reply = self.read_record().await?;
        let mut reader = XdrReader::new(&reply);
        let xid = reader.u32().context("nfs reply was short")?;
        if xid != self.xid {
            bail!("The phone's folder reply did not match.");
        }
        let kind = reader.u32().context("nfs reply was short")?;
        if kind != 1 {
            bail!("The phone's folder reply was not a reply.");
        }
        let status = reader.u32().context("nfs reply was short")?;
        if status != 0 {
            bail!("denied");
        }
        let _flavor = reader.u32().context("nfs reply was short")?;
        let _verf = reader.opaque().context("nfs reply was short")?;
        let accept = reader.u32().context("nfs reply was short")?;
        if accept != 0 {
            bail!("The phone could not run that folder request ({accept}).");
        }
        Ok(reader.rest().to_vec())
    }

    async fn write_record(&mut self, body: &[u8]) -> Result<()> {
        if body.len() > 8 * 1024 * 1024 {
            bail!("The folder request is too large.");
        }
        let header = ((body.len() as u32) | 0x8000_0000).to_be_bytes();
        self.stream.write_all(&header).await.context("the folder connection closed")?;
        self.stream.write_all(body).await.context("the folder connection closed")?;
        self.stream.flush().await.ok();
        Ok(())
    }

    async fn read_record(&mut self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        loop {
            let mut header = [0u8; 4];
            tokio::time::timeout(Duration::from_secs(12), self.stream.read_exact(&mut header))
                .await
                .context("the phone stopped answering about folders")?
                .context("the phone stopped answering about folders")?;
            let word = u32::from_be_bytes(header);
            let last = word & 0x8000_0000 != 0;
            let len = (word & 0x7fff_ffff) as usize;
            if len > 8 * 1024 * 1024 || out.len() + len > 8 * 1024 * 1024 {
                bail!("The phone's folder reply was too large.");
            }
            let start = out.len();
            out.resize(start + len, 0);
            tokio::time::timeout(Duration::from_secs(12), self.stream.read_exact(&mut out[start..]))
                .await
                .context("the phone stopped answering about folders")?
                .context("the phone stopped answering about folders")?;
            if last {
                return Ok(out);
            }
        }
    }
}

#[derive(Default)]
struct XdrWriter {
    buf: Vec<u8>,
}

impl XdrWriter {
    fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    fn opaque_bytes(&mut self, bytes: &[u8]) {
        self.u32(bytes.len() as u32);
        self.buf.extend_from_slice(bytes);
        let pad = (4 - (bytes.len() % 4)) % 4;
        self.buf.extend(std::iter::repeat(0).take(pad));
    }

    fn string(&mut self, text: &str) {
        self.opaque_bytes(text.as_bytes());
    }

    fn auth_none(&mut self) {
        self.u32(0);
        self.u32(0);
    }

    fn auth_unix(&mut self) {
        let mut body = XdrWriter::default();
        body.u32(0);
        body.string("mac");
        body.u32(0);
        body.u32(0);
        body.u32(0);
        self.u32(1);
        self.opaque_bytes(&body.buf);
    }
}

struct XdrReader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> XdrReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn rest(&self) -> &'a [u8] {
        &self.data[self.at..]
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        if self.at + len > self.data.len() {
            return None;
        }
        let slice = &self.data[self.at..self.at + len];
        self.at += len;
        Some(slice)
    }

    fn u32(&mut self) -> Option<u32> {
        let bytes = self.take(4)?;
        Some(u32::from_be_bytes(bytes.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        let bytes = self.take(8)?;
        Some(u64::from_be_bytes(bytes.try_into().ok()?))
    }

    fn fixed<const N: usize>(&mut self) -> Option<[u8; N]> {
        let bytes = self.take(N)?;
        bytes.try_into().ok()
    }

    fn opaque(&mut self) -> Option<Vec<u8>> {
        let len = self.u32()? as usize;
        if len > 1024 * 1024 {
            return None;
        }
        let bytes = self.take(len)?.to_vec();
        let pad = (4 - (len % 4)) % 4;
        self.take(pad)?;
        Some(bytes)
    }

    fn string(&mut self) -> Option<String> {
        let bytes = self.opaque()?;
        String::from_utf8(bytes).ok()
    }

    /// post_op_attr. Missing attributes come back as kind 0 and size 0.
    fn attr(&mut self) -> Option<(u32, u64)> {
        let follows = self.u32()?;
        if follows == 0 {
            return Some((0, 0));
        }
        let kind = self.u32()?;
        self.take(16)?; // mode, nlink, uid, gid
        let size = self.u64()?;
        self.take(8 + 8 + 8 + 8)?; // used, rdev, fsid, fileid
        self.take(24)?; // atime, mtime, ctime
        Some((kind, size))
    }

    fn post_handle(&mut self) -> Option<()> {
        let follows = self.u32()?;
        if follows == 0 {
            return Some(());
        }
        self.opaque()?;
        Some(())
    }
}

fn fingerprint_bytes(text: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(text.trim()).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn client_config(fingerprint: [u8; 32], cert_pem: &str, key_pem: &str) -> Result<ClientConfig> {
    let mut cert_reader = std::io::Cursor::new(cert_pem.as_bytes());
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader).collect::<Result<Vec<_>, _>>()?;
    let mut key_reader = std::io::Cursor::new(key_pem.as_bytes());
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)?.context("identity has no private key")?;
    ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinCert { fingerprint }))
        .with_client_auth_cert(certs, key)
        .context("could not present the folder certificate")
}

#[derive(Debug)]
struct PinCert {
    fingerprint: [u8; 32],
}

impl ServerCertVerifier for PinCert {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let digest = Sha256::digest(end_entity.as_ref());
        if digest.as_slice() == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("certificate fingerprint does not match".into()))
        }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = rustls::crypto::ring::default_provider().signature_verification_algorithms;
        rustls::crypto::verify_tls12_signature(message, cert, dss, &algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = rustls::crypto::ring::default_provider().signature_verification_algorithms;
        rustls::crypto::verify_tls13_signature(message, cert, dss, &algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_rejects_parent_segments() {
        assert!(clean_path("").unwrap().is_empty());
        assert_eq!(clean_path("DCIM/Camera").unwrap(), vec!["DCIM".to_string(), "Camera".to_string()]);
        assert!(clean_path("../DCIM").is_err());
        assert!(clean_path("DCIM/../secret").is_err());
    }

    #[test]
    fn directory_page_skips_dot_entries() {
        let mut page = XdrWriter::default();
        page.u32(0); // status
        page.u32(0); // no dir attributes
        page.bytes(&[1, 2, 3, 4, 5, 6, 7, 8]); // cookie verifier
        page.u32(1); // entry follows
        page.u64(1);
        page.string(".");
        page.u64(1);
        page.u32(0); // no attributes
        page.u32(0); // no handle
        page.u32(1);
        page.u64(2);
        page.string("Photos");
        page.u64(2);
        attr_dir(&mut page);
        page.u32(0);
        page.u32(0); // end of entries
        page.u32(1); // eof
        let mut reader = XdrReader::new(&page.buf);
        assert_eq!(reader.u32(), Some(0));
        assert_eq!(reader.attr(), Some((0, 0)));
        assert_eq!(reader.fixed::<8>().unwrap(), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(reader.u32(), Some(1));
        let _ = reader.u64();
        assert_eq!(reader.string().as_deref(), Some("."));
        let _ = reader.u64();
        assert_eq!(reader.attr(), Some((0, 0)));
        reader.post_handle().unwrap();
        assert_eq!(reader.u32(), Some(1));
        let _ = reader.u64();
        assert_eq!(reader.string().as_deref(), Some("Photos"));
        let _ = reader.u64();
        assert_eq!(reader.attr(), Some((2, 0)));
    }

    fn attr_dir(page: &mut XdrWriter) {
        page.u32(1);
        page.u32(2); // directory
        page.bytes(&[0u8; 16]);
        page.u64(0); // size
        page.bytes(&[0u8; 32]);
        page.bytes(&[0u8; 24]);
    }

    #[test]
    fn auth_unix_length_matches_its_body() {
        let mut writer = XdrWriter::default();
        writer.auth_unix();
        let mut reader = XdrReader::new(&writer.buf);
        assert_eq!(reader.u32(), Some(1));
        let body = reader.opaque().unwrap();
        assert_eq!(body.len(), reader_at_end_len(&body));
    }

    fn reader_at_end_len(body: &[u8]) -> usize {
        let mut reader = XdrReader::new(body);
        let _stamp = reader.u32().unwrap();
        let _name = reader.string().unwrap();
        let _uid = reader.u32().unwrap();
        let _gid = reader.u32().unwrap();
        let _gids = reader.u32().unwrap();
        body.len() - reader.rest().len()
    }
}
