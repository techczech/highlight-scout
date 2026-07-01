use anyhow::{anyhow, bail, Context, Result};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::Method;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{self, Config};

const ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b':')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');
const QUERY_ENCODE_SET: &AsciiSet = &ENCODE_SET.add(b'/');

const KEYCHAIN_SERVICE: &str = "Highlight Scout R2";
const ACCESS_KEY_ACCOUNT: &str = "access_key_id";
const SECRET_KEY_ACCOUNT: &str = "secret_access_key";

#[derive(Debug, Clone)]
pub struct R2Creds {
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Debug, Clone)]
pub struct R2Progress {
    pub uploaded: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub message: String,
}

pub fn endpoint(config: &Config) -> Result<String> {
    let explicit = config.r2_endpoint.trim();
    if !explicit.is_empty() {
        return Ok(explicit.trim_end_matches('/').to_string());
    }
    let account = config.r2_account_id.trim();
    if account.is_empty() {
        bail!("R2 account id is not set");
    }
    Ok(format!("https://{}.r2.cloudflarestorage.com", account))
}

pub fn key_for(prefix: &str, area: &str, rel_path: &str) -> String {
    [prefix, area, rel_path]
        .iter()
        .map(|s| s.trim_matches('/'))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn has_credentials() -> bool {
    load_credentials().is_ok()
}

pub fn save_credentials(access_key_id: &str, secret_access_key: &str) -> Result<()> {
    save_secret(ACCESS_KEY_ACCOUNT, access_key_id)?;
    save_secret(SECRET_KEY_ACCOUNT, secret_access_key)?;
    Ok(())
}

pub fn load_credentials() -> Result<R2Creds> {
    Ok(R2Creds {
        access_key_id: find_secret(ACCESS_KEY_ACCOUNT)?,
        secret_access_key: find_secret(SECRET_KEY_ACCOUNT)?,
    })
}

pub async fn test_connection(config: &Config) -> Result<()> {
    let creds = load_credentials()?;
    let client = R2Client::new(config.clone(), creds)?;
    client.list(&key_for(&config.r2_prefix, "archive", ""), 1).await?;
    Ok(())
}

pub async fn push_archive(config: &Config) -> Result<R2Progress> {
    let creds = load_credentials()?;
    let client = R2Client::new(config.clone(), creds)?;
    let archive_root = PathBuf::from(&config.archive_path);
    if !archive_root.exists() {
        bail!("Local highlights folder does not exist: {}", archive_root.display());
    }

    let mut uploaded = 0;
    let mut skipped = 0;
    let mut failed = 0;
    for path in list_files(&archive_root)? {
        let rel = relative_key(&archive_root, &path)?;
        let key = key_for(&config.r2_prefix, "archive", &rel);
        match client.head(&key).await {
            Ok(true) => {
                skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(_) => {}
        }
        match fs::read(&path) {
            Ok(bytes) => match client.put(&key, bytes).await {
                Ok(()) => uploaded += 1,
                Err(_) => failed += 1,
            },
            Err(_) => failed += 1,
        }
    }

    let index = config::index_path();
    if index.exists() {
        let key = key_for(&config.r2_prefix, "index", "index.sqlite");
        match fs::read(&index) {
            Ok(bytes) => match client.put(&key, bytes).await {
                Ok(()) => uploaded += 1,
                Err(_) => failed += 1,
            },
            Err(_) => failed += 1,
        }
    }

    Ok(R2Progress {
        uploaded,
        downloaded: 0,
        skipped,
        failed,
        message: format!("{} uploaded, {} already present, {} failed", uploaded, skipped, failed),
    })
}

pub async fn pull_archive(config: &Config) -> Result<R2Progress> {
    let creds = load_credentials()?;
    let client = R2Client::new(config.clone(), creds)?;
    let archive_root = PathBuf::from(&config.archive_path);
    fs::create_dir_all(&archive_root)?;

    let archive_prefix = key_for(&config.r2_prefix, "archive", "");
    let keys = client.list_all(&archive_prefix).await?;
    let mut downloaded = 0;
    let mut skipped = 0;
    let mut failed = 0;
    for key in keys {
        let rel = key
            .strip_prefix(archive_prefix.trim_end_matches('/'))
            .unwrap_or(&key)
            .trim_start_matches('/');
        if rel.is_empty() {
            continue;
        }
        let dest = archive_root.join(rel);
        if dest.exists() {
            skipped += 1;
            continue;
        }
        match client.get_to_file(&key, &dest).await {
            Ok(()) => downloaded += 1,
            Err(_) => failed += 1,
        }
    }

    let index_key = key_for(&config.r2_prefix, "index", "index.sqlite");
    if client.head(&index_key).await.unwrap_or(false) {
        if let Some(parent) = config::index_path().parent() {
            fs::create_dir_all(parent)?;
        }
        match client.get_to_file(&index_key, &config::index_path()).await {
            Ok(()) => downloaded += 1,
            Err(_) => failed += 1,
        }
    }

    Ok(R2Progress {
        uploaded: 0,
        downloaded,
        skipped,
        failed,
        message: format!("{} downloaded, {} already present, {} failed", downloaded, skipped, failed),
    })
}

fn list_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git" || name == ".DS_Store" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out)?;
            } else if path.is_file() && path.strip_prefix(root).is_ok() {
                out.push(path);
            }
        }
        Ok(())
    }
    walk(root, root, &mut out)?;
    Ok(out)
}

fn relative_key(root: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(root)?
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn save_secret(account: &str, value: &str) -> Result<()> {
    let status = Command::new("security")
        .args(["add-generic-password", "-U", "-s", KEYCHAIN_SERVICE, "-a", account, "-w", value])
        .status()
        .context("run macOS security")?;
    if status.success() {
        Ok(())
    } else {
        bail!("could not save R2 credentials to Keychain")
    }
}

fn find_secret(account: &str) -> Result<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-a", account, "-w"])
        .output()
        .context("run macOS security")?;
    if !output.status.success() {
        bail!("R2 credentials are not saved in Keychain");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
}

struct R2Client {
    config: Config,
    creds: R2Creds,
    endpoint: String,
    http: reqwest::Client,
}

impl R2Client {
    fn new(config: Config, creds: R2Creds) -> Result<Self> {
        if config.r2_bucket.trim().is_empty() {
            bail!("R2 bucket is not set");
        }
        Ok(Self {
            endpoint: endpoint(&config)?,
            config,
            creds,
            http: reqwest::Client::new(),
        })
    }

    async fn head(&self, key: &str) -> Result<bool> {
        let response = self.request(Method::HEAD, key, None, Vec::new()).await?;
        Ok(response.status().is_success())
    }

    async fn put(&self, key: &str, body: Vec<u8>) -> Result<()> {
        let response = self.request(Method::PUT, key, None, body).await?;
        if response.status().is_success() {
            Ok(())
        } else {
            bail!("R2 PUT failed for {}: HTTP {}", key, response.status())
        }
    }

    async fn get_to_file(&self, key: &str, dest: &Path) -> Result<()> {
        let response = self.request(Method::GET, key, None, Vec::new()).await?;
        if !response.status().is_success() {
            bail!("R2 GET failed for {}: HTTP {}", key, response.status());
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(dest, response.bytes().await?)?;
        Ok(())
    }

    async fn list(&self, prefix: &str, max_keys: usize) -> Result<ListPage> {
        self.list_page(prefix, max_keys, None).await
    }

    async fn list_all(&self, prefix: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut token = None;
        loop {
            let page = self.list_page(prefix, 1000, token.as_deref()).await?;
            keys.extend(page.keys);
            if page.next_token.is_none() {
                break;
            }
            token = page.next_token;
        }
        Ok(keys)
    }

    async fn list_page(&self, prefix: &str, max_keys: usize, token: Option<&str>) -> Result<ListPage> {
        let mut query = vec![
            ("list-type".to_string(), "2".to_string()),
            ("max-keys".to_string(), max_keys.to_string()),
            ("prefix".to_string(), prefix.to_string()),
        ];
        if let Some(token) = token {
            query.push(("continuation-token".to_string(), token.to_string()));
        }
        let response = self.request(Method::GET, "", Some(query), Vec::new()).await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("R2 list failed: HTTP {}", status);
        }
        Ok(parse_list_page(&body))
    }

    async fn request(
        &self,
        method: Method,
        key: &str,
        query: Option<Vec<(String, String)>>,
        body: Vec<u8>,
    ) -> Result<reqwest::Response> {
        let now = chrono::Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();
        let payload_hash = sha256_hex(&body);
        let encoded_key = encode_key(key);
        let uri = format!("/{}/{}", self.config.r2_bucket.trim(), encoded_key).trim_end_matches('/').to_string();
        let url = format!("{}{}{}", self.endpoint, uri, query_string(query.as_deref(), false));
        let host = self.endpoint.trim_start_matches("https://").trim_start_matches("http://");
        let canonical_query = query_string(query.as_deref(), true).trim_start_matches('?').to_string();
        let canonical_headers = format!(
            "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            host, payload_hash, amz_date
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.as_str(),
            uri,
            canonical_query,
            canonical_headers,
            signed_headers,
            payload_hash
        );
        let credential_scope = format!("{}/auto/s3/aws4_request", date);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            sha256_hex(canonical_request.as_bytes())
        );
        let signing_key = signing_key(&self.creds.secret_access_key, &date);
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
        let auth = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.creds.access_key_id, credential_scope, signed_headers, signature
        );

        let mut request = self
            .http
            .request(method, url)
            .header("host", host)
            .header("x-amz-content-sha256", payload_hash)
            .header("x-amz-date", amz_date)
            .header("authorization", auth);
        if !body.is_empty() {
            request = request.body(body);
        }
        request.send().await.map_err(|e| anyhow!(e))
    }
}

#[derive(Debug)]
struct ListPage {
    keys: Vec<String>,
    next_token: Option<String>,
}

fn parse_list_page(xml: &str) -> ListPage {
    ListPage {
        keys: xml_values(xml, "Key"),
        next_token: xml_values(xml, "NextContinuationToken").into_iter().next(),
    }
}

fn xml_values(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        let Some(end) = after.find(&close) else { break };
        out.push(xml_unescape(&after[..end]));
        rest = &after[end + close.len()..];
    }
    out
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn encode_key(key: &str) -> String {
    key.split('/')
        .filter(|part| !part.is_empty())
        .map(|part| utf8_percent_encode(part, ENCODE_SET).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_query_value(value: &str) -> String {
    utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
}

fn query_string(query: Option<&[(String, String)]>, canonical: bool) -> String {
    let Some(query) = query else { return String::new() };
    let mut parts = query
        .iter()
        .map(|(k, v)| (encode_query_value(k), encode_query_value(v)))
        .collect::<Vec<_>>();
    if canonical {
        parts.sort();
    }
    format!(
        "?{}",
        parts
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&")
    )
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut key_block = [0u8; 64];
    if key.len() > 64 {
        key_block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut o_key_pad = [0x5cu8; 64];
    let mut i_key_pad = [0x36u8; 64];
    for i in 0..64 {
        o_key_pad[i] ^= key_block[i];
        i_key_pad[i] ^= key_block[i];
    }
    let mut inner = Sha256::new();
    inner.update(i_key_pad);
    inner.update(data);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner_hash);
    outer.finalize().to_vec()
}

fn signing_key(secret: &str, date: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{}", secret).as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, b"auto");
    let k_service = hmac_sha256(&k_region, b"s3");
    hmac_sha256(&k_service, b"aws4_request")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r2_endpoint_derives_from_account_id() {
        let mut c = Config::default();
        c.r2_account_id = "abc123".into();
        assert_eq!(endpoint(&c).unwrap(), "https://abc123.r2.cloudflarestorage.com");
    }

    #[test]
    fn r2_key_mirrors_archive_paths_under_prefix() {
        assert_eq!(
            key_for("scout", "archive", "readings/works/a file.md"),
            "scout/archive/readings/works/a file.md"
        );
    }

    #[test]
    fn list_parser_extracts_keys_and_continuation_token() {
        let page = parse_list_page(
            "<ListBucketResult><Contents><Key>scout/archive/a&amp;b.md</Key></Contents><NextContinuationToken>next</NextContinuationToken></ListBucketResult>",
        );
        assert_eq!(page.keys, vec!["scout/archive/a&b.md"]);
        assert_eq!(page.next_token.as_deref(), Some("next"));
    }
}
