use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    async fn put(&self, path: &str, bytes: &[u8]) -> Result<(), std::io::Error>;
    async fn delete(&self, path: &str);
    async fn exists(&self, path: &str) -> bool;
    async fn read(&self, path: &str) -> Result<Vec<u8>, std::io::Error>;
    fn url(&self, path: &str) -> String;
}

#[derive(Clone)]
pub struct LocalStorage {
    root: PathBuf,
    public_prefix: String,
}

impl LocalStorage {
    pub fn new(root: PathBuf, public_prefix: String) -> Self {
        Self {
            root,
            public_prefix,
        }
    }

    fn resolve(&self, path: &str) -> Option<PathBuf> {
        let p = Path::new(path);
        if p.components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
        {
            return None;
        }
        Some(self.root.join(p))
    }
}

#[async_trait::async_trait]
impl Storage for LocalStorage {
    async fn put(&self, path: &str, bytes: &[u8]) -> Result<(), std::io::Error> {
        let dest = self
            .resolve(path)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad path"))?;
        if let Some(dir) = dest.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(dest, bytes).await
    }

    async fn delete(&self, path: &str) {
        if let Some(p) = self.resolve(path) {
            let _ = tokio::fs::remove_file(p).await;
        }
    }

    async fn exists(&self, path: &str) -> bool {
        self.resolve(path).is_some_and(|p| p.exists())
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        let p = self
            .resolve(path)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad path"))?;
        tokio::fs::read(p).await
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.public_prefix.trim_end_matches('/'), path)
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub struct S3Storage {
    client: reqwest::Client,
    endpoint: String,
    region: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    public_base: String,
    path_style: bool,
    acl: Option<String>,
}

impl S3Storage {
    pub fn from_env() -> Option<Self> {
        if std::env::var("STORAGE_DISK").unwrap_or_else(|_| "local".into()) != "s3" {
            return None;
        }
        let env_any = |names: &[&str]| -> Option<String> {
            names
                .iter()
                .filter_map(|n| std::env::var(n).ok())
                .find(|v| !v.is_empty())
        };
        let endpoint = env_any(&["S3_ENDPOINT", "OVH_ENDPOINT", "CONTABO_ENDPOINT"])?
            .trim_end_matches('/')
            .to_string();
        let bucket = env_any(&["S3_BUCKET", "OVH_BUCKET", "CONTABO_BUCKET"])?;
        let access_key = env_any(&["S3_ACCESS_KEY_ID", "OVH_ACCESS_KEY_ID", "CONTABO_ACCESS_KEY_ID"])?;
        let secret_key =
            env_any(&["S3_SECRET_ACCESS_KEY", "OVH_SECRET_ACCESS_KEY", "CONTABO_SECRET_ACCESS_KEY"])?;
        let region =
            env_any(&["S3_REGION", "OVH_DEFAULT_REGION", "CONTABO_DEFAULT_REGION", "AWS_DEFAULT_REGION"])
                .unwrap_or_else(|| "us-east-1".into());
        let path_style = env_any(&["S3_PATH_STYLE", "OVH_USE_PATH_STYLE_ENDPOINT", "CONTABO_USE_PATH_STYLE_ENDPOINT"])
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        let host = endpoint
            .strip_prefix("https://")
            .or_else(|| endpoint.strip_prefix("http://"))
            .unwrap_or(&endpoint);
        let public_base = env_any(&["S3_URL", "OVH_URL", "CONTABO_URL", "AWS_URL"])
            .unwrap_or_else(|| {
                if path_style {
                    format!("{endpoint}/{bucket}")
                } else {
                    format!("https://{bucket}.{host}")
                }
            })
            .trim_end_matches('/')
            .to_string();
        let acl = env_any(&["S3_ACL"]).or(Some("public-read".into()));
        Some(Self {
            client: reqwest::Client::new(),
            endpoint,
            region,
            bucket,
            access_key,
            secret_key,
            public_base,
            path_style,
            acl: acl.filter(|a| !a.is_empty()),
        })
    }

    fn host_and_uri(&self, key: &str) -> (String, String) {
        let host = self
            .endpoint
            .strip_prefix("https://")
            .or_else(|| self.endpoint.strip_prefix("http://"))
            .unwrap_or(&self.endpoint)
            .to_string();
        let enc_key = uri_encode(key, true);
        if self.path_style {
            (host, format!("/{}/{}", uri_encode(&self.bucket, false), enc_key))
        } else {
            (format!("{}.{}", self.bucket, host), format!("/{enc_key}"))
        }
    }

    fn sign(&self, method: &str, key: &str, body: &[u8]) -> (String, Vec<(String, String)>) {
        self.sign_at(method, key, body, &chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string())
    }

    fn sign_at(
        &self,
        method: &str,
        key: &str,
        body: &[u8],
        amzdate: &str,
    ) -> (String, Vec<(String, String)>) {
        use hmac::{Hmac, Mac};
        use sha2::Digest;
        type H = Hmac<sha2::Sha256>;
        let (host, uri) = self.host_and_uri(key);
        let datestamp = &amzdate[..8];
        let payload_hash = hex::encode(sha2::Sha256::digest(body));

        let mut headers = vec![
            ("host".to_string(), host.clone()),
            ("x-amz-content-sha256".to_string(), payload_hash.clone()),
            ("x-amz-date".to_string(), amzdate.to_string()),
        ];
        if method == "PUT" {
            if let Some(acl) = &self.acl {
                headers.push(("x-amz-acl".to_string(), acl.clone()));
            }
        }
        headers.sort();
        let canonical_headers: String =
            headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
        let signed_headers: String = headers
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let canonical = format!(
            "{method}\n{uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );
        let scope = format!("{datestamp}/{}/s3/aws4_request", self.region);
        let to_sign = format!(
            "AWS4-HMAC-SHA256\n{amzdate}\n{scope}\n{}",
            hex::encode(sha2::Sha256::digest(canonical.as_bytes()))
        );
        let mac = |key: &[u8], data: &str| -> Vec<u8> {
            let mut m = <H as Mac>::new_from_slice(key).unwrap();
            m.update(data.as_bytes());
            m.finalize().into_bytes().to_vec()
        };
        let k_date = mac(format!("AWS4{}", self.secret_key).as_bytes(), datestamp);
        let k_region = mac(&k_date, &self.region);
        let k_service = mac(&k_region, "s3");
        let k_signing = mac(&k_service, "aws4_request");
        let signature = hex::encode(mac(&k_signing, &to_sign));
        headers.push((
            "authorization".to_string(),
            format!(
                "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
                self.access_key
            ),
        ));
        let scheme = if self.endpoint.starts_with("http://") { "http" } else { "https" };
        (format!("{scheme}://{host}{uri}"), headers)
    }

    async fn send(
        &self,
        method: reqwest::Method,
        key: &str,
        body: Vec<u8>,
    ) -> Result<reqwest::Response, std::io::Error> {
        let (url, headers) = self.sign(method.as_str(), key, &body);
        let mut req = self.client.request(method, &url).body(body);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        req.send().await.map_err(to_io)
    }
}

fn to_io(e: reqwest::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e)
}

fn uri_encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b'/' if keep_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[async_trait::async_trait]
impl Storage for S3Storage {
    async fn put(&self, path: &str, bytes: &[u8]) -> Result<(), std::io::Error> {
        let resp = self.send(reqwest::Method::PUT, path, bytes.to_vec()).await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("s3 put {}: {}", path, resp.status()),
            ))
        }
    }

    async fn delete(&self, path: &str) {
        let _ = self.send(reqwest::Method::DELETE, path, Vec::new()).await;
    }

    async fn exists(&self, path: &str) -> bool {
        self.send(reqwest::Method::HEAD, path, Vec::new())
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        let resp = self.send(reqwest::Method::GET, path, Vec::new()).await?;
        if !resp.status().is_success() {
            return Err(std::io::Error::new(
                if resp.status() == reqwest::StatusCode::NOT_FOUND {
                    std::io::ErrorKind::NotFound
                } else {
                    std::io::ErrorKind::Other
                },
                format!("s3 get {}: {}", path, resp.status()),
            ));
        }
        resp.bytes().await.map(|b| b.to_vec()).map_err(to_io)
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.public_base, uri_encode(path, true))
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub struct UpstreamFallback {
    inner: Arc<dyn Storage>,
    upstream: String,
    client: reqwest::Client,
}

impl UpstreamFallback {
    pub fn new(inner: Arc<dyn Storage>, upstream: String) -> Self {
        Self {
            inner,
            upstream: upstream.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

#[async_trait::async_trait]
impl Storage for UpstreamFallback {
    async fn put(&self, path: &str, bytes: &[u8]) -> Result<(), std::io::Error> {
        self.inner.put(path, bytes).await
    }

    async fn delete(&self, path: &str) {
        self.inner.delete(path).await
    }

    async fn exists(&self, path: &str) -> bool {
        if self.inner.exists(path).await {
            return true;
        }
        self.client
            .head(format!("{}/{}", self.upstream, uri_encode(path, true)))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        match self.inner.read(path).await {
            Ok(b) => Ok(b),
            Err(e) => {
                let url = format!("{}/{}", self.upstream, uri_encode(path, true));
                let resp = self.client.get(&url).send().await.map_err(to_io)?;
                if !resp.status().is_success() {
                    return Err(e);
                }
                let bytes = resp.bytes().await.map_err(to_io)?.to_vec();
                let inner = self.inner.clone();
                let path = path.to_string();
                let data = bytes.clone();
                tokio::spawn(async move { let _ = inner.put(&path, &data).await; });
                Ok(bytes)
            }
        }
    }

    fn url(&self, path: &str) -> String {
        self.inner.url(path)
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub fn backend_from_env(root: PathBuf, public_prefix: String) -> Arc<dyn Storage> {
    let inner: Arc<dyn Storage> = match S3Storage::from_env() {
        Some(s3) => {
            tracing::info!(bucket = %s3.bucket, "storage: s3 backend");
            Arc::new(s3)
        }
        None => Arc::new(LocalStorage::new(root, public_prefix)),
    };
    match std::env::var("STORAGE_UPSTREAM_URL")
        .ok()
        .filter(|v| !v.is_empty())
    {
        Some(up) => {
            tracing::info!(upstream = %up, "storage: lazy upstream fallback enabled");
            Arc::new(UpstreamFallback::new(inner, up))
        }
        None => inner,
    }
}

pub async fn serve_stored(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::{header, StatusCode};
    use axum::response::IntoResponse;
    match state.storage.read(&path).await {
        Ok(bytes) => {
            let mime = mime_for(&path);
            (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, "public, max-age=3600".into()),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn mime_for(path: &str) -> String {
    match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "webp" => "image/webp",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "json" => "application/json",
        "css" => "text/css",
        "js" => "text/javascript",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
    .into()
}
pub fn image_to_webp_contain(bytes: &[u8], w: u32, h: u32) -> Result<Vec<u8>, ()> {
    let img = image::load_from_memory(bytes).map_err(|_| ())?;
    let img = img.thumbnail(w, h);
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::WebP).map_err(|_| ())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rejects_traversal() {
        let s = LocalStorage::new("/tmp/x".into(), "/storage".into());
        assert!(s.resolve("users/a.webp").is_some());
        assert!(s.resolve("../etc/passwd").is_none());
        assert!(s.resolve("/abs/path").is_none());
    }

    #[test]
    fn url_joins_prefix() {
        let s = LocalStorage::new("/tmp/x".into(), "/storage/".into());
        assert_eq!(s.url("users/a.webp"), "/storage/users/a.webp");
    }

    fn s3(path_style: bool) -> S3Storage {
        S3Storage {
            client: reqwest::Client::new(),
            endpoint: "https://s3.gra.io.cloud.ovh.net".into(),
            region: "gra".into(),
            bucket: "blindall".into(),
            access_key: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
            public_base: "https://blindall.s3.gra.io.cloud.ovh.net".into(),
            path_style,
            acl: Some("public-read".into()),
        }
    }

    #[test]
    fn s3_vhost_uri() {
        let (host, uri) = s3(false).host_and_uri("photos/room 1.webp");
        assert_eq!(host, "blindall.s3.gra.io.cloud.ovh.net");
        assert_eq!(uri, "/photos/room%201.webp");
    }

    #[test]
    fn s3_path_style_uri() {
        let (host, uri) = s3(true).host_and_uri("photos/room 1.webp");
        assert_eq!(host, "s3.gra.io.cloud.ovh.net");
        assert_eq!(uri, "/blindall/photos/room%201.webp");
    }

    #[test]
    fn s3_signature_is_stable() {
        let (url, headers) =
            s3(false).sign_at("GET", "photos/a.webp", &[], "20240101T000000Z");
        assert_eq!(url, "https://blindall.s3.gra.io.cloud.ovh.net/photos/a.webp");
        let auth = headers
            .iter()
            .find(|(k, _)| k == "authorization")
            .map(|(_, v)| v.clone())
            .unwrap();
        assert!(auth.starts_with(
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20240101/gra/s3/aws4_request"
        ));
        assert!(auth.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"));
        let sig = auth.rsplit("Signature=").next().unwrap();
        assert_eq!(sig.len(), 64, "hex sha256 signature");
        let (_, again) = s3(false).sign_at("GET", "photos/a.webp", &[], "20240101T000000Z");
        assert_eq!(headers.last().unwrap().1, again.last().unwrap().1);
    }

    #[test]
    fn s3_put_signs_acl_header() {
        let (_, headers) = s3(false).sign_at("PUT", "x.webp", b"body", "20240101T000000Z");
        assert!(headers.iter().any(|(k, v)| k == "x-amz-acl" && v == "public-read"));
        let auth = &headers.last().unwrap().1;
        assert!(auth.contains("x-amz-acl"), "acl must be in SignedHeaders");
    }

    #[test]
    fn uri_encode_strict() {
        assert_eq!(uri_encode("a b/c.png", true), "a%20b/c.png");
        assert_eq!(uri_encode("été.mp3", true), "%C3%A9t%C3%A9.mp3");
        assert_eq!(uri_encode("ok_-.~", false), "ok_-.~");
        assert_eq!(uri_encode("a/b", false), "a%2Fb");
    }

    #[test]
    fn mime_map() {
        assert_eq!(mime_for("a.webp"), "image/webp");
        assert_eq!(mime_for("a.MP3"), "audio/mpeg");
        assert_eq!(mime_for("a.bin"), "application/octet-stream");
    }
}
