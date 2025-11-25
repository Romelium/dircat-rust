// src/web.rs

use crate::config::ConfigBuilder;
use crate::core_types::FileInfo;
use crate::progress::ProgressReporter;
use crate::security::SafeModeConfig;
use crate::{discover, MarkdownFormatter, OutputConfig};
use axum::extract::Query;
use axum::{
    extract::{ConnectInfo, Json, State},
    http::{header, Method, StatusCode, Uri},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Router,
};
use byte_unit::Byte;
use futures::stream::Stream;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tokio::sync::Semaphore;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
#[cfg(feature = "git")]
use walkdir::WalkDir;

#[derive(RustEmbed)]
#[folder = "assets/"]
struct Assets;

// --- Internal Types ---
#[derive(Clone, Debug)]
struct BroadcastMessage {
    request_id: String,
    payload: String,
}

// --- Shared State for SSE ---
struct AppState {
    tx: broadcast::Sender<BroadcastMessage>,
    security: SafeModeConfig,
    // Limit concurrent heavy operations (clones/scans) to prevent DoS
    concurrency_limit: Arc<Semaphore>,
}

// --- Web Progress Reporter ---
// Bridges dircat's synchronous progress trait to async SSE
struct WebProgressReporter {
    tx: broadcast::Sender<BroadcastMessage>,
    request_id: String,
}

impl WebProgressReporter {
    fn send(&self, payload: String) {
        let _ = self.tx.send(BroadcastMessage {
            request_id: self.request_id.clone(),
            payload,
        });
    }
}

impl ProgressReporter for WebProgressReporter {
    fn set_length(&self, _len: u64) { /* Simplified for web */
    }
    fn set_position(&self, _pos: u64) { /* Simplified for web */
    }
    fn set_message(&self, msg: String) {
        self.send(format!(r#"{{"type": "progress", "msg": "{}"}}"#, msg));
    }
    fn finish(&self) {
        self.send(r#"{"type": "done"}"#.to_string());
    }
    fn finish_with_message(&self, msg: String) {
        self.set_message(msg);
        self.finish();
    }
}

// --- Limited Writer for DoS Protection ---
struct LimitedWriter {
    inner: Vec<u8>,
    limit: u64,
    current: u64,
}

impl LimitedWriter {
    fn new(limit: u64) -> Self {
        Self {
            inner: Vec::new(),
            limit,
            current: 0,
        }
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.current + buf.len() as u64 > self.limit {
            return Err(std::io::Error::other("Output size limit exceeded"));
        }
        self.current += buf.len() as u64;
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limited_writer_stops_at_limit() {
        let mut writer = LimitedWriter::new(10); // 10 bytes limit

        // Write 5 bytes (OK)
        assert!(writer.write(b"12345").is_ok());
        assert_eq!(writer.current, 5);

        // Write 5 bytes (OK - hits limit exactly)
        assert!(writer.write(b"67890").is_ok());
        assert_eq!(writer.current, 10);

        // Write 1 byte (Fail)
        let res = writer.write(b"1");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Output size limit exceeded");
    }
}

// --- Request Structs ---
#[derive(Deserialize, Serialize, Debug)]
pub struct ScanRequest {
    pub input_path: String,
    pub max_size: Option<String>,
    pub extensions: Option<String>,
    pub exclude_extensions: Option<String>,
    pub no_gitignore: bool,
    pub no_lockfiles: bool,
    pub git_branch: Option<String>,
    pub git_depth: Option<u32>,
    pub path_regex: Option<String>,
    pub filename_regex: Option<String>,
    pub exclude_path_regex: Option<String>,
    pub ignore_patterns: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GenerateRequest {
    pub input_path: String,
    pub selected_files: Option<Vec<String>>,
    pub remove_comments: bool,
    pub remove_empty_lines: bool,
    pub include_binary: bool,
    pub line_numbers: bool,
    pub filename_only: bool,
    pub backticks: bool,
    pub git_branch: Option<String>,
    pub summary: bool,
    pub counts: bool,
    pub ticks: Option<u8>,
    pub process_last: Option<String>,
    pub output_file: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CopyRequest {
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub struct EventQuery {
    pub request_id: Option<String>,
}

#[derive(Serialize)]
pub struct FeaturesResponse {
    pub git: bool,
    pub clipboard: bool,
}

// --- Router Builder ---
fn build_router(state: Arc<AppState>) -> Router {
    let cors = if state.security.enabled {
        // Safe Mode: Restrict methods and headers, but allow origins for tool interoperability
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([header::CONTENT_TYPE])
            .allow_origin(Any)
    } else {
        // Standard Mode: Developer convenience (e.g., accessing from separate frontend dev server)
        CorsLayer::permissive()
    };

    Router::new()
        .route("/api/scan", post(scan_handler))
        .route("/api/generate", post(generate_handler))
        .route("/api/copy", post(copy_handler)) // Server-side copy
        .route("/api/events", get(sse_handler)) // Real-time events
        .route("/api/features", get(features_handler)) // Feature flags
        .fallback(static_handler)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub fn create_router() -> Router {
    let (tx, _rx) = broadcast::channel(100);
    // Default concurrency limit for tests/dev
    let state = Arc::new(AppState {
        tx,
        security: SafeModeConfig::default(),
        concurrency_limit: Arc::new(Semaphore::new(10)),
    });
    build_router(state)
}

pub fn create_router_with_config(security: SafeModeConfig) -> Router {
    let (tx, _rx) = broadcast::channel(100);
    let state = Arc::new(AppState {
        tx,
        security,
        concurrency_limit: Arc::new(Semaphore::new(10)),
    });
    build_router(state)
}

// --- Server Startup ---
pub async fn start_server(
    port: u16,
    open_browser: bool,
    safe: bool,
    allowed_domains: Option<String>,
    safe_max_file_size: Option<String>,
    safe_max_repo_size: Option<String>,
    safe_timeout: Option<u64>,
) -> anyhow::Result<()> {
    // 1. Build Configuration
    let mut security = if safe {
        SafeModeConfig::strict()
    } else {
        SafeModeConfig::default()
    };

    // 2. Apply Overrides
    if safe {
        if let Some(domains) = allowed_domains {
            if domains == "*" {
                security.allowed_domains = None; // Allow all domains (still HTTPS only)
            } else {
                security.allowed_domains =
                    Some(domains.split(',').map(|s| s.trim().to_string()).collect());
            }
        }
        if let Some(s) = safe_max_file_size {
            security.max_file_size = Byte::from_str(&s).map_err(|e| anyhow::anyhow!(e))?.as_u64();
        }
        if let Some(s) = safe_max_repo_size {
            security.max_repo_size = Byte::from_str(&s).map_err(|e| anyhow::anyhow!(e))?.as_u64();
        }

        // Ensure reasonable defaults if not set

        if let Some(t) = safe_timeout {
            security.request_timeout = std::time::Duration::from_secs(t);
        }
        println!("🔒 Safe Mode Enabled: HTTPS only, strict limits active.");
    }

    let (tx, _rx) = broadcast::channel(100);
    // In Safe Mode, limit concurrent heavy tasks to 5 to prevent resource exhaustion.
    // In Unsafe Mode, allow more (e.g., 20).
    let limit = if safe { 5 } else { 20 };
    let state = Arc::new(AppState {
        tx,
        security,
        concurrency_limit: Arc::new(Semaphore::new(limit)),
    });

    let app = build_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("🚀 Dircat Studio running at http://{}", addr);

    if open_browser {
        let _ = open::that(format!("http://127.0.0.1:{}", port));
    }

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

// --- Handlers ---

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

async fn features_handler() -> Json<FeaturesResponse> {
    Json(FeaturesResponse {
        git: cfg!(feature = "git"),
        clipboard: cfg!(feature = "clipboard"),
    })
}

// Server-Sent Events Handler
async fn sse_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EventQuery>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let filter_id = params.request_id.unwrap_or_else(|| "global".to_string());
    let rx = state.tx.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(move |msg| {
        // Filter messages server-side so clients only see their own events
        match msg {
            Ok(m) if m.request_id == filter_id => Some(Ok(Event::default().data(m.payload))),
            Ok(_) => None, // Skip messages for other clients
            Err(_) => Some(Ok(Event::default().event("error").data("Stream error"))),
        }
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

async fn scan_handler(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(req): Json<ScanRequest>,
) -> impl IntoResponse {
    let start_time = Instant::now();
    let addr = connect_info
        .map(|ci| ci.0)
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
    tracing::info!(src_ip = %addr, url = %req.input_path, "Received scan request");
    tracing::debug!(request = ?req, "Scan request details");

    // 1. Input Validation
    // Move validation to blocking task to prevent blocking async runtime with DNS
    let input_path = req.input_path.clone();
    let security = state.security.clone();
    let validation_result =
        tokio::task::spawn_blocking(move || security.validate_input(&input_path)).await;

    let resolved_ip = match validation_result {
        Ok(Ok(ip)) => ip,
        Ok(Err(e)) => {
            tracing::warn!(src_ip = %addr, url = %req.input_path, error = %e, "Scan request blocked by security validation");
            return (StatusCode::FORBIDDEN, e.to_string()).into_response();
        }
        Err(e) => {
            tracing::error!(src_ip = %addr, error = %e, "Validation task panicked");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Validation failed").into_response();
        }
    };

    // 2. Complexity & Sanitization Checks
    if let Err(e) = validate_scan_request_complexity(&state.security, &req) {
        tracing::warn!(src_ip = %addr, error = %e, "Scan request rejected due to complexity limits");
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    // 3. Concurrency Limit
    let _permit = match state.concurrency_limit.acquire().await {
        Ok(p) => p,
        Err(_) => {
            tracing::error!(src_ip = %addr, "Concurrency limit reached, dropping request");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Concurrency limit error").into_response();
        }
    };

    let tx = state.tx.clone();
    let request_id = req
        .request_id
        .clone()
        .unwrap_or_else(|| "global".to_string());
    let security = state.security.clone();

    let result = tokio::time::timeout(
        security.request_timeout,
        tokio::task::spawn_blocking(move || {
            let reporter = Arc::new(WebProgressReporter { tx, request_id });
            reporter.set_message("Configuring...".to_string());

            let extensions = parse_ext_string(req.extensions);
            let exclude_extensions = parse_ext_string(req.exclude_extensions);
            let path_regex = parse_multiline_string(req.path_regex);
            let filename_regex = parse_multiline_string(req.filename_regex);
            let exclude_path_regex = parse_multiline_string(req.exclude_path_regex);
            let ignore_patterns = parse_multiline_string(req.ignore_patterns);

            let mut builder = ConfigBuilder::new()
                .input_path(req.input_path)
                .no_gitignore(req.no_gitignore)
                .no_lockfiles(req.no_lockfiles);

            if let Some(size) = req.max_size.filter(|s| !s.trim().is_empty()) {
                builder = builder.max_size(size);
            }

            if let Some(exts) = extensions {
                builder = builder.extensions(exts);
            }
            if let Some(ex_exts) = exclude_extensions {
                builder = builder.exclude_extensions(ex_exts);
            }

            if let Some(re) = path_regex {
                builder = builder.path_regex(re);
            }
            if let Some(re) = filename_regex {
                builder = builder.filename_regex(re);
            }
            if let Some(re) = exclude_path_regex {
                builder = builder.exclude_path_regex(re);
            }
            if let Some(pats) = ignore_patterns {
                builder = builder.ignore_patterns(pats);
            }

            if let Some(branch) = req.git_branch {
                #[cfg(feature = "git")]
                {
                    builder = builder.git_branch(branch);
                    if let Some(depth) = req.git_depth {
                        builder = builder.git_depth(depth);
                    }
                }
                #[cfg(not(feature = "git"))]
                {
                    let _ = branch; // Suppress unused variable warning
                }
            }

            let config = builder.build()?;

            reporter.set_message("Resolving input (Cloning if needed)...".to_string());

            // Safe Mode: Use persistent safe cache with pruning
            #[cfg(feature = "git")]
            let (cache_path_override, _temp_dir_guard) = if security.enabled {
                // Allow config override OR env var override for testing isolation
                let safe_cache_dir = security.cache_dir.clone()
                    .or_else(|| std::env::var("DIRCAT_SAFE_CACHE_DIR").ok().map(std::path::PathBuf::from))
                    .unwrap_or_else(|| std::env::temp_dir().join("dircat-safe-cache"));
                if let Err(e) = std::fs::create_dir_all(&safe_cache_dir) {
                    return Err(anyhow::anyhow!("Failed to create safe cache dir: {}", e));
                }

                // Prune cache to ensure we don't exhaust disk space.
                // We allow the total cache to be 5x the single repo limit.
                prune_cache(&safe_cache_dir, security.max_repo_size * 5);

                (
                    Some(safe_cache_dir.to_string_lossy().to_string()),
                    Option::<()>::None,
                )
            } else {
                (config.git_cache_path.clone(), Option::<()>::None)
            };

            #[cfg(feature = "git")]
            let resolved = crate::config::resolve_input(
                &config.input_path,
                &config.git_branch,
                config.git_depth, // Use the depth from config (populated by builder)
                &cache_path_override, // Use override if present, else config value
                Some(reporter.clone()), // Pass reporter to git logic
                resolved_ip,
            )?;

            #[cfg(not(feature = "git"))]
            let resolved = crate::config::resolve_input(
                &config.input_path,
                &None,
                None,
                &None,
                Some(reporter.clone()),
                resolved_ip,
            )?;

            // 3. Post-Clone Size Check (DoS Protection)
            #[cfg(feature = "git")]
            if security.enabled {
                let size = get_dir_size(&resolved.path).unwrap_or(0);
                if size > security.max_repo_size {
                    tracing::warn!(src_ip = %addr, repo_size = size, limit = security.max_repo_size, "Repository exceeded size limit");
                    // Nuke the repo immediately
                    let _ = std::fs::remove_dir_all(&resolved.path);
                    return Err(anyhow::anyhow!(
                        "Safe Mode: Repository size ({}) exceeds limit ({})",
                        size,
                        security.max_repo_size
                    ));
                }
            }

            reporter.set_message("Scanning directory...".to_string());

            // Inject Safe Mode into Discovery Config
            let mut discovery_config = config.discovery.clone();
            discovery_config.safe_mode = security.enabled;
            discovery_config.max_file_count = Some(security.max_file_count);

            let token = crate::cancellation::CancellationToken::new();
            let files: Vec<FileInfo> = discover(&discovery_config, &resolved, &token)?.collect();

            reporter.finish();
            Ok::<Vec<FileInfo>, anyhow::Error>(files)
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok(files))) => {
            tracing::info!(src_ip = %addr, duration = ?start_time.elapsed(), file_count = files.len(), "Scan completed successfully");
            (StatusCode::OK, axum::Json(files)).into_response()
        }
        Ok(Ok(Err(e))) => {
            tracing::error!(src_ip = %addr, error = %e, "Scan logic failed");
            (
                StatusCode::BAD_REQUEST,
                state.security.sanitize_error(e.to_string()),
            )
                .into_response()
        }
        Ok(Err(_join_err)) => {
            tracing::error!(src_ip = %addr, "Scan task panicked");
            (StatusCode::INTERNAL_SERVER_ERROR, "Task failed/panicked").into_response()
        }
        Err(_timeout) => {
            tracing::warn!(src_ip = %addr, duration = ?start_time.elapsed(), "Scan operation timed out");
            (StatusCode::REQUEST_TIMEOUT, "Operation timed out").into_response()
        }
    }
}

async fn generate_handler(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(req): Json<GenerateRequest>,
) -> impl IntoResponse {
    let start_time = Instant::now();
    let addr = connect_info
        .map(|ci| ci.0)
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
    tracing::info!(src_ip = %addr, url = %req.input_path, "Received generate request");
    tracing::debug!(request = ?req, "Generate request details");

    // 1. Input Validation
    // Move validation to blocking task to prevent blocking async runtime with DNS
    let input_path = req.input_path.clone();
    let security = state.security.clone();
    let validation_result =
        tokio::task::spawn_blocking(move || security.validate_input(&input_path)).await;

    let resolved_ip = match validation_result {
        Ok(Ok(ip)) => ip,
        Ok(Err(e)) => {
            tracing::warn!(src_ip = %addr, url = %req.input_path, error = %e, "Generate request blocked by security validation");
            return (StatusCode::FORBIDDEN, e.to_string()).into_response();
        }
        Err(e) => {
            tracing::error!(src_ip = %addr, error = %e, "Validation task panicked");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Validation failed").into_response();
        }
    };

    // 2. Block Output File
    if state.security.enabled && req.output_file.is_some() {
        tracing::warn!(src_ip = %addr, "Blocked attempt to write output file in Safe Mode");
        return (
            StatusCode::FORBIDDEN,
            "Safe Mode: Server-side file writing is disabled.",
        )
            .into_response();
    }

    // 3. Sanitize Selection (Path Traversal)
    if let Some(files) = &req.selected_files {
        for file in files {
            if file.contains("..") || file.starts_with('/') || file.contains('\\') {
                tracing::warn!(src_ip = %addr, file = %file, "Blocked path traversal in selected_files");
                return (
                    StatusCode::BAD_REQUEST,
                    "Safe Mode: Invalid file path in selection.",
                )
                    .into_response();
            }
        }
    }

    // 4. Complexity & Sanitization Checks
    if let Err(e) = validate_generate_request_complexity(&state.security, &req) {
        tracing::warn!(src_ip = %addr, error = %e, "Generate request rejected due to complexity limits");
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

    // 5. Concurrency Limit
    let _permit = match state.concurrency_limit.acquire().await {
        Ok(p) => p,
        Err(_) => {
            tracing::error!(src_ip = %addr, "Concurrency limit reached");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Concurrency limit error").into_response();
        }
    };

    let tx = state.tx.clone();
    let request_id = req
        .request_id
        .clone()
        .unwrap_or_else(|| "global".to_string());
    let security = state.security.clone();

    let result = tokio::time::timeout(
        security.request_timeout,
        tokio::task::spawn_blocking(move || {
            let reporter = Arc::new(WebProgressReporter { tx, request_id });
            reporter.set_message("Processing files...".to_string());

            let mut builder = ConfigBuilder::new()
                .input_path(req.input_path)
                .remove_comments(req.remove_comments)
                .remove_empty_lines(req.remove_empty_lines)
                .include_binary(req.include_binary)
                .line_numbers(req.line_numbers)
                .filename_only(req.filename_only)
                .backticks(req.backticks)
                .summary(req.summary)
                .counts(req.counts)
                .ticks(req.ticks.unwrap_or(3))
                .output_file(req.output_file.clone().unwrap_or_default());

            if let Some(branch) = req.git_branch {
                #[cfg(feature = "git")]
                {
                    builder = builder.git_branch(branch);
                }
                #[cfg(not(feature = "git"))]
                {
                    let _ = branch;
                }
            }
            if let Some(selected) = req.selected_files {
                let escaped_selected: Vec<String> = selected
                    .into_iter()
                    .map(|path| {
                        // 1. Normalize/Escape Backslashes
                        #[cfg(windows)]
                        let path = path.replace('\\', "/");
                        #[cfg(not(windows))]
                        let path = path.replace('\\', "\\\\");

                        // 2. Escape Glob Specials (*, ?, [, ])
                        glob::Pattern::escape(&path)
                    })
                    .collect();
                builder = builder.only(escaped_selected);
            } else if let Some(last_pats) = parse_multiline_string(req.process_last) {
                // Only apply process_last if specific files weren't selected (to avoid conflict)
                // This mimics CLI behavior where --only conflicts with --last
                builder = builder.process_last(last_pats);
            }

            let mut config = builder.build()?;

            // Inject Security Config
            config.discovery.safe_mode = security.enabled;
            config.processing.security = Some(security.clone());

            let token = crate::cancellation::CancellationToken::new();

            // We need to manually resolve here to pass the IP.
            #[cfg(feature = "git")]
            let resolved = crate::config::resolve_input(
                &config.input_path,
                &config.git_branch,
                config.git_depth,
                &config.git_cache_path,
                Some(reporter.clone()),
                resolved_ip,
            )?;
            #[cfg(not(feature = "git"))]
            let resolved = crate::config::resolve_input(
                &config.input_path,
                &None,
                None,
                &None,
                Some(reporter.clone()),
                resolved_ip,
            )?;

            let discovered_iter = discover(&config.discovery, &resolved, &token)?;
            let dircat_result = crate::DircatResult {
                files: crate::processing::process_files(
                    discovered_iter,
                    &config.processing,
                    &token,
                    Some(&resolved.path),
                )
                .collect::<Result<Vec<_>, _>>()?,
            };

            reporter.set_message("Formatting output...".to_string());
            let mut buffer = LimitedWriter::new(security.max_output_size);
            let output_opts = OutputConfig::from(&config);
            let formatter = MarkdownFormatter;

            // If an output file was specified, we also write to it on the server side
            if let Some(path) = req.output_file.filter(|p| !p.is_empty()) {
                let mut file_writer = std::fs::File::create(path)?;
                dircat_result.format_with(&formatter, &output_opts, &mut file_writer)?;
            }

            dircat_result.format_with(&formatter, &output_opts, &mut buffer)?;

            reporter.finish();
            Ok::<String, anyhow::Error>(String::from_utf8(buffer.inner)?)
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok(output))) => {
            tracing::info!(src_ip = %addr, duration = ?start_time.elapsed(), output_bytes = output.len(), "Generate completed successfully");
            (StatusCode::OK, output).into_response()
        }
        Ok(Ok(Err(e))) => {
            tracing::error!(src_ip = %addr, error = %e, "Generate logic failed");
            (
                StatusCode::BAD_REQUEST,
                state.security.sanitize_error(e.to_string()),
            )
                .into_response()
        }
        Ok(Err(_join_err)) => {
            tracing::error!(src_ip = %addr, "Generate task panicked");
            (StatusCode::INTERNAL_SERVER_ERROR, "Task failed").into_response()
        }
        Err(_timeout) => {
            tracing::warn!(src_ip = %addr, duration = ?start_time.elapsed(), "Generate operation timed out");
            (StatusCode::REQUEST_TIMEOUT, "Operation timed out").into_response()
        }
    }
}

// Server-side clipboard copy
async fn copy_handler(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(_req): Json<CopyRequest>,
) -> impl IntoResponse {
    let addr = connect_info
        .map(|ci| ci.0)
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
    tracing::info!(src_ip = %addr, "Received clipboard copy request");

    if state.security.enabled && !state.security.allow_clipboard {
        tracing::warn!(src_ip = %addr, "Clipboard access blocked by Safe Mode");
        return (
            StatusCode::FORBIDDEN,
            "Safe Mode: Clipboard access is disabled.",
        )
            .into_response();
    }
    // We use a blocking task because arboard might block
    let result = tokio::task::spawn_blocking(move || {
        #[cfg(feature = "clipboard")]
        {
            use arboard::Clipboard;
            let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
            clipboard
                .set_text(_req.content)
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }
        #[cfg(not(feature = "clipboard"))]
        {
            Err::<(), String>("Clipboard feature not enabled".to_string())
        }
    })
    .await;

    match result {
        Ok(Ok(_)) => {
            tracing::info!(src_ip = %addr, "Clipboard copy successful");
            StatusCode::OK.into_response()
        }
        Ok(Err(e)) => {
            tracing::error!(src_ip = %addr, error = %e, "Clipboard copy failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn parse_ext_string(input: Option<String>) -> Option<Vec<String>> {
    input
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.split_whitespace().map(|s| s.trim().to_string()).collect())
}

fn parse_multiline_string(input: Option<String>) -> Option<Vec<String>> {
    input.filter(|s| !s.trim().is_empty()).map(|s| {
        s.lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

// Helper for repo size
#[cfg(feature = "git")]
fn get_dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total_size = 0;
    for entry in WalkDir::new(path) {
        let entry = entry?;
        if entry.metadata()?.is_file() {
            total_size += entry.metadata()?.len();
        }
    }
    Ok(total_size)
}

/// Prunes the cache directory to keep total size under limit (LRU strategy).
#[cfg(feature = "git")]
fn prune_cache(cache_root: &std::path::Path, max_total_size: u64) {
    let mut entries = Vec::new();
    let mut total_size = 0;

    // 1. Collect all repo directories
    for e in WalkDir::new(cache_root)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .flatten()
    {
        if e.file_type().is_dir() {
            let size = get_dir_size(e.path()).unwrap_or(0);
            let mtime = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            entries.push((e.path().to_path_buf(), size, mtime));
            total_size += size;
        }
    }

    // 2. If over limit, delete oldest accessed
    if total_size > max_total_size {
        // Sort by modification time (oldest first)
        entries.sort_by_key(|k| k.2);

        for (path, size, _) in entries {
            if total_size <= max_total_size {
                break;
            }

            // Attempt to remove. If locked by another process (via fs2 in clone.rs),
            // remove_dir_all might fail on some OSs or we just skip it.
            if std::fs::remove_dir_all(&path).is_ok() {
                total_size = total_size.saturating_sub(size);
                let _ = std::fs::remove_file(path.with_extension("lock"));
            }
        }
    }
}

#[cfg(test)]
mod prune_tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    #[cfg(feature = "git")]
    fn test_prune_cache_eviction() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        // Create 3 directories with dummy content
        let dir1 = root.join("repo1");
        let dir2 = root.join("repo2");
        let dir3 = root.join("repo3");

        fs::create_dir(&dir1).unwrap();
        fs::create_dir(&dir2).unwrap();
        fs::create_dir(&dir3).unwrap();

        // Write 100 bytes to each
        let content = vec![0u8; 100];
        fs::write(dir1.join("data"), &content).unwrap();
        fs::write(dir2.join("data"), &content).unwrap();
        fs::write(dir3.join("data"), &content).unwrap();

        // Set mtimes to ensure order: dir1 (oldest) < dir2 < dir3 (newest)
        // We need significant delays or manual mtime setting.
        // Using filetime crate which is now a dependency.
        let t1 = filetime::FileTime::from_unix_time(1000, 0);
        let t2 = filetime::FileTime::from_unix_time(2000, 0);
        let t3 = filetime::FileTime::from_unix_time(3000, 0);

        filetime::set_file_times(&dir1, t1, t1).unwrap();
        filetime::set_file_times(&dir2, t2, t2).unwrap();
        filetime::set_file_times(&dir3, t3, t3).unwrap();

        // Total size is ~300 bytes. Limit to 250 bytes.
        // Should evict repo1 (oldest).
        prune_cache(root, 250);

        assert!(!dir1.exists(), "Oldest repo1 should be evicted");
        assert!(dir2.exists(), "Repo2 should remain");
        assert!(dir3.exists(), "Newest repo3 should remain");
    }
}

// --- Validation Helpers ---

fn validate_string_length(name: &str, val: &Option<String>, max_len: usize) -> anyhow::Result<()> {
    if let Some(s) = val {
        if s.len() > max_len {
            return Err(anyhow::anyhow!(
                "Safe Mode: '{}' exceeds maximum length of {} characters.",
                name,
                max_len
            ));
        }
    }
    Ok(())
}

fn validate_git_ref(git_ref: &Option<String>) -> anyhow::Result<()> {
    if let Some(r) = git_ref {
        // Allow alphanumeric, -, _, ., /
        // Disallow control characters, spaces, or dangerous shell chars
        if !r
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
        {
            return Err(anyhow::anyhow!(
                "Safe Mode: Invalid characters in git branch/tag name."
            ));
        }
    }
    Ok(())
}

fn validate_scan_request_complexity(sec: &SafeModeConfig, req: &ScanRequest) -> anyhow::Result<()> {
    if !sec.enabled {
        return Ok(());
    }

    // Limit Regex/Glob lengths to prevent ReDoS or excessive parsing overhead
    const MAX_REGEX_LEN: usize = 1000;

    validate_string_length("extensions", &req.extensions, MAX_REGEX_LEN)?;
    validate_string_length("exclude_extensions", &req.exclude_extensions, MAX_REGEX_LEN)?;
    validate_string_length("path_regex", &req.path_regex, MAX_REGEX_LEN)?;
    validate_string_length("filename_regex", &req.filename_regex, MAX_REGEX_LEN)?;
    validate_string_length("exclude_path_regex", &req.exclude_path_regex, MAX_REGEX_LEN)?;
    validate_string_length("ignore_patterns", &req.ignore_patterns, MAX_REGEX_LEN)?;

    // ReDoS Protection for Globs
    if let Some(pats) = &req.ignore_patterns {
        if pats.split('\n').any(|line| line.matches('*').count() > 4) {
            return Err(anyhow::anyhow!(
                "Safe Mode: Ignore pattern contains too many wildcards (ReDoS prevention)."
            ));
        }
    }

    validate_string_length("git_branch", &req.git_branch, 255)?;
    validate_git_ref(&req.git_branch)?;

    Ok(())
}

fn validate_generate_request_complexity(
    sec: &SafeModeConfig,
    req: &GenerateRequest,
) -> anyhow::Result<()> {
    if !sec.enabled {
        return Ok(());
    }

    validate_string_length("git_branch", &req.git_branch, 255)?;
    validate_git_ref(&req.git_branch)?;
    validate_string_length("process_last", &req.process_last, 1000)?;

    Ok(())
}
