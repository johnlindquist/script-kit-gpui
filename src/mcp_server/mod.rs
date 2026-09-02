//! MCP Server Foundation
//!
//! Provides an HTTP server for MCP (Model Context Protocol) integration.
//! Features:
//! - HTTP server on localhost:43210
//! - Bearer token authentication from ~/.scriptkit/agent-token
//! - Health endpoint at GET /health
//! - Discovery file at ~/.scriptkit/server.json

// Allow dead code - ServerHandle methods provide full lifecycle API for future use
#![allow(dead_code)]

// --- merged from part_000.rs ---
use crate::logging;
use crate::mcp_kit_tools::McpKitRuntimeBridge;
use crate::mcp_notes_tools::SharedNotesMutationBridge;
use crate::mcp_protocol::{self, JsonRpcResponse, McpRuntimeContext};
use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use tracing::{debug, error, info, warn};
/// Default port for the MCP server
pub const DEFAULT_PORT: u16 = 43210;
/// MCP Server version for discovery
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) type SharedComputerRuntime =
    Arc<dyn crate::computer_use::runtime_bridge::ComputerUseRuntimeBridge + Send + Sync + 'static>;
/// Server capabilities advertised in discovery
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerCapabilities {
    pub scripts: bool,
    pub prompts: bool,
    pub tools: bool,
}
impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            scripts: true,
            prompts: true,
            tools: true,
        }
    }
}
/// Discovery file structure written to ~/.scriptkit/server.json
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveryInfo {
    pub url: String,
    pub token: String,
    pub version: String,
    pub capabilities: ServerCapabilities,
}
/// MCP HTTP Server
///
/// Lightweight HTTP server for MCP protocol communication.
/// Uses std::net for simplicity (no async runtime required).
pub struct McpServer {
    port: u16,
    token: String,
    running: Arc<AtomicBool>,
    kit_path: PathBuf,
    runtime_context: McpRuntimeContext,
}
impl McpServer {
    /// Create a new MCP server instance
    ///
    /// # Arguments
    /// * `port` - Port to listen on (default: 43210)
    /// * `kit_path` - Path to ~/.scriptkit directory
    pub fn new(port: u16, kit_path: PathBuf) -> Result<Self> {
        let token = Self::load_or_create_token(&kit_path)?;

        Ok(Self {
            port,
            token,
            running: Arc::new(AtomicBool::new(false)),
            kit_path,
            runtime_context: McpRuntimeContext::default(),
        })
    }

    pub fn new_with_runtime_context(
        port: u16,
        kit_path: PathBuf,
        runtime_context: McpRuntimeContext,
    ) -> Result<Self> {
        let token = Self::load_or_create_token(&kit_path)?;

        Ok(Self {
            port,
            token,
            running: Arc::new(AtomicBool::new(false)),
            kit_path,
            runtime_context,
        })
    }

    /// Create server with default settings
    pub fn with_defaults() -> Result<Self> {
        let kit_path = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".scriptkit");

        let port = std::env::var("MCP_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);

        Self::new(port, kit_path)
    }

    pub fn with_defaults_and_runtime_context(runtime_context: McpRuntimeContext) -> Result<Self> {
        let kit_path = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".scriptkit");

        let port = std::env::var("MCP_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);

        Self::new_with_runtime_context(port, kit_path, runtime_context)
    }

    pub(crate) fn with_computer_runtime(mut self, runtime: SharedComputerRuntime) -> Self {
        self.runtime_context.computer = Some(runtime);
        self
    }

    pub(crate) fn with_notes_mutation_bridge(mut self, bridge: SharedNotesMutationBridge) -> Self {
        self.runtime_context.notes_mutation_bridge = Some(bridge);
        self
    }

    pub(crate) fn with_kit_runtime_bridge(
        mut self,
        bridge: Arc<dyn McpKitRuntimeBridge + Send + Sync>,
    ) -> Self {
        self.runtime_context.kit_runtime_bridge = Some(bridge);
        self
    }

    /// Load existing token or create a new one
    fn load_or_create_token(kit_path: &PathBuf) -> Result<String> {
        let token_path = kit_path.join("agent-token");

        if token_path.exists() {
            let token = fs::read_to_string(&token_path)
                .context("Failed to read agent-token file")?
                .trim()
                .to_string();

            if !token.is_empty() {
                info!("Loaded existing agent token from {:?}", token_path);
                return Ok(token);
            }
        }

        // Generate new token
        let token = uuid::Uuid::new_v4().to_string();

        // Ensure kit directory exists
        fs::create_dir_all(kit_path).context("Failed to create .kit directory")?;

        // Write token file with restrictive permissions (0o600 - owner read/write only)
        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;

            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&token_path)
                .and_then(|mut file| {
                    use std::io::Write;
                    file.write_all(token.as_bytes())
                })
                .context("Failed to write agent-token file")?;
        }

        #[cfg(not(unix))]
        {
            fs::write(&token_path, &token).context("Failed to write agent-token file")?;
        }

        info!("Generated new agent token at {:?}", token_path);
        Ok(token)
    }

    /// Get the authentication token
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Get the server URL
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Write discovery file to ~/.scriptkit/server.json
    fn write_discovery_file(&self) -> Result<()> {
        let discovery = DiscoveryInfo {
            url: self.url(),
            token: self.token.clone(),
            version: VERSION.to_string(),
            capabilities: ServerCapabilities::default(),
        };

        let discovery_path = self.kit_path.join("server.json");
        let json = serde_json::to_string_pretty(&discovery)
            .context("Failed to serialize discovery info")?;

        // Write discovery file with restrictive permissions (0o600 - contains token)
        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;

            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&discovery_path)
                .and_then(|mut file| {
                    use std::io::Write;
                    file.write_all(json.as_bytes())
                })
                .context("Failed to write server.json")?;
        }

        #[cfg(not(unix))]
        {
            fs::write(&discovery_path, json).context("Failed to write server.json")?;
        }

        info!("Wrote discovery file to {:?}", discovery_path);
        Ok(())
    }

    /// Remove discovery file on shutdown
    fn remove_discovery_file(&self) {
        let discovery_path = self.kit_path.join("server.json");
        if discovery_path.exists() {
            if let Err(e) = fs::remove_file(&discovery_path) {
                warn!("Failed to remove discovery file: {}", e);
            } else {
                debug!("Removed discovery file");
            }
        }
    }

    /// Start the HTTP server in a background thread
    ///
    /// Returns a handle that can be used to stop the server.
    pub fn start(&self) -> Result<ServerHandle> {
        if self.is_running() {
            anyhow::bail!("Server is already running");
        }

        // Write discovery file before starting
        self.write_discovery_file()?;

        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))
            .with_context(|| format!("Failed to bind to port {}", self.port))?;

        // Set non-blocking for graceful shutdown
        listener
            .set_nonblocking(true)
            .context("Failed to set non-blocking mode")?;

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let token = self.token.clone();
        let kit_path = self.kit_path.clone();
        let port = self.port;
        let runtime_context = self.runtime_context.clone();

        let handle = thread::spawn(move || {
            info!("MCP server started on port {}", port);

            while running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        debug!("Connection from {}", addr);
                        let token = token.clone();
                        let kit_path = kit_path.clone();
                        let runtime_context = runtime_context.clone();
                        thread::spawn(move || {
                            if let Err(e) =
                                handle_connection(stream, &token, &kit_path, runtime_context)
                            {
                                error!("Error handling connection: {}", e);
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No connection available, sleep before next poll.
                        // 100ms is responsive enough for new connections while reducing
                        // CPU wakeups from 100/sec to 10/sec (was 10ms = 100% CPU spin risk)
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => {
                        error!("Accept error: {}", e);
                    }
                }
            }

            // Cleanup on shutdown
            let discovery_path = kit_path.join("server.json");
            if discovery_path.exists() {
                let _ = fs::remove_file(&discovery_path);
            }

            info!("MCP server stopped");
        });

        Ok(ServerHandle {
            running: self.running.clone(),
            thread: Some(handle),
        })
    }

    /// Stop the server
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.remove_discovery_file();
    }
}
/// Handle for controlling the running server
pub struct ServerHandle {
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

static RETAINED_SERVER_HANDLE: OnceLock<Mutex<Option<ServerHandle>>> = OnceLock::new();

pub fn retain_server_handle(handle: ServerHandle) {
    let slot = RETAINED_SERVER_HANDLE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(handle);
    }
}

impl ServerHandle {
    /// Stop the server and wait for it to finish
    pub fn stop(mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    /// Check if server is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}
impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Note: we don't join here to avoid blocking on drop
    }
}
/// Handle a single HTTP connection
fn handle_connection(
    mut stream: TcpStream,
    expected_token: &str,
    kit_path: &std::path::Path,
    mut runtime_context: McpRuntimeContext,
) -> Result<()> {
    stream
        .set_nonblocking(false)
        .context("Failed to set accepted MCP connection to blocking mode")?;
    let mut reader = BufReader::new(stream.try_clone()?);

    // Read request line
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let request_line = request_line.trim();

    // Parse method and path
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return send_response(&mut stream, 400, "Bad Request", "Invalid request line");
    }

    let method = parts[0];
    let path = parts[1];
    let path_slug = path.trim_matches('/').replace('/', "_");
    let correlation_id = format!(
        "mcp:{}:{}:{}",
        method.to_lowercase(),
        if path_slug.is_empty() {
            "root".to_string()
        } else {
            path_slug
        },
        uuid::Uuid::new_v4()
    );
    let _request_guard = logging::set_correlation_id(correlation_id.clone());
    debug!(
        category = "MCP",
        event_type = "mcp_http_request",
        method = method,
        path = path,
        correlation_id = %correlation_id,
        "Received MCP HTTP request"
    );

    // Read headers
    let mut headers = std::collections::HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_lowercase(), value.trim().to_string());
        }
    }

    // Check authorization for non-health endpoints
    let mut token_scopes = mcp_protocol::legacy_all_scopes();
    if path != "/health" {
        let auth_scopes = headers
            .get("authorization")
            .and_then(|auth| auth.strip_prefix("Bearer "))
            .and_then(|token| authorize_bearer_token(token, expected_token, kit_path).ok())
            .flatten();

        let Some(scopes) = auth_scopes else {
            return send_response(&mut stream, 401, "Unauthorized", "Invalid or missing token");
        };
        token_scopes = scopes;
    }
    runtime_context.token_scopes = token_scopes;
    runtime_context.trace_id = correlation_id.clone();

    // Route request
    match (method, path) {
        ("GET", "/health") => send_response(&mut stream, 200, "OK", r#"{"status":"healthy"}"#),
        ("GET", "/") => {
            let info = serde_json::json!({
                "name": "Script Kit MCP Server",
                "version": VERSION,
                "capabilities": ServerCapabilities::default(),
            });
            send_response(&mut stream, 200, "OK", &info.to_string())
        }
        ("POST", "/rpc") => {
            // Handle JSON-RPC request
            handle_rpc_request(&mut reader, &mut stream, &headers, runtime_context)
        }
        _ => send_response(&mut stream, 404, "Not Found", "Endpoint not found"),
    }
}
/// Handle a JSON-RPC request on the /rpc endpoint
fn handle_rpc_request(
    reader: &mut BufReader<TcpStream>,
    stream: &mut TcpStream,
    headers: &std::collections::HashMap<String, String>,
    runtime_context: McpRuntimeContext,
) -> Result<()> {
    // Get Content-Length
    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if content_length == 0 {
        let response = JsonRpcResponse::error(
            serde_json::Value::Null,
            mcp_protocol::error_codes::INVALID_REQUEST,
            "Missing or invalid Content-Length header",
        );
        let body = serde_json::to_string(&response)?;
        return send_response(stream, 400, "Bad Request", &body);
    }

    // Read request body
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    let body_str = String::from_utf8_lossy(&body);
    let body_summary = logging::summarize_payload(&body_str);
    debug!(
        category = "MCP",
        event_type = "mcp_rpc_request_body",
        content_length = content_length,
        payload_summary = %body_summary,
        "Received MCP RPC request body"
    );

    // Load scripts and scriptlets for context-aware responses
    // This allows resources/read and tools/list to return actual data
    let scripts = crate::scripts::read_scripts()?;
    let scriptlets = crate::scripts::load_scriptlets()?.into_scriptlets();

    // Parse and handle request with full context
    let response = match mcp_protocol::parse_request(&body_str) {
        Ok(request) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(mcp_protocol::handle_request_with_runtime_context(
                request,
                &scripts,
                &scriptlets,
                None,
                Some(&runtime_context),
            ))
        }
        Err(e) => e,
    };

    let response_body = serde_json::to_string(&response)?;
    send_response(stream, 200, "OK", &response_body)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentTokenConfig {
    token_hash: String,
    scopes: Vec<String>,
}

fn authorize_bearer_token(
    token: &str,
    legacy_token: &str,
    kit_path: &std::path::Path,
) -> Result<Option<Vec<String>>> {
    if token == legacy_token {
        return Ok(Some(mcp_protocol::legacy_all_scopes()));
    }

    let agents_dir = kit_path.join("agents");
    if !agents_dir.is_dir() {
        return Ok(None);
    }

    let token_hash = hash_token(token);
    for entry in fs::read_dir(&agents_dir)
        .with_context(|| format!("Failed to read {}", agents_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(config) = serde_json::from_str::<AgentTokenConfig>(&text) else {
            continue;
        };
        if config.token_hash == token_hash {
            return Ok(Some(config.scopes));
        }
    }

    Ok(None)
}

fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(token.as_bytes()))
}
/// Send an HTTP response
fn send_response(stream: &mut TcpStream, status: u16, reason: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status,
        reason,
        body.len(),
        body
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

// --- merged from part_001.rs ---
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use tempfile::TempDir;

    /// Helper to create a server with a temporary kit directory
    fn create_test_server(port: u16) -> anyhow::Result<(McpServer, TempDir)> {
        let temp_dir = TempDir::new().context("create temp dir")?;
        let server = McpServer::new(port, temp_dir.path().to_path_buf())?;
        Ok((server, temp_dir))
    }

    /// Helper to send an HTTP request and get the response
    fn http_request(
        port: u16,
        method: &str,
        path: &str,
        token: Option<&str>,
    ) -> anyhow::Result<(u16, String)> {
        let mut stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).context("connect to server")?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .context("set read timeout")?;

        let mut request = format!("{} {} HTTP/1.1\r\nHost: localhost\r\n", method, path);
        if let Some(token) = token {
            request.push_str(&format!("Authorization: Bearer {}\r\n", token));
        }
        request.push_str("\r\n");

        stream
            .write_all(request.as_bytes())
            .context("write request")?;
        stream.flush().context("flush request")?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .context("read response")?;

        // Parse status code from response
        let status_line = response.lines().next().unwrap_or("");
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Get body (after blank line)
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();

        Ok((status_code, body))
    }

    #[test]
    fn test_server_starts_and_stops() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43211)?;

        // Server should not be running initially
        assert!(!server.is_running());

        // Start server
        let handle = server.start()?;

        // Give server time to start
        thread::sleep(std::time::Duration::from_millis(100));

        // Server should be running
        assert!(handle.is_running());

        // Stop server
        handle.stop();

        // Server should stop
        assert!(!server.is_running());
        Ok(())
    }

    #[test]
    fn test_health_endpoint_returns_200() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43212)?;
        let _handle = server.start()?;

        // Give server time to start
        thread::sleep(std::time::Duration::from_millis(100));

        let (status, body) = http_request(43212, "GET", "/health", None)?;

        assert_eq!(status, 200);
        assert!(body.contains("healthy"));
        Ok(())
    }

    #[test]
    fn test_auth_rejects_invalid_token() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43213)?;
        let _handle = server.start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        // Request to root without token should fail
        let (status, _) = http_request(43213, "GET", "/", None)?;
        assert_eq!(status, 401);

        // Request with wrong token should fail
        let (status, _) = http_request(43213, "GET", "/", Some("wrong-token"))?;
        assert_eq!(status, 401);
        Ok(())
    }

    #[test]
    fn test_auth_accepts_valid_token() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43214)?;
        let token = server.token().to_string();
        let _handle = server.start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        let (status, body) = http_request(43214, "GET", "/", Some(&token))?;

        assert_eq!(status, 200);
        assert!(body.contains("Script Kit MCP Server"));
        Ok(())
    }

    #[test]
    fn test_auth_accepts_scoped_agent_token_and_applies_scopes() -> anyhow::Result<()> {
        let (server, temp_dir) = create_test_server(43228)?;
        let scoped_token = "scoped-agent-token-for-test";
        let agents_dir = temp_dir.path().join("agents");
        fs::create_dir_all(&agents_dir).context("create agents dir")?;
        let config = serde_json::json!({
            "tokenHash": hash_token(scoped_token),
            "scopes": ["mcp:read"]
        });
        fs::write(
            agents_dir.join("readonly-agent.json"),
            serde_json::to_string_pretty(&config)?,
        )
        .context("write scoped agent config")?;

        let _handle = server.start()?;
        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":"scope-denied","method":"tools/call","params":{"name":"kit/notes_create","arguments":{"body":"denied"}}}"#;
        let (status, body) = http_post_json(43228, "/rpc", scoped_token, request)?;

        assert_eq!(status, 200);
        let response: serde_json::Value =
            serde_json::from_str(&body).context("parse scoped token RPC response")?;
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["result"]["isError"], true);
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .context("missing tool text")?;
        let payload: serde_json::Value =
            serde_json::from_str(text).context("parse scope-denial tool payload")?;
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("notes:write"),
            "scoped token should authenticate but still deny notes writes"
        );
        Ok(())
    }

    #[test]
    fn test_discovery_file_created() -> anyhow::Result<()> {
        let (server, temp_dir) = create_test_server(43215)?;
        let token = server.token().to_string();

        // Discovery file should not exist before start
        let discovery_path = temp_dir.path().join("server.json");
        assert!(!discovery_path.exists());

        // Start server
        let handle = server.start()?;
        thread::sleep(std::time::Duration::from_millis(100));

        // Discovery file should exist
        assert!(discovery_path.exists());

        // Verify contents
        let content = fs::read_to_string(&discovery_path).context("read discovery file")?;
        let discovery: DiscoveryInfo =
            serde_json::from_str(&content).context("parse discovery JSON")?;

        assert!(discovery.url.contains("43215"));
        assert_eq!(discovery.token, token);
        assert_eq!(discovery.version, VERSION);
        assert!(discovery.capabilities.scripts);

        // Stop server
        handle.stop();

        // Discovery file should be removed after stop
        thread::sleep(std::time::Duration::from_millis(100));
        assert!(!discovery_path.exists());
        Ok(())
    }

    #[test]
    fn test_generates_token_if_missing() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().context("create temp dir")?;
        let token_path = temp_dir.path().join("agent-token");

        // Token file should not exist
        assert!(!token_path.exists());

        // Create server - should generate token
        let server = McpServer::new(43216, temp_dir.path().to_path_buf())?;

        // Token file should now exist
        assert!(token_path.exists());

        // Token should be a valid UUID-like string
        let token = server.token();
        assert!(!token.is_empty());
        assert!(token.len() >= 32); // UUID v4 format

        // Token should match file contents
        let file_token = fs::read_to_string(&token_path).context("read token file")?;
        assert_eq!(token, file_token.trim());

        // Creating another server should use the same token
        let server2 = McpServer::new(43217, temp_dir.path().to_path_buf())?;
        assert_eq!(server.token(), server2.token());
        Ok(())
    }

    #[test]
    fn test_url_format() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43218)?;
        assert_eq!(server.url(), "http://localhost:43218");
        Ok(())
    }

    /// Helper to send a POST request with a JSON body
    fn http_post_json(
        port: u16,
        path: &str,
        token: &str,
        body: &str,
    ) -> anyhow::Result<(u16, String)> {
        let mut stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).context("connect to server")?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .context("set read timeout")?;

        let request = format!(
            "POST {} HTTP/1.1\r\n\
             Host: localhost\r\n\
             Authorization: Bearer {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            path,
            token,
            body.len(),
            body
        );

        stream
            .write_all(request.as_bytes())
            .context("write request")?;
        stream.flush().context("flush request")?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .context("read response")?;

        // Parse status code from response
        let status_line = response.lines().next().unwrap_or("");
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Get body (after blank line)
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();

        Ok((status_code, body))
    }

    #[test]
    fn test_rpc_endpoint_tools_list() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43219)?;
        let token = server.token().to_string();
        let _handle = server.start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let (status, body) = http_post_json(43219, "/rpc", &token, request)?;

        assert_eq!(status, 200);

        // Parse response
        let response: serde_json::Value =
            serde_json::from_str(&body).context("parse RPC response")?;
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response["result"]["tools"].is_array());
        Ok(())
    }

    #[test]
    fn test_rpc_computer_see_routes_live_runtime() -> anyhow::Result<()> {
        struct Runtime;

        impl crate::computer_use::runtime_bridge::ComputerUseRuntimeBridge for Runtime {
            fn inspect_automation_window(
                &self,
                request: crate::computer_use::runtime_bridge::ComputerUseInspectRequest,
            ) -> Result<
                crate::protocol::AutomationInspectSnapshot,
                crate::computer_use::runtime_bridge::ComputerUseRuntimeError,
            > {
                assert_eq!(
                    request.target,
                    Some(crate::protocol::AutomationWindowTarget::Focused)
                );
                Ok(crate::protocol::AutomationInspectSnapshot {
                    schema_version: crate::protocol::AUTOMATION_INSPECT_SCHEMA_VERSION,
                    window_id: "mcp-server-runtime".to_string(),
                    window_kind: "Main".to_string(),
                    surface_kind: Some("ScriptList".to_string()),
                    app_view_variant: Some("ScriptList".to_string()),
                    native_footer_surface: Some("script_list".to_string()),
                    target_generation: Some(1),
                    surface_generation: Some(1),
                    data_generation: Some(1),
                    title: None,
                    resolved_bounds: None,
                    target_bounds_in_screenshot: None,
                    surface_hit_point: None,
                    suggested_hit_points: Vec::new(),
                    elements: Vec::new(),
                    total_count: 0,
                    focused_semantic_id: None,
                    selected_semantic_id: None,
                    screenshot_width: Some(800),
                    screenshot_height: Some(600),
                    pixel_probes: Vec::new(),
                    os_window_id: Some(123),
                    semantic_quality: Some(crate::protocol::SemanticQuality::Full),
                    warnings: Vec::new(),
                    pid: Some(1234),
                })
            }

            fn list_running_apps(
                &self,
                _request: crate::computer_use::runtime_bridge::ComputerUseListAppsRequest,
            ) -> Result<
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
                crate::computer_use::runtime_bridge::ComputerUseRuntimeError,
            > {
                Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                        apps: Vec::new(),
                        frontmost_pid: None,
                    },
                )
            }

            fn list_app_windows(
                &self,
                _request: crate::computer_use::runtime_bridge::ComputerUseListAppWindowsRequest,
            ) -> Result<
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
                crate::computer_use::runtime_bridge::ComputerUseRuntimeError,
            > {
                Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: None,
                        windows: Vec::new(),
                        warnings: Vec::new(),
                    },
                )
            }
        }

        let (server, _temp_dir) = create_test_server(43227)?;
        let token = server.token().to_string();
        let runtime: SharedComputerRuntime = Arc::new(Runtime);
        let _handle = server.with_computer_runtime(runtime).start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"computer/see","arguments":{"target":{"type":"focused"}}}}"#;
        let (status, body) = http_post_json(43227, "/rpc", &token, request)?;

        assert_eq!(status, 200);
        let response: serde_json::Value =
            serde_json::from_str(&body).context("parse RPC response")?;
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .context("missing computer/see result text")?;
        let snapshot: crate::protocol::AutomationInspectSnapshot =
            serde_json::from_str(text).context("parse computer/see snapshot")?;
        assert_eq!(snapshot.window_id, "mcp-server-runtime");
        assert_eq!(snapshot.screenshot_width, Some(800));
        Ok(())
    }

    #[test]
    fn test_rpc_endpoint_initialize() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43220)?;
        let token = server.token().to_string();
        let _handle = server.start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":"init-1","method":"initialize","params":{}}"#;
        let (status, body) = http_post_json(43220, "/rpc", &token, request)?;

        assert_eq!(status, 200);

        let response: serde_json::Value =
            serde_json::from_str(&body).context("parse RPC response")?;
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], "init-1");
        assert!(response["result"]["serverInfo"]["name"].is_string());
        assert!(response["result"]["capabilities"].is_object());
        Ok(())
    }

    #[test]
    fn test_rpc_endpoint_method_not_found() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43221)?;
        let token = server.token().to_string();
        let _handle = server.start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":99,"method":"unknown/method","params":{}}"#;
        let (status, body) = http_post_json(43221, "/rpc", &token, request)?;

        assert_eq!(status, 200);

        let response: serde_json::Value =
            serde_json::from_str(&body).context("parse RPC response")?;
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 99);
        assert_eq!(response["error"]["code"], -32601);
        assert!(response["error"]["message"]
            .as_str()
            .context("error message should be a string")?
            .contains("Method not found"));
        Ok(())
    }

    #[test]
    fn test_rpc_endpoint_invalid_json() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43222)?;
        let token = server.token().to_string();
        let _handle = server.start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0", invalid}"#;
        let (status, body) = http_post_json(43222, "/rpc", &token, request)?;

        assert_eq!(status, 200);

        let response: serde_json::Value =
            serde_json::from_str(&body).context("parse RPC response")?;
        assert_eq!(response["error"]["code"], -32700); // Parse error
        Ok(())
    }

    #[test]
    fn test_rpc_endpoint_requires_auth() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43223)?;
        let _handle = server.start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        // Try POST /rpc without token - should fail auth
        let mut stream = TcpStream::connect("127.0.0.1:43223").context("connect to server")?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .context("set read timeout")?;

        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let request = format!(
            "POST /rpc HTTP/1.1\r\n\
             Host: localhost\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );

        stream
            .write_all(request.as_bytes())
            .context("write request")?;
        stream.flush().context("flush request")?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .context("read response")?;

        let status_line = response.lines().next().unwrap_or("");
        let status_code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        assert_eq!(status_code, 401);
        Ok(())
    }

    #[test]
    fn test_rpc_endpoint_resources_list() -> anyhow::Result<()> {
        let (server, _temp_dir) = create_test_server(43224)?;
        let token = server.token().to_string();
        let _handle = server.start()?;

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}"#;
        let (status, body) = http_post_json(43224, "/rpc", &token, request)?;

        assert_eq!(status, 200);

        let response: serde_json::Value =
            serde_json::from_str(&body).context("parse RPC response")?;
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 2);
        assert!(response["result"]["resources"].is_array());
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_token_file_has_secure_permissions() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().context("create temp dir")?;
        let token_path = temp_dir.path().join("agent-token");

        // Create server - should generate token with secure permissions
        let _server = McpServer::new(43225, temp_dir.path().to_path_buf())?;

        // Token file should exist
        assert!(token_path.exists());

        // Check file permissions (should be 0o600 - owner read/write only)
        let metadata = fs::metadata(&token_path).context("read token file metadata")?;
        let mode = metadata.permissions().mode();
        let file_perms = mode & 0o777;

        assert_eq!(
            file_perms, 0o600,
            "Token file should have 0o600 permissions, got 0o{:o}",
            file_perms
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_discovery_file_has_secure_permissions() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let (server, temp_dir) = create_test_server(43226)?;
        let discovery_path = temp_dir.path().join("server.json");

        // Start server - should create discovery file with secure permissions
        let _handle = server.start()?;
        thread::sleep(std::time::Duration::from_millis(100));

        // Discovery file should exist
        assert!(discovery_path.exists());

        // Check file permissions (should be 0o600 - contains token)
        let metadata = fs::metadata(&discovery_path).context("read discovery file metadata")?;
        let mode = metadata.permissions().mode();
        let file_perms = mode & 0o777;

        assert_eq!(
            file_perms, 0o600,
            "Discovery file should have 0o600 permissions, got 0o{:o}",
            file_perms
        );
        Ok(())
    }
}
