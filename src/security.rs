// src/security.rs

use anyhow::{anyhow, Result};
use std::net::{IpAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
use tracing::warn;

/// Trait to abstract DNS resolution for testing purposes.
pub trait DnsResolver: Send + Sync + std::fmt::Debug {
    fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>>;
}

#[derive(Debug)]
struct SystemDnsResolver;

impl DnsResolver for SystemDnsResolver {
    fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
        let socket_addrs = format!("{}:443", host).to_socket_addrs()?;
        Ok(socket_addrs.map(|a| a.ip()).collect())
    }
}

/// Configuration for Safe Mode execution.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SafeModeConfig {
    /// Is safe mode active?
    pub enabled: bool,
    /// List of allowed domains (e.g., "github.com"). If None, and enabled is true, strict default applies.
    pub allowed_domains: Option<Vec<String>>,
    /// Max size of a single file to read into memory.
    pub max_file_size: u64,
    /// Max size of the cloned repository on disk.
    pub max_repo_size: u64,
    /// Max size of the generated output buffer.
    pub max_output_size: u64,
    /// Max number of files to process.
    pub max_file_count: usize,
    /// Hard timeout for operations.
    pub request_timeout: Duration,
    /// Allow copying to server clipboard?
    pub allow_clipboard: bool,
    /// Allow local file paths?
    pub allow_local_paths: bool,
    /// Allow symlinks?
    pub allow_symlinks: bool,
    /// Optional override for the git cache directory.
    pub cache_dir: Option<std::path::PathBuf>,
    /// DNS resolver for validation.
    pub dns_resolver: Arc<dyn DnsResolver>,
}

impl Default for SafeModeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_domains: None,
            max_file_size: u64::MAX,
            max_repo_size: u64::MAX,
            max_output_size: u64::MAX,
            max_file_count: usize::MAX,
            request_timeout: Duration::from_secs(3600),
            allow_clipboard: true,
            allow_local_paths: true,
            allow_symlinks: true,
            cache_dir: None,
            dns_resolver: Arc::new(SystemDnsResolver),
        }
    }
}

impl SafeModeConfig {
    /// Returns the strict configuration used when --safe is passed without overrides.
    pub fn strict() -> Self {
        Self {
            enabled: true,
            // Default: Only GitHub and GitLab.
            allowed_domains: Some(vec!["github.com".to_string(), "gitlab.com".to_string()]),
            max_file_size: 10 * 1024 * 1024,   // 10 MB
            max_repo_size: 500 * 1024 * 1024,  // 500 MB
            max_output_size: 50 * 1024 * 1024, // 50 MB
            max_file_count: 1000,
            request_timeout: Duration::from_secs(30),
            allow_clipboard: false,
            allow_local_paths: false,
            allow_symlinks: false,
            cache_dir: None,
            dns_resolver: Arc::new(SystemDnsResolver),
        }
    }

    /// Validates an input string against security rules.
    /// Checks: Protocol (HTTPS only), Domain Allowlist, Local Path restriction.
    /// Returns: Ok(Some(IpAddr)) if a DNS resolution occurred and passed checks (for pinning).
    ///          Ok(None) if validation passed without DNS resolution (e.g. disabled, or no domain check needed).
    pub fn validate_input(&self, input: &str) -> Result<Option<IpAddr>> {
        if !self.enabled {
            return Ok(None);
        }

        // 1. Block Local Paths
        // Check for standard protocol separator "://" or the git-specific "ext::" protocol
        let has_protocol = input.contains("://") || input.starts_with("ext::");
        if !has_protocol {
            if self.allow_local_paths {
                return Ok(None);
            }
            warn!("Security Block: Local path access attempted: '{}'", input);
            return Err(anyhow!("Safe Mode: Local file system paths are disabled."));
        }

        // 2. Parse URL
        let url = Url::parse(input).map_err(|_| anyhow!("Safe Mode: Invalid URL format."))?;

        // 3. Enforce HTTPS (Prevents file://, ssh://, etc.)
        if url.scheme() != "https" {
            warn!(
                "Security Block: Non-HTTPS protocol attempted: '{}'",
                url.scheme()
            );
            return Err(anyhow!("Safe Mode: Only 'https' protocol is allowed."));
        }

        // 4. Domain Allowlist (SSRF Protection)
        if let Some(domains) = &self.allowed_domains {
            let host = url.host_str().unwrap_or("");
            // Check if host matches exactly or is a subdomain
            let allowed = domains
                .iter()
                .any(|d| host == d || host.ends_with(&format!(".{}", d)));
            if !allowed {
                warn!("Security Block: Domain not in allowlist: '{}'", host);
                return Err(anyhow!(
                    "Safe Mode: Domain '{}' is not in the allowlist. Allowed: {:?}",
                    host,
                    domains
                ));
            }

            // 5. DNS Rebinding / Private Network Check
            // Resolve the host to ensure it's not pointing to localhost or private IPs
            if let Some(host) = url.host_str() {
                let ips = self
                    .dns_resolver
                    .resolve(host)
                    .map_err(|_| anyhow!("Safe Mode: Could not resolve hostname '{}'", host))?;

                let mut resolved_ip = None;

                for ip in ips {
                    if ip.is_loopback()
                        || ip.is_unspecified()
                        || (ip.is_ipv4() && ip.to_string().starts_with("192.168."))
                        || (ip.is_ipv4() && ip.to_string().starts_with("10."))
                    {
                        warn!("Security Block: DNS Rebinding detected. Host '{}' resolved to private IP '{}'", host, ip);
                        return Err(anyhow!(
                            "Safe Mode: Domain resolves to a private/local IP address."
                        ));
                    }
                    if resolved_ip.is_none() {
                        resolved_ip = Some(ip);
                    }
                }
                return Ok(resolved_ip);
            }
        }

        Ok(None)
    }

    /// Validates a file path before reading to prevent Symlink/LFI attacks.
    /// Ensures the resolved path is physically inside the root directory.
    /// If `open_file_metadata` is provided, it verifies that the path resolves to the same file.
    pub fn validate_file_access(
        &self,
        file_path: &Path,
        root_dir: &Path,
        open_file: Option<&std::fs::File>,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if !self.allow_symlinks && std::fs::symlink_metadata(file_path)?.is_symlink() {
            warn!(
                "Security Block: Symlink detected at '{}'",
                file_path.display()
            );
            return Err(anyhow!("Safe Mode: Symlinks are disabled."));
        }

        // SECURITY: Hardlink Check
        // Canonicalize resolves symlinks but NOT hardlinks. A hardlink to /etc/passwd inside
        // the jail would pass the prefix check. We reject files with nlink > 1.
        #[cfg(unix)]
        if std::fs::metadata(file_path)?.nlink() > 1 {
            warn!(
                "Security Block: Hardlink detected at '{}'",
                file_path.display()
            );
            return Err(anyhow!("Safe Mode: Hardlinks are not allowed (nlink > 1)."));
        }

        // 1. Canonicalize: Resolves all symlinks and absolute paths
        let canonical_path = std::fs::canonicalize(file_path)
            .map_err(|e| anyhow!("Safe Mode: Failed to resolve path: {}", e))?;

        let canonical_root = std::fs::canonicalize(root_dir)
            .map_err(|e| anyhow!("Safe Mode: Failed to resolve root: {}", e))?;

        // 2. Prefix Check: Is the file inside the jail?
        if !canonical_path.starts_with(&canonical_root) {
            warn!(
                "Security Block: Path traversal/LFI attempt. Resolved '{}' is outside root '{}'",
                canonical_path.display(),
                canonical_root.display()
            );
            return Err(anyhow!(
                "Safe Mode Security Violation: Symlink target '{}' escapes the sandbox root.",
                canonical_path.display()
            ));
        }

        // 3. TOCTOU Check: Verify the validated path matches the open file
        if let Some(open_f) = open_file {
            let path_f = std::fs::File::open(&canonical_path)
                .map_err(|e| anyhow!("Safe Mode: Failed to open resolved path: {}", e))?;

            if !is_same_file(open_f, &path_f)? {
                warn!(
                    "Security Block: TOCTOU detected for '{}'",
                    file_path.display()
                );
                return Err(anyhow!(
                    "Safe Mode Security Violation: TOCTOU detected. The file at '{}' changed during processing.",
                    file_path.display()
                ));
            }
        }

        Ok(())
    }

    /// Sanitizes error messages to remove sensitive server paths.
    pub fn sanitize_error(&self, msg: String) -> String {
        if !self.enabled {
            return msg;
        }
        // Simple heuristic: remove absolute paths starting with / or C:\
        // This is a basic mitigation, not perfect.
        let re = regex::Regex::new(r"(/[a-zA-Z0-9_.-]+)+").unwrap();
        let sanitized = re.replace_all(&msg, "<path_redacted>");
        // Also handle Windows paths roughly
        let re_win = regex::Regex::new(r"[a-zA-Z]:\\[a-zA-Z0-9_.\-\\]+").unwrap();
        re_win
            .replace_all(&sanitized, "<path_redacted>")
            .to_string()
    }
}

fn is_same_file(f1: &std::fs::File, f2: &std::fs::File) -> Result<bool> {
    #[cfg(unix)]
    {
        let m1 = f1.metadata()?;
        let m2 = f2.metadata()?;
        Ok(m1.dev() == m2.dev() && m1.ino() == m2.ino())
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        unsafe {
            let h1 = f1.as_raw_handle() as HANDLE;
            let h2 = f2.as_raw_handle() as HANDLE;
            let mut i1: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            let mut i2: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();

            if GetFileInformationByHandle(h1, &mut i1) == 0
                || GetFileInformationByHandle(h2, &mut i2) == 0
            {
                return Ok(true);
            }
            Ok(i1.dwVolumeSerialNumber == i2.dwVolumeSerialNumber
                && i1.nFileIndexHigh == i2.nFileIndexHigh
                && i1.nFileIndexLow == i2.nFileIndexLow)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(true)
    }
}
