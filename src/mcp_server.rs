//! MCP Server Foundation
//!
//! Provides an HTTP server for MCP (Model Context Protocol) integration.
//! Features:
//! - HTTP server on localhost:43210
//! - Bearer token authentication from ~/.sk/kit/agent-token
//! - Health endpoint at GET /health
//! - Discovery file at ~/.sk/kit/server.json

// Allow dead code - ServerHandle methods provide full lifecycle API for future use
#![allow(dead_code)]

use crate::mcp_protocol::{self, JsonRpcResponse};
use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, warn};

/// Default port for the MCP server
pub const DEFAULT_PORT: u16 = 43210;

/// MCP Server version for discovery
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// Discovery file structure written to ~/.sk/kit/server.json
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
}

impl McpServer {
    /// Create a new MCP server instance
    ///
    /// # Arguments
    /// * `port` - Port to listen on (default: 43210)
    /// * `kit_path` - Path to ~/.sk/kit directory
    pub fn new(port: u16, kit_path: PathBuf) -> Result<Self> {
        let token = Self::load_or_create_token(&kit_path)?;

        Ok(Self {
            port,
            token,
            running: Arc::new(AtomicBool::new(false)),
            kit_path,
        })
    }

    /// Create server with default settings
    pub fn with_defaults() -> Result<Self> {
        let kit_path = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".sk/kit");

        let port = std::env::var("MCP_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);

        Self::new(port, kit_path)
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

        fs::write(&token_path, &token).context("Failed to write agent-token file")?;

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

    /// Write discovery file to ~/.sk/kit/server.json
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

        fs::write(&discovery_path, json).context("Failed to write server.json")?;

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

        let handle = thread::spawn(move || {
            info!("MCP server started on port {}", port);

            while running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        debug!("Connection from {}", addr);
                        let token = token.clone();
                        thread::spawn(move || {
                            if let Err(e) = handle_connection(stream, &token) {
                                error!("Error handling connection: {}", e);
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No connection available, sleep briefly
                        thread::sleep(std::time::Duration::from_millis(10));
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
fn handle_connection(mut stream: TcpStream, expected_token: &str) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    // Read request line
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let request_line = request_line.trim();

    debug!("Request: {}", request_line);

    // Parse method and path
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return send_response(&mut stream, 400, "Bad Request", "Invalid request line");
    }

    let method = parts[0];
    let path = parts[1];

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
    if path != "/health" {
        let auth_valid = headers
            .get("authorization")
            .map(|auth| {
                auth.strip_prefix("Bearer ")
                    .map(|token| token == expected_token)
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        if !auth_valid {
            return send_response(&mut stream, 401, "Unauthorized", "Invalid or missing token");
        }
    }

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
            handle_rpc_request(&mut reader, &mut stream, &headers)
        }
        _ => send_response(&mut stream, 404, "Not Found", "Endpoint not found"),
    }
}

/// Handle a JSON-RPC request on the /rpc endpoint
fn handle_rpc_request(
    reader: &mut BufReader<TcpStream>,
    stream: &mut TcpStream,
    headers: &std::collections::HashMap<String, String>,
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

    debug!("RPC request body: {}", body_str);

    // Load scripts and scriptlets for context-aware responses
    // This allows resources/read and tools/list to return actual data
    let scripts = crate::scripts::read_scripts();
    let scriptlets = crate::scripts::load_scriptlets();

    // Parse and handle request with full context
    let response = match mcp_protocol::parse_request(&body_str) {
        Ok(request) => {
            mcp_protocol::handle_request_with_context(request, &scripts, &scriptlets, None)
        }
        Err(error_response) => error_response,
    };

    let response_body = serde_json::to_string(&response)?;
    send_response(stream, 200, "OK", &response_body)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use tempfile::TempDir;

    /// Helper to create a server with a temporary kit directory
    fn create_test_server(port: u16) -> (McpServer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let server = McpServer::new(port, temp_dir.path().to_path_buf()).unwrap();
        (server, temp_dir)
    }

    /// Helper to send an HTTP request and get the response
    fn http_request(port: u16, method: &str, path: &str, token: Option<&str>) -> (u16, String) {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        let mut request = format!("{} {} HTTP/1.1\r\nHost: localhost\r\n", method, path);
        if let Some(token) = token {
            request.push_str(&format!("Authorization: Bearer {}\r\n", token));
        }
        request.push_str("\r\n");

        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        // Parse status code from response
        let status_line = response.lines().next().unwrap_or("");
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Get body (after blank line)
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();

        (status_code, body)
    }

    #[test]
    fn test_server_starts_and_stops() {
        let (server, _temp_dir) = create_test_server(43211);

        // Server should not be running initially
        assert!(!server.is_running());

        // Start server
        let handle = server.start().unwrap();

        // Give server time to start
        thread::sleep(std::time::Duration::from_millis(100));

        // Server should be running
        assert!(handle.is_running());

        // Stop server
        handle.stop();

        // Server should stop
        assert!(!server.is_running());
    }

    #[test]
    fn test_health_endpoint_returns_200() {
        let (server, _temp_dir) = create_test_server(43212);
        let _handle = server.start().unwrap();

        // Give server time to start
        thread::sleep(std::time::Duration::from_millis(100));

        let (status, body) = http_request(43212, "GET", "/health", None);

        assert_eq!(status, 200);
        assert!(body.contains("healthy"));
    }

    #[test]
    fn test_auth_rejects_invalid_token() {
        let (server, _temp_dir) = create_test_server(43213);
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        // Request to root without token should fail
        let (status, _) = http_request(43213, "GET", "/", None);
        assert_eq!(status, 401);

        // Request with wrong token should fail
        let (status, _) = http_request(43213, "GET", "/", Some("wrong-token"));
        assert_eq!(status, 401);
    }

    #[test]
    fn test_auth_accepts_valid_token() {
        let (server, _temp_dir) = create_test_server(43214);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let (status, body) = http_request(43214, "GET", "/", Some(&token));

        assert_eq!(status, 200);
        assert!(body.contains("Script Kit MCP Server"));
    }

    #[test]
    fn test_discovery_file_created() {
        let (server, temp_dir) = create_test_server(43215);
        let token = server.token().to_string();

        // Discovery file should not exist before start
        let discovery_path = temp_dir.path().join("server.json");
        assert!(!discovery_path.exists());

        // Start server
        let handle = server.start().unwrap();
        thread::sleep(std::time::Duration::from_millis(100));

        // Discovery file should exist
        assert!(discovery_path.exists());

        // Verify contents
        let content = fs::read_to_string(&discovery_path).unwrap();
        let discovery: DiscoveryInfo = serde_json::from_str(&content).unwrap();

        assert!(discovery.url.contains("43215"));
        assert_eq!(discovery.token, token);
        assert_eq!(discovery.version, VERSION);
        assert!(discovery.capabilities.scripts);

        // Stop server
        handle.stop();

        // Discovery file should be removed after stop
        thread::sleep(std::time::Duration::from_millis(100));
        assert!(!discovery_path.exists());
    }

    #[test]
    fn test_generates_token_if_missing() {
        let temp_dir = TempDir::new().unwrap();
        let token_path = temp_dir.path().join("agent-token");

        // Token file should not exist
        assert!(!token_path.exists());

        // Create server - should generate token
        let server = McpServer::new(43216, temp_dir.path().to_path_buf()).unwrap();

        // Token file should now exist
        assert!(token_path.exists());

        // Token should be a valid UUID-like string
        let token = server.token();
        assert!(!token.is_empty());
        assert!(token.len() >= 32); // UUID v4 format

        // Token should match file contents
        let file_token = fs::read_to_string(&token_path).unwrap();
        assert_eq!(token, file_token.trim());

        // Creating another server should use the same token
        let server2 = McpServer::new(43217, temp_dir.path().to_path_buf()).unwrap();
        assert_eq!(server.token(), server2.token());
    }

    #[test]
    fn test_url_format() {
        let (server, _temp_dir) = create_test_server(43218);
        assert_eq!(server.url(), "http://localhost:43218");
    }

    /// Helper to send a POST request with a JSON body
    fn http_post_json(port: u16, path: &str, token: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

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

        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        // Parse status code from response
        let status_line = response.lines().next().unwrap_or("");
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Get body (after blank line)
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();

        (status_code, body)
    }

    #[test]
    fn test_rpc_endpoint_tools_list() {
        let (server, _temp_dir) = create_test_server(43219);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let (status, body) = http_post_json(43219, "/rpc", &token, request);

        assert_eq!(status, 200);

        // Parse response
        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response["result"]["tools"].is_array());
    }

    #[test]
    fn test_rpc_endpoint_initialize() {
        let (server, _temp_dir) = create_test_server(43220);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":"init-1","method":"initialize","params":{}}"#;
        let (status, body) = http_post_json(43220, "/rpc", &token, request);

        assert_eq!(status, 200);

        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], "init-1");
        assert!(response["result"]["serverInfo"]["name"].is_string());
        assert!(response["result"]["capabilities"].is_object());
    }

    #[test]
    fn test_rpc_endpoint_method_not_found() {
        let (server, _temp_dir) = create_test_server(43221);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":99,"method":"unknown/method","params":{}}"#;
        let (status, body) = http_post_json(43221, "/rpc", &token, request);

        assert_eq!(status, 200);

        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 99);
        assert_eq!(response["error"]["code"], -32601);
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Method not found"));
    }

    #[test]
    fn test_rpc_endpoint_invalid_json() {
        let (server, _temp_dir) = create_test_server(43222);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0", invalid}"#;
        let (status, body) = http_post_json(43222, "/rpc", &token, request);

        assert_eq!(status, 200);

        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["error"]["code"], -32700); // Parse error
    }

    #[test]
    fn test_rpc_endpoint_requires_auth() {
        let (server, _temp_dir) = create_test_server(43223);
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        // Try POST /rpc without token - should fail auth
        let mut stream = TcpStream::connect("127.0.0.1:43223").unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

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

        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        let status_line = response.lines().next().unwrap_or("");
        let status_code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        assert_eq!(status_code, 401);
    }

    #[test]
    fn test_rpc_endpoint_resources_list() {
        let (server, _temp_dir) = create_test_server(43224);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}"#;
        let (status, body) = http_post_json(43224, "/rpc", &token, request);

        assert_eq!(status, 200);

        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 2);
        assert!(response["result"]["resources"].is_array());
    }
}
