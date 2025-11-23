// src/web.rs

use crate::config::ConfigBuilder;
use crate::core_types::FileInfo;
use crate::progress::ProgressReporter;
use crate::{discover, execute, MarkdownFormatter, OutputConfig};
use axum::extract::Query;
use axum::{
    extract::{Json, State},
    http::{header, StatusCode, Uri},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Router,
};
use futures::stream::Stream;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

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

// --- Server Startup ---
pub fn create_router() -> Router {
    let (tx, _rx) = broadcast::channel(100);
    let state = Arc::new(AppState { tx });

    Router::new()
        .route("/api/scan", post(scan_handler))
        .route("/api/generate", post(generate_handler))
        .route("/api/copy", post(copy_handler)) // Server-side copy
        .route("/api/events", get(sse_handler)) // Real-time events
        .route("/api/features", get(features_handler)) // Feature flags
        .fallback(static_handler)
        .with_state(state)
}

pub async fn start_server(port: u16, open_browser: bool) -> anyhow::Result<()> {
    let app = create_router();

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let url = format!("http://{}", addr);

    println!("✨ Dircat Studio running at {}", url);

    if open_browser {
        let _ = open::that(&url);
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
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
    Json(req): Json<ScanRequest>,
) -> impl IntoResponse {
    let tx = state.tx.clone();
    let request_id = req
        .request_id
        .clone()
        .unwrap_or_else(|| "global".to_string());

    let result = tokio::task::spawn_blocking(move || {
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

        #[cfg(feature = "git")]
        let resolved = crate::config::resolve_input(
            &config.input_path,
            &config.git_branch,
            config.git_depth, // Use the depth from config (populated by builder)
            &config.git_cache_path,
            Some(reporter.clone()), // Pass reporter to git logic
        )?;

        #[cfg(not(feature = "git"))]
        let resolved = crate::config::resolve_input(
            &config.input_path,
            &None,
            None,
            &None,
            Some(reporter.clone()),
        )?;

        reporter.set_message("Scanning directory...".to_string());
        let token = crate::cancellation::CancellationToken::new();
        let files: Vec<FileInfo> = discover(&config.discovery, &resolved, &token)?.collect();

        reporter.finish();
        Ok::<Vec<FileInfo>, anyhow::Error>(files)
    })
    .await;

    match result {
        Ok(Ok(files)) => (StatusCode::OK, axum::Json(files)).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn generate_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GenerateRequest>,
) -> impl IntoResponse {
    let tx = state.tx.clone();
    let request_id = req
        .request_id
        .clone()
        .unwrap_or_else(|| "global".to_string());

    let result = tokio::task::spawn_blocking(move || {
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

                    // 2. Escape Glob Specials (*, ?, [, ]) and '!' (for gitignore)
                    glob::Pattern::escape(&path).replace('!', "\\!")
                })
                .collect();
            builder = builder.only(escaped_selected);
        } else if let Some(last_pats) = parse_multiline_string(req.process_last) {
            // Only apply process_last if specific files weren't selected (to avoid conflict)
            // This mimics CLI behavior where --only conflicts with --last
            builder = builder.process_last(last_pats);
        }

        let config = builder.build()?;
        let token = crate::cancellation::CancellationToken::new();

        let dircat_result = execute(&config, &token, Some(reporter.clone()))?;

        reporter.set_message("Formatting output...".to_string());
        let mut buffer = Vec::new();
        let output_opts = OutputConfig::from(&config);
        let formatter = MarkdownFormatter;

        // If an output file was specified, we also write to it on the server side
        if let Some(path) = req.output_file.filter(|p| !p.is_empty()) {
            let mut file_writer = std::fs::File::create(path)?;
            dircat_result.format_with(&formatter, &output_opts, &mut file_writer)?;
        }

        dircat_result.format_with(&formatter, &output_opts, &mut buffer)?;

        reporter.finish();
        Ok::<String, anyhow::Error>(String::from_utf8(buffer)?)
    })
    .await;

    match result {
        Ok(Ok(output)) => (StatusCode::OK, output).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// Server-side clipboard copy
async fn copy_handler(Json(_req): Json<CopyRequest>) -> impl IntoResponse {
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
        Ok(Ok(_)) => StatusCode::OK.into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
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
