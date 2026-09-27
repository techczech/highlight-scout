use anyhow::{anyhow, bail, Context, Result};
use md5::Md5;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::Method;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{self, Config};

/// AWS SigV4 canonical encoding: percent-encode everything except the
/// unreserved characters (A-Z a-z 0-9 - . _ ~). Anything laxer (e.g. leaving
/// '=' or parentheses bare) makes the server compute a different canonical
/// request and reject the signature.
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');
const QUERY_ENCODE_SET: &AsciiSet = ENCODE_SET;

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
    client
        .list(&key_for(&config.r2_prefix, "archive", ""), 1)
        .await?;
    Ok(())
}

pub async fn push_archive(config: &Config) -> Result<R2Progress> {
    let creds = load_credentials()?;
    let client = R2Client::new(config.clone(), creds)?;
    let archive_root = PathBuf::from(&config.archive_path);
    if !archive_root.exists() {
        bail!(
            "Local highlights folder does not exist: {}",
            archive_root.display()
        );
    }

    // One LIST gives every remote key + content etag, so changed files are
    // detected without a HEAD per file and unchanged ones are never re-sent.
    let archive_prefix = key_for(&config.r2_prefix, "archive", "");
    let remote = client.list_all(&archive_prefix).await?;
    let remote: HashMap<String, Option<String>> =
        remote.into_iter().map(|e| (e.key, e.etag)).collect();

    let mut uploaded = 0;
    let mut skipped = 0;
    let mut failures: Vec<String> = Vec::new();
    for path in list_files(&archive_root)? {
        let rel = relative_key(&archive_root, &path)?;
        let key = key_for(&config.r2_prefix, "archive", &rel);
        match fs::read(&path) {
            Ok(bytes) => {
                if unchanged(remote.get(&key).cloned().flatten().as_deref(), &bytes) {
                    skipped += 1;
                    continue;
                }
                match client.put(&key, bytes).await {
                    Ok(()) => uploaded += 1,
                    Err(e) => failures.push(format!("{}: {}", rel, e)),
                }
            }
            Err(e) => failures.push(format!("{}: {}", rel, e)),
        }
    }

    let index = config::index_path();
    if index.exists() {
        let key = key_for(&config.r2_prefix, "index", "index.sqlite");
        match fs::read(&index) {
            Ok(bytes) => {
                let remote_etag = client.head(&key).await.unwrap_or(None);
                if unchanged(remote_etag.as_deref(), &bytes) {
                    skipped += 1;
                } else {
                    match client.put(&key, bytes).await {
                        Ok(()) => uploaded += 1,
                        Err(e) => failures.push(format!("index.sqlite: {}", e)),
                    }
                }
            }
            Err(e) => failures.push(format!("index.sqlite: {}", e)),
        }
    }

    let failed = failures.len();
    Ok(R2Progress {
        uploaded,
        downloaded: 0,
        skipped,
        failed,
        message: progress_message("uploaded (new or changed)", uploaded, skipped, &failures),
    })
}

pub async fn pull_archive(config: &Config) -> Result<R2Progress> {
    let creds = load_credentials()?;
    let client = R2Client::new(config.clone(), creds)?;
    let archive_root = PathBuf::from(&config.archive_path);
    fs::create_dir_all(&archive_root)?;

    let archive_prefix = key_for(&config.r2_prefix, "archive", "");
    let entries = client.list_all(&archive_prefix).await?;
    let mut downloaded = 0;
    let mut skipped = 0;
    let mut failures: Vec<String> = Vec::new();
    for entry in entries {
        let key = entry.key;
        let rel = key
            .strip_prefix(archive_prefix.trim_end_matches('/'))
            .unwrap_or(&key)
            .trim_start_matches('/');
        if rel.is_empty() {
            continue;
        }
        let dest = archive_root.join(rel);
        // Restore overwrites local files whose content differs from the
        // backup; identical files are left untouched.
        if dest.exists() {
            if let Ok(local) = fs::read(&dest) {
                if unchanged(entry.etag.as_deref(), &local) {
                    skipped += 1;
                    continue;
                }
            }
        }
        match client.get_to_file(&key, &dest).await {
            Ok(()) => downloaded += 1,
            Err(e) => failures.push(format!("{}: {}", rel, e)),
        }
    }

    let index_key = key_for(&config.r2_prefix, "index", "index.sqlite");
    if client.head(&index_key).await.unwrap_or(None).is_some() {
        if let Some(parent) = config::index_path().parent() {
            fs::create_dir_all(parent)?;
        }
        match client.get_to_file(&index_key, &config::index_path()).await {
            Ok(()) => downloaded += 1,
            Err(e) => failures.push(format!("index.sqlite: {}", e)),
        }
    }

    let failed = failures.len();
    Ok(R2Progress {
        uploaded: 0,
        downloaded,
        skipped,
        failed,
        message: progress_message("downloaded", downloaded, skipped, &failures),
    })
}

/// "N <verb>, M unchanged" plus up to three named failures so problems are
/// visible in the UI instead of being reduced to a count.
fn progress_message(verb: &str, done: usize, skipped: usize, failures: &[String]) -> String {
    let mut msg = format!("{} {}, {} unchanged", done, verb, skipped);
    if !failures.is_empty() {
        let shown = failures
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        let more = if failures.len() > 3 {
            format!(" (+{} more)", failures.len() - 3)
        } else {
            String::new()
        };
        msg.push_str(&format!(", {} FAILED: {}{}", failures.len(), shown, more));
    }
    msg
}

fn list_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git"
                || name == ".DS_Store"
                || crate::archive_meta::is_temp_write_name(&name)
            {
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
        .args([
            "add-generic-password",
            "-U",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            account,
            "-w",
            value,
        ])
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
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            account,
            "-w",
        ])
        .output()
        .context("run macOS security")?;
    if !output.status.success() {
        bail!("R2 credentials are not saved in Keychain");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
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

    /// Ok(Some(etag)) if the object exists (etag may be empty when the header
    /// is missing), Ok(None) if it does not.
    async fn head(&self, key: &str) -> Result<Option<String>> {
        let response = self.request(Method::HEAD, key, None, Vec::new()).await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(normalize_etag)
            .unwrap_or_default();
        Ok(Some(etag))
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

    async fn list_all(&self, prefix: &str) -> Result<Vec<ListEntry>> {
        let mut entries = Vec::new();
        let mut token = None;
        loop {
            let page = self.list_page(prefix, 1000, token.as_deref()).await?;
            entries.extend(page.entries);
            if page.next_token.is_none() {
                break;
            }
            token = page.next_token;
        }
        Ok(entries)
    }

    async fn list_page(
        &self,
        prefix: &str,
        max_keys: usize,
        token: Option<&str>,
    ) -> Result<ListPage> {
        let mut query = vec![
            ("list-type".to_string(), "2".to_string()),
            ("max-keys".to_string(), max_keys.to_string()),
            ("prefix".to_string(), prefix.to_string()),
        ];
        if let Some(token) = token {
            query.push(("continuation-token".to_string(), token.to_string()));
        }
        let response = self
            .request(Method::GET, "", Some(query), Vec::new())
            .await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!(
                "R2 list failed: HTTP {} — {}",
                status,
                body.chars().take(2500).collect::<String>()
            );
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
        let uri = format!("/{}/{}", self.config.r2_bucket.trim(), encoded_key)
            .trim_end_matches('/')
            .to_string();
        let url = format!(
            "{}{}{}",
            self.endpoint,
            uri,
            query_string(query.as_deref(), false)
        );
        let host = self
            .endpoint
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let canonical_query = query_string(query.as_deref(), true)
            .trim_start_matches('?')
            .to_string();
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

        if std::env::var("HS_R2_DEBUG").is_ok() {
            eprintln!(
                "URL: {}\n--- canonical request ---\n{}\n---",
                url, canonical_request
            );
        }
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
    entries: Vec<ListEntry>,
    next_token: Option<String>,
}

#[derive(Debug)]
struct ListEntry {
    key: String,
    /// Content MD5 for single-part uploads; None when R2 reports a
    /// non-comparable etag (multipart) or none at all.
    etag: Option<String>,
}

fn parse_list_page(xml: &str) -> ListPage {
    let entries = xml_values(xml, "Contents")
        .iter()
        .filter_map(|block| {
            let key = xml_values(block, "Key").into_iter().next()?;
            let etag = xml_values(block, "ETag")
                .into_iter()
                .next()
                .map(|raw| normalize_etag(&raw))
                .filter(|e| !e.is_empty() && !e.contains('-'));
            Some(ListEntry { key, etag })
        })
        .collect();
    ListPage {
        entries,
        next_token: xml_values(xml, "NextContinuationToken").into_iter().next(),
    }
}

fn normalize_etag(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("W/")
        .trim_matches('"')
        .to_ascii_lowercase()
}

fn md5_hex(data: &[u8]) -> String {
    hex::encode(Md5::digest(data))
}

/// True only when the remote etag is a comparable content hash that matches
/// the local bytes. Unknown or non-comparable etags count as changed, so the
/// caller re-uploads rather than silently keeping a stale copy.
fn unchanged(remote_etag: Option<&str>, local: &[u8]) -> bool {
    matches!(remote_etag, Some(etag) if etag == md5_hex(local))
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
    let Some(query) = query else {
        return String::new();
    };
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
        assert_eq!(
            endpoint(&c).unwrap(),
            "https://abc123.r2.cloudflarestorage.com"
        );
    }

    #[test]
    fn backup_listing_skips_interrupted_temp_writes() {
        let root = std::env::temp_dir().join(format!("hs-r2-list-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let works = root.join("readings/works");
        fs::create_dir_all(&works).unwrap();
        fs::write(works.join("a.md"), b"a").unwrap();
        fs::write(works.join(".a.md.tmp-123-456"), b"partial").unwrap();
        let names: Vec<String> = list_files(&root)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.md"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn r2_key_mirrors_archive_paths_under_prefix() {
        assert_eq!(
            key_for("scout", "archive", "readings/works/a file.md"),
            "scout/archive/readings/works/a file.md"
        );
    }

    #[test]
    fn list_parser_extracts_keys_etags_and_continuation_token() {
        let page = parse_list_page(
            "<ListBucketResult>\
             <Contents><Key>scout/archive/a&amp;b.md</Key><ETag>&quot;900150983cd24fb0d6963f7d28e17f72&quot;</ETag></Contents>\
             <Contents><Key>scout/archive/multi.bin</Key><ETag>&quot;abc123-4&quot;</ETag></Contents>\
             <NextContinuationToken>next</NextContinuationToken></ListBucketResult>",
        );
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].key, "scout/archive/a&b.md");
        assert_eq!(
            page.entries[0].etag.as_deref(),
            Some("900150983cd24fb0d6963f7d28e17f72")
        );
        assert_eq!(page.entries[1].key, "scout/archive/multi.bin");
        assert_eq!(
            page.entries[1].etag, None,
            "multipart etags are not comparable"
        );
        assert_eq!(page.next_token.as_deref(), Some("next"));
    }

    #[test]
    fn unchanged_only_when_etag_matches_content_md5() {
        // md5("abc") — the classic known vector.
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert!(unchanged(Some("900150983cd24fb0d6963f7d28e17f72"), b"abc"));
        assert!(!unchanged(
            Some("900150983cd24fb0d6963f7d28e17f72"),
            b"abcd"
        ));
        assert!(!unchanged(Some(""), b"abc"), "missing etag means re-upload");
        assert!(!unchanged(None, b"abc"), "absent object means upload");
    }

    #[test]
    fn query_and_path_encoding_follow_aws_unreserved_rules() {
        // '=' (base64 padding in continuation tokens) must become %3D — the
        // exact bug that broke LIST pagination against R2.
        assert_eq!(encode_query_value("N0Q="), "N0Q%3D");
        assert_eq!(encode_query_value("a/b"), "a%2Fb");
        assert_eq!(encode_query_value("safe-._~"), "safe-._~");
        // Path segments: parens and '=' encoded, '/' preserved as separator.
        assert_eq!(encode_key("works/a (b)=c.md"), "works/a%20%28b%29%3Dc.md");
    }

    #[test]
    fn etag_normalization_strips_quotes_weak_prefix_and_case() {
        assert_eq!(normalize_etag("\"ABC123\""), "abc123");
        assert_eq!(normalize_etag("W/\"abc\""), "abc");
        assert_eq!(normalize_etag(" \"abc\" "), "abc");
    }

    /// Live round-trip against the real bucket: push the configured archive,
    /// pull it back into a scratch folder, and compare every file's bytes.
    /// Needs r2 settings in the app config and credentials in the Keychain.
    /// Note: pull_archive also overwrites the live index.sqlite with the
    /// just-pushed copy — quit the app and snapshot the index before running.
    ///
    ///   cargo test --lib r2::tests::live_r2_round_trip -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "live R2 round-trip; requires Keychain creds + r2 config"]
    async fn live_r2_round_trip() {
        let cfg = crate::config::load();
        assert!(cfg.r2_enabled, "r2_enabled is false in config");
        assert!(has_credentials(), "no R2 credentials in Keychain");

        let push = push_archive(&cfg).await.expect("push_archive failed");
        println!("push: {}", push.message);
        assert_eq!(push.failed, 0, "push had failures: {}", push.message);

        let scratch = std::env::temp_dir().join(format!("hs-r2-verify-{}", std::process::id()));
        let mut restore_cfg = cfg.clone();
        restore_cfg.archive_path = scratch.to_string_lossy().to_string();
        let pull = pull_archive(&restore_cfg)
            .await
            .expect("pull_archive failed");
        println!("pull: {}", pull.message);
        assert_eq!(pull.failed, 0, "pull had failures: {}", pull.message);

        let original_root = PathBuf::from(&cfg.archive_path);
        let mut compared = 0usize;
        for path in list_files(&original_root).expect("list original archive") {
            let rel = relative_key(&original_root, &path).unwrap();
            let restored = scratch.join(&rel);
            assert!(restored.exists(), "missing from restore: {}", rel);
            assert_eq!(
                fs::read(&path).unwrap(),
                fs::read(&restored).unwrap(),
                "content mismatch: {}",
                rel
            );
            compared += 1;
        }
        println!("verified {} files byte-for-byte", compared);
        assert!(compared > 0, "archive was empty — nothing verified");
        let _ = fs::remove_dir_all(&scratch);
    }
}
