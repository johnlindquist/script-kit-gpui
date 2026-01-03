impl ScriptListApp {
    fn execute_interactive(&mut self, script: &scripts::Script, cx: &mut Context<Self>) {
        logging::log(
            "EXEC",
            &format!("Starting interactive execution: {}", script.name),
        );

        // Store script path for error reporting in reader thread
        let script_path_for_errors = script.path.to_string_lossy().to_string();

        match executor::execute_script_interactive(&script.path) {
            Ok(session) => {
                logging::log("EXEC", "Interactive session started successfully");

                // Store PID for explicit cleanup (belt-and-suspenders approach)
                let pid = session.pid();
                self.current_script_pid = Some(pid);
                logging::log("EXEC", &format!("Stored script PID {} for cleanup", pid));

                *self.script_session.lock() = Some(session);

                // Create async_channel for script thread to send prompt messages to UI (event-driven)
                // P1-6: Use bounded channel to prevent unbounded memory growth from slow UI
                // Capacity of 100 is generous (scripts rarely send > 10 messages/sec)
                let (tx, rx) = async_channel::bounded(100);
                let rx_for_listener = rx.clone();
                self.prompt_receiver = Some(rx);

                // Spawn event-driven listener for prompt messages (replaces 50ms polling)
                cx.spawn(async move |this, cx| {
                    logging::log("EXEC", "Prompt message listener started (event-driven)");

                    // Event-driven: recv().await yields until a message arrives
                    while let Ok(msg) = rx_for_listener.recv().await {
                        logging::log("EXEC", &format!("Prompt message received: {:?}", msg));
                        let _ = cx.update(|cx| {
                            this.update(cx, |app, cx| {
                                app.handle_prompt_message(msg, cx);
                            })
                        });
                    }

                    logging::log("EXEC", "Prompt message listener exiting (channel closed)");
                })
                .detach();

                // We need separate threads for reading and writing to avoid deadlock
                // The read thread blocks on receive_message(), so we can't check for responses in the same loop

                // Take ownership of the session and split it
                let session = self.script_session.lock().take().unwrap();
                let split = session.split();

                let mut stdin = split.stdin;
                let mut stdout_reader = split.stdout_reader;
                // Capture stderr for error reporting - we'll read it in real-time for debugging
                let stderr_handle = split.stderr;
                // CRITICAL: Keep process_handle and child alive - they kill the process on drop!
                // We move them into the reader thread so they live until the script exits.
                let _process_handle = split.process_handle;
                let mut _child = split.child;

                // Stderr reader thread - forwards script stderr to logs in real-time
                if let Some(stderr) = stderr_handle {
                    std::thread::spawn(move || {
                        use std::io::BufRead;
                        let reader = std::io::BufReader::new(stderr);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => logging::log("SCRIPT", &l),
                                Err(e) => {
                                    logging::log("SCRIPT", &format!("stderr read error: {}", e));
                                    break;
                                }
                            }
                        }
                        logging::log("SCRIPT", "stderr reader exiting");
                    });
                }

                // Now stderr_handle is consumed, we pass None to reader thread
                let stderr_handle: Option<std::process::ChildStderr> = None;

                // Channel for sending responses from UI to writer thread
                let (response_tx, response_rx) = mpsc::channel::<Message>();

                // Clone response_tx for the reader thread to handle direct responses
                // (e.g., getSelectedText, setSelectedText, checkAccessibility)
                let reader_response_tx = response_tx.clone();

                // Writer thread - handles sending responses to script
                std::thread::spawn(move || {
                    use std::io::Write;
                    use std::os::unix::io::AsRawFd;

                    // Log the stdin file descriptor for debugging
                    let fd = stdin.as_raw_fd();
                    logging::log("EXEC", &format!("Writer thread started, stdin fd={}", fd));

                    // Check if fd is a valid pipe
                    #[cfg(unix)]
                    {
                        let stat_result = unsafe {
                            let mut stat: libc::stat = std::mem::zeroed();
                            libc::fstat(fd, &mut stat)
                        };
                        if stat_result == 0 {
                            logging::log("EXEC", &format!("fd={} fstat succeeded", fd));
                        } else {
                            logging::log(
                                "EXEC",
                                &format!(
                                    "fd={} fstat FAILED: errno={}",
                                    fd,
                                    std::io::Error::last_os_error()
                                ),
                            );
                        }
                    }

                    loop {
                        match response_rx.recv() {
                            Ok(response) => {
                                let json = match protocol::serialize_message(&response) {
                                    Ok(j) => j,
                                    Err(e) => {
                                        logging::log(
                                            "EXEC",
                                            &format!("Failed to serialize: {}", e),
                                        );
                                        continue;
                                    }
                                };
                                logging::log(
                                    "EXEC",
                                    &format!("Writing to stdin fd={}: {}", fd, json),
                                );
                                let bytes = format!("{}\n", json);
                                let bytes_len = bytes.len();

                                // Check fd validity before write
                                let fcntl_result = unsafe { libc::fcntl(fd, libc::F_GETFD) };
                                logging::log(
                                    "EXEC",
                                    &format!(
                                        "Pre-write fcntl(F_GETFD) on fd={}: {}",
                                        fd, fcntl_result
                                    ),
                                );

                                match stdin.write_all(bytes.as_bytes()) {
                                    Ok(()) => {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Write succeeded: {} bytes to fd={}",
                                                bytes_len, fd
                                            ),
                                        );
                                    }
                                    Err(e) => {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Failed to write {} bytes: {} (kind={:?})",
                                                bytes_len,
                                                e,
                                                e.kind()
                                            ),
                                        );
                                        break;
                                    }
                                }
                                if let Err(e) = stdin.flush() {
                                    logging::log(
                                        "EXEC",
                                        &format!("Failed to flush fd={}: {}", fd, e),
                                    );
                                    break;
                                }
                                logging::log("EXEC", &format!("Flush succeeded for fd={}", fd));
                            }
                            Err(_) => {
                                logging::log("EXEC", "Response channel closed, writer exiting");
                                break;
                            }
                        }
                    }
                    logging::log("EXEC", "Writer thread exiting");
                });

                // Reader thread - handles receiving messages from script (blocking is OK here)
                // CRITICAL: Move _process_handle and _child into this thread to keep them alive!
                // When the reader thread exits, they'll be dropped and the process killed.
                let script_path_clone = script_path_for_errors.clone();
                std::thread::spawn(move || {
                    // These variables keep the process alive - they're dropped when the thread exits
                    let _keep_alive_handle = _process_handle;
                    let mut keep_alive_child = _child;
                    let mut stderr_for_errors = stderr_handle;
                    let script_path = script_path_clone;

                    loop {
                        // Use next_message_graceful_with_handler to skip non-JSON lines and report parse issues
                        match stdout_reader.next_message_graceful_with_handler(|issue| {
                            let should_report = matches!(
                                issue.kind,
                                protocol::ParseIssueKind::InvalidPayload
                                    | protocol::ParseIssueKind::UnknownType
                            );
                            if !should_report {
                                return;
                            }

                            let summary = match issue.kind {
                                protocol::ParseIssueKind::InvalidPayload => issue
                                    .message_type
                                    .as_deref()
                                    .map(|message_type| {
                                        format!(
                                            "Invalid '{}' message payload from script",
                                            message_type
                                        )
                                    })
                                    .unwrap_or_else(|| {
                                        "Invalid message payload from script".to_string()
                                    }),
                                protocol::ParseIssueKind::UnknownType => issue
                                    .message_type
                                    .as_deref()
                                    .map(|message_type| {
                                        format!(
                                            "Unknown '{}' message type from script",
                                            message_type
                                        )
                                    })
                                    .unwrap_or_else(|| {
                                        "Unknown message type from script".to_string()
                                    }),
                                _ => "Protocol message issue from script".to_string(),
                            };

                            let mut details_lines = Vec::new();
                            details_lines.push(format!("Script: {}", script_path));
                            if let Some(ref message_type) = issue.message_type {
                                details_lines.push(format!("Type: {}", message_type));
                            }
                            if let Some(ref error) = issue.error {
                                details_lines.push(format!("Error: {}", error));
                            }
                            if !issue.raw_preview.is_empty() {
                                details_lines.push(format!("Preview: {}", issue.raw_preview));
                            }
                            let details = Some(details_lines.join("\n"));

                            let severity = match issue.kind {
                                protocol::ParseIssueKind::InvalidPayload => ErrorSeverity::Error,
                                protocol::ParseIssueKind::UnknownType => ErrorSeverity::Warning,
                                _ => ErrorSeverity::Warning,
                            };

                            let correlation_id = issue.correlation_id.clone();
                            let prompt_msg = PromptMessage::ProtocolError {
                                correlation_id: issue.correlation_id,
                                summary,
                                details,
                                severity,
                                script_path: script_path.clone(),
                            };

                            if tx.send_blocking(prompt_msg).is_err() {
                                tracing::warn!(
                                    correlation_id = %correlation_id,
                                    script_path = %script_path,
                                    "Prompt channel closed, dropping protocol error"
                                );
                            }
                        }) {
                            Ok(Some(msg)) => {
                                logging::log("EXEC", &format!("Received message: {:?}", msg));

                                // First, try to handle selected text messages directly (no UI needed)
                                match executor::handle_selected_text_message(&msg) {
                                    executor::SelectedTextHandleResult::Handled(response) => {
                                        logging::log("EXEC", &format!("Handled selected text message, sending response: {:?}", response));
                                        if let Err(e) = reader_response_tx.send(response) {
                                            logging::log(
                                                "EXEC",
                                                &format!(
                                                    "Failed to send selected text response: {}",
                                                    e
                                                ),
                                            );
                                        }
                                        continue;
                                    }
                                    executor::SelectedTextHandleResult::NotHandled => {
                                        // Fall through to other message handling
                                    }
                                }

                                // Handle ClipboardHistory directly (no UI needed)
                                if let Message::ClipboardHistory {
                                    request_id,
                                    action,
                                    entry_id,
                                } = &msg
                                {
                                    logging::log(
                                        "EXEC",
                                        &format!("ClipboardHistory request: {:?}", action),
                                    );

                                    let response = match action {
                                        protocol::ClipboardHistoryAction::List => {
                                            let entries =
                                                clipboard_history::get_clipboard_history(100);
                                            let entry_data: Vec<protocol::ClipboardHistoryEntryData> = entries
                                                .into_iter()
                                                .map(|e| {
                                                    // Truncate large content to avoid pipe buffer issues
                                                    // Images are stored as base64 which can be huge
                                                    let content = match e.content_type {
                                                        clipboard_history::ContentType::Image => {
                                                            // For images, send a placeholder with metadata
                                                            format!("[image:{}]", e.id)
                                                        }
                                                        clipboard_history::ContentType::Text => {
                                                            // Truncate very long text entries
                                                            if e.content.len() > 1000 {
                                                                format!("{}...", &e.content[..1000])
                                                            } else {
                                                                e.content
                                                            }
                                                        }
                                                    };
                                                    protocol::ClipboardHistoryEntryData {
                                                        entry_id: e.id,
                                                        content,
                                                        content_type: match e.content_type {
                                                            clipboard_history::ContentType::Text => protocol::ClipboardEntryType::Text,
                                                            clipboard_history::ContentType::Image => protocol::ClipboardEntryType::Image,
                                                        },
                                                        timestamp: chrono::DateTime::from_timestamp(e.timestamp, 0)
                                                            .map(|dt| dt.to_rfc3339())
                                                            .unwrap_or_default(),
                                                        pinned: e.pinned,
                                                    }
                                                })
                                                .collect();
                                            Message::clipboard_history_list_response(
                                                request_id.clone(),
                                                entry_data,
                                            )
                                        }
                                        protocol::ClipboardHistoryAction::Pin => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::pin_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(
                                                        request_id.clone(),
                                                    ),
                                                    Err(e) => Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        e.to_string(),
                                                    ),
                                                }
                                            } else {
                                                Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    "Missing entry_id".to_string(),
                                                )
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Unpin => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::unpin_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(
                                                        request_id.clone(),
                                                    ),
                                                    Err(e) => Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        e.to_string(),
                                                    ),
                                                }
                                            } else {
                                                Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    "Missing entry_id".to_string(),
                                                )
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Remove => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::remove_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(
                                                        request_id.clone(),
                                                    ),
                                                    Err(e) => Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        e.to_string(),
                                                    ),
                                                }
                                            } else {
                                                Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    "Missing entry_id".to_string(),
                                                )
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Clear => {
                                            match clipboard_history::clear_history() {
                                                Ok(()) => Message::clipboard_history_success(
                                                    request_id.clone(),
                                                ),
                                                Err(e) => Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    e.to_string(),
                                                ),
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::TrimOversize => {
                                            match clipboard_history::trim_oversize_text_entries() {
                                                Ok(_) => Message::clipboard_history_success(
                                                    request_id.clone(),
                                                ),
                                                Err(e) => Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    e.to_string(),
                                                ),
                                            }
                                        }
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Failed to send clipboard history response: {}",
                                                e
                                            ),
                                        );
                                    }
                                    continue;
                                }

                                // Handle Clipboard read/write directly (no UI needed)
                                if let Message::Clipboard {
                                    id,
                                    action,
                                    format,
                                    content,
                                } = &msg
                                {
                                    logging::log(
                                        "EXEC",
                                        &format!(
                                            "Clipboard request: {:?} format: {:?}",
                                            action, format
                                        ),
                                    );

                                    // If no request ID, we can't send a response, so just handle and continue
                                    let req_id = match id {
                                        Some(rid) => rid.clone(),
                                        None => {
                                            // Handle clipboard operation without response
                                            if let protocol::ClipboardAction::Write = action {
                                                if let Some(text) = content {
                                                    use arboard::Clipboard;
                                                    if let Ok(mut clipboard) = Clipboard::new() {
                                                        let _ = clipboard.set_text(text.clone());
                                                    }
                                                }
                                            }
                                            continue;
                                        }
                                    };

                                    let response = match action {
                                        protocol::ClipboardAction::Read => {
                                            // Read from clipboard
                                            match format {
                                                Some(protocol::ClipboardFormat::Text) | None => {
                                                    use arboard::Clipboard;
                                                    match Clipboard::new() {
                                                        Ok(mut clipboard) => {
                                                            match clipboard.get_text() {
                                                                Ok(text) => Message::Submit {
                                                                    id: req_id,
                                                                    value: Some(text),
                                                                },
                                                                Err(e) => {
                                                                    logging::log("EXEC", &format!("Clipboard read error: {}", e));
                                                                    Message::Submit {
                                                                        id: req_id,
                                                                        value: Some(String::new()),
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            logging::log(
                                                                "EXEC",
                                                                &format!(
                                                                    "Clipboard init error: {}",
                                                                    e
                                                                ),
                                                            );
                                                            Message::Submit {
                                                                id: req_id,
                                                                value: Some(String::new()),
                                                            }
                                                        }
                                                    }
                                                }
                                                Some(protocol::ClipboardFormat::Image) => {
                                                    use arboard::Clipboard;
                                                    match Clipboard::new() {
                                                        Ok(mut clipboard) => {
                                                            match clipboard.get_image() {
                                                                Ok(img) => {
                                                                    // Convert image to base64
                                                                    use base64::Engine;
                                                                    let bytes = img.bytes.to_vec();
                                                                    let base64_str = base64::engine::general_purpose::STANDARD.encode(&bytes);
                                                                    Message::Submit {
                                                                        id: req_id,
                                                                        value: Some(base64_str),
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    logging::log("EXEC", &format!("Clipboard read image error: {}", e));
                                                                    Message::Submit {
                                                                        id: req_id,
                                                                        value: Some(String::new()),
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            logging::log(
                                                                "EXEC",
                                                                &format!(
                                                                    "Clipboard init error: {}",
                                                                    e
                                                                ),
                                                            );
                                                            Message::Submit {
                                                                id: req_id,
                                                                value: Some(String::new()),
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        protocol::ClipboardAction::Write => {
                                            // Write to clipboard
                                            use arboard::Clipboard;
                                            match Clipboard::new() {
                                                Ok(mut clipboard) => {
                                                    if let Some(text) = content {
                                                        match clipboard.set_text(text.clone()) {
                                                            Ok(()) => {
                                                                logging::log("EXEC", &format!("Clipboard write success: {} bytes", text.len()));
                                                                Message::Submit {
                                                                    id: req_id,
                                                                    value: Some("ok".to_string()),
                                                                }
                                                            }
                                                            Err(e) => {
                                                                logging::log(
                                                                    "EXEC",
                                                                    &format!(
                                                                        "Clipboard write error: {}",
                                                                        e
                                                                    ),
                                                                );
                                                                Message::Submit {
                                                                    id: req_id,
                                                                    value: Some(String::new()),
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        logging::log(
                                                            "EXEC",
                                                            "Clipboard write: no content provided",
                                                        );
                                                        Message::Submit {
                                                            id: req_id,
                                                            value: Some(String::new()),
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    logging::log(
                                                        "EXEC",
                                                        &format!("Clipboard init error: {}", e),
                                                    );
                                                    Message::Submit {
                                                        id: req_id,
                                                        value: Some(String::new()),
                                                    }
                                                }
                                            }
                                        }
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!("Failed to send clipboard response: {}", e),
                                        );
                                    }
                                    continue;
                                }

                                // Handle WindowList directly (no UI needed)
                                if let Message::WindowList { request_id } = &msg {
                                    logging::log(
                                        "EXEC",
                                        &format!("WindowList request: {}", request_id),
                                    );

                                    let response = match window_control::list_windows() {
                                        Ok(windows) => {
                                            let window_infos: Vec<protocol::SystemWindowInfo> =
                                                windows
                                                    .into_iter()
                                                    .map(|w| protocol::SystemWindowInfo {
                                                        window_id: w.id,
                                                        title: w.title,
                                                        app_name: w.app,
                                                        bounds: Some(
                                                            protocol::TargetWindowBounds {
                                                                x: w.bounds.x,
                                                                y: w.bounds.y,
                                                                width: w.bounds.width,
                                                                height: w.bounds.height,
                                                            },
                                                        ),
                                                        is_minimized: None,
                                                        is_active: None,
                                                    })
                                                    .collect();
                                            Message::window_list_result(
                                                request_id.clone(),
                                                window_infos,
                                            )
                                        }
                                        Err(e) => {
                                            logging::log(
                                                "EXEC",
                                                &format!("WindowList error: {}", e),
                                            );
                                            // Return empty list on error
                                            Message::window_list_result(request_id.clone(), vec![])
                                        }
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!("Failed to send window list response: {}", e),
                                        );
                                    }
                                    continue;
                                }

                                // Handle WindowAction directly (no UI needed)
                                if let Message::WindowAction {
                                    request_id,
                                    action,
                                    window_id,
                                    bounds,
                                } = &msg
                                {
                                    logging::log(
                                        "EXEC",
                                        &format!(
                                            "WindowAction request: {:?} for window {:?}",
                                            action, window_id
                                        ),
                                    );

                                    let result = match action {
                                        protocol::WindowActionType::Focus => {
                                            if let Some(id) = window_id {
                                                window_control::focus_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Close => {
                                            if let Some(id) = window_id {
                                                window_control::close_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Minimize => {
                                            if let Some(id) = window_id {
                                                window_control::minimize_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Maximize => {
                                            if let Some(id) = window_id {
                                                window_control::maximize_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Resize => {
                                            if let (Some(id), Some(b)) = (window_id, bounds) {
                                                window_control::resize_window(
                                                    *id, b.width, b.height,
                                                )
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id or bounds"))
                                            }
                                        }
                                        protocol::WindowActionType::Move => {
                                            if let (Some(id), Some(b)) = (window_id, bounds) {
                                                window_control::move_window(*id, b.x, b.y)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id or bounds"))
                                            }
                                        }
                                    };

                                    let response = match result {
                                        Ok(()) => {
                                            Message::window_action_success(request_id.clone())
                                        }
                                        Err(e) => Message::window_action_error(
                                            request_id.clone(),
                                            e.to_string(),
                                        ),
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Failed to send window action response: {}",
                                                e
                                            ),
                                        );
                                    }
                                    continue;
                                }

                                // Handle FileSearch directly (no UI needed)
                                if let Message::FileSearch {
                                    request_id,
                                    query,
                                    only_in,
                                } = &msg
                                {
                                    logging::log(
                                        "EXEC",
                                        &format!(
                                            "FileSearch request: query='{}', only_in={:?}",
                                            query, only_in
                                        ),
                                    );

                                    let results = file_search::search_files(
                                        query,
                                        only_in.as_deref(),
                                        file_search::DEFAULT_LIMIT,
                                    );
                                    let file_entries: Vec<protocol::FileSearchResultEntry> =
                                        results
                                            .into_iter()
                                            .map(|f| protocol::FileSearchResultEntry {
                                                path: f.path,
                                                name: f.name,
                                                is_directory: f.file_type
                                                    == file_search::FileType::Directory,
                                                size: Some(f.size),
                                                modified_at: chrono::DateTime::from_timestamp(
                                                    f.modified as i64,
                                                    0,
                                                )
                                                .map(|dt| dt.to_rfc3339()),
                                            })
                                            .collect();

                                    let response = Message::file_search_result(
                                        request_id.clone(),
                                        file_entries,
                                    );

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!("Failed to send file search response: {}", e),
                                        );
                                    }
                                    continue;
                                }

                                // Handle GetWindowBounds directly (no UI needed)
                                if let Message::GetWindowBounds { request_id } = &msg {
                                    logging::log(
                                        "EXEC",
                                        &format!("GetWindowBounds request: {}", request_id),
                                    );

                                    #[cfg(target_os = "macos")]
                                    let bounds_json = {
                                        if let Some(window) = window_manager::get_main_window() {
                                            unsafe {
                                                // Get the window frame
                                                let frame: NSRect = msg_send![window, frame];

                                                // Get the PRIMARY screen's height for coordinate conversion
                                                // macOS uses bottom-left origin, we convert to top-left
                                                let screens: id =
                                                    msg_send![class!(NSScreen), screens];
                                                let main_screen: id =
                                                    msg_send![screens, firstObject];
                                                let main_screen_frame: NSRect =
                                                    msg_send![main_screen, frame];
                                                let primary_screen_height =
                                                    main_screen_frame.size.height;

                                                // Convert from bottom-left origin (macOS) to top-left origin
                                                let flipped_y = primary_screen_height
                                                    - frame.origin.y
                                                    - frame.size.height;

                                                logging::log("EXEC", &format!(
                                                    "Window bounds: x={:.0}, y={:.0}, width={:.0}, height={:.0}",
                                                    frame.origin.x, flipped_y, frame.size.width, frame.size.height
                                                ));

                                                // Create JSON string with bounds
                                                format!(
                                                    r#"{{"x":{},"y":{},"width":{},"height":{}}}"#,
                                                    frame.origin.x as f64,
                                                    flipped_y as f64,
                                                    frame.size.width as f64,
                                                    frame.size.height as f64
                                                )
                                            }
                                        } else {
                                            logging::log(
                                                "EXEC",
                                                "GetWindowBounds: Main window not registered",
                                            );
                                            r#"{"error":"Main window not found"}"#.to_string()
                                        }
                                    };

                                    #[cfg(not(target_os = "macos"))]
                                    let bounds_json =
                                        r#"{"error":"Not supported on this platform"}"#.to_string();

                                    let response = Message::Submit {
                                        id: request_id.clone(),
                                        value: Some(bounds_json),
                                    };
                                    logging::log(
                                        "EXEC",
                                        &format!("Sending window bounds response: {:?}", response),
                                    );
                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Failed to send window bounds response: {}",
                                                e
                                            ),
                                        );
                                    }
                                    continue;
                                }

                                // Handle GetState - needs UI state, forward to UI thread
                                if let Message::GetState { request_id } = &msg {
                                    logging::log(
                                        "EXEC",
                                        &format!("GetState request: {}", request_id),
                                    );
                                    let prompt_msg = PromptMessage::GetState {
                                        request_id: request_id.clone(),
                                    };
                                    if tx.send_blocking(prompt_msg).is_err() {
                                        logging::log(
                                            "EXEC",
                                            "Prompt channel closed, reader exiting",
                                        );
                                        break;
                                    }
                                    continue;
                                }

                                // Handle CaptureScreenshot directly (no UI needed)
                                if let Message::CaptureScreenshot { request_id, hi_dpi } = &msg {
                                    let hi_dpi_mode = hi_dpi.unwrap_or(false);
                                    tracing::info!(request_id = %request_id, hi_dpi = hi_dpi_mode, "Capturing screenshot");

                                    let response = match capture_app_screenshot(hi_dpi_mode) {
                                        Ok((png_data, width, height)) => {
                                            use base64::Engine;
                                            let base64_data =
                                                base64::engine::general_purpose::STANDARD
                                                    .encode(&png_data);
                                            tracing::info!(
                                                request_id = %request_id,
                                                width = width,
                                                height = height,
                                                hi_dpi = hi_dpi_mode,
                                                data_len = base64_data.len(),
                                                "Screenshot captured successfully"
                                            );
                                            Message::screenshot_result(
                                                request_id.clone(),
                                                base64_data,
                                                width,
                                                height,
                                            )
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                request_id = %request_id,
                                                error = %e,
                                                "Screenshot capture failed"
                                            );
                                            // Send empty result on error
                                            Message::screenshot_result(
                                                request_id.clone(),
                                                String::new(),
                                                0,
                                                0,
                                            )
                                        }
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        tracing::error!(error = %e, "Failed to send screenshot response");
                                    }
                                    continue;
                                }

                                let prompt_msg = match msg {
                                    Message::Arg {
                                        id,
                                        placeholder,
                                        choices,
                                        actions,
                                    } => Some(PromptMessage::ShowArg {
                                        id,
                                        placeholder,
                                        choices,
                                        actions,
                                    }),
                                    Message::Div {
                                        id,
                                        html,
                                        container_classes,
                                        actions,
                                        placeholder,
                                        hint,
                                        footer,
                                        container_bg,
                                        container_padding,
                                        opacity,
                                    } => Some(PromptMessage::ShowDiv {
                                        id,
                                        html,
                                        container_classes,
                                        actions,
                                        placeholder,
                                        hint,
                                        footer,
                                        container_bg,
                                        container_padding,
                                        opacity,
                                    }),
                                    Message::Form { id, html, actions } => {
                                        Some(PromptMessage::ShowForm { id, html, actions })
                                    }
                                    Message::Term {
                                        id,
                                        command,
                                        actions,
                                    } => Some(PromptMessage::ShowTerm {
                                        id,
                                        command,
                                        actions,
                                    }),
                                    Message::Editor {
                                        id,
                                        content,
                                        language,
                                        template,
                                        actions,
                                        ..
                                    } => Some(PromptMessage::ShowEditor {
                                        id,
                                        content,
                                        language,
                                        template,
                                        actions,
                                    }),
                                    // New prompt types (scaffolding)
                                    Message::Path {
                                        id,
                                        start_path,
                                        hint,
                                    } => Some(PromptMessage::ShowPath {
                                        id,
                                        start_path,
                                        hint,
                                    }),
                                    Message::Env { id, key, secret } => {
                                        Some(PromptMessage::ShowEnv {
                                            id,
                                            key,
                                            prompt: None,
                                            secret: secret.unwrap_or(false),
                                        })
                                    }
                                    Message::Drop { id } => Some(PromptMessage::ShowDrop {
                                        id,
                                        placeholder: None,
                                        hint: None,
                                    }),
                                    Message::Template { id, template } => {
                                        Some(PromptMessage::ShowTemplate { id, template })
                                    }
                                    Message::Select {
                                        id,
                                        placeholder,
                                        choices,
                                        multiple,
                                    } => Some(PromptMessage::ShowSelect {
                                        id,
                                        placeholder: Some(placeholder),
                                        choices,
                                        multiple: multiple.unwrap_or(false),
                                    }),
                                    Message::Exit { .. } => Some(PromptMessage::ScriptExit),
                                    Message::ForceSubmit { value } => {
                                        Some(PromptMessage::ForceSubmit { value })
                                    }
                                    Message::Hide {} => Some(PromptMessage::HideWindow),
                                    Message::Browse { url } => {
                                        Some(PromptMessage::OpenBrowser { url })
                                    }
                                    Message::Hud { text, duration_ms } => {
                                        Some(PromptMessage::ShowHud { text, duration_ms })
                                    }
                                    Message::SetActions { actions } => {
                                        Some(PromptMessage::SetActions { actions })
                                    }
                                    Message::SetInput { text } => {
                                        Some(PromptMessage::SetInput { text })
                                    }
                                    Message::ShowGrid { options } => {
                                        Some(PromptMessage::ShowGrid { options })
                                    }
                                    Message::HideGrid => Some(PromptMessage::HideGrid),
                                    other => {
                                        // Get the message type name for user feedback
                                        let msg_type = format!("{:?}", other);
                                        // Extract just the variant name (before any {})
                                        let type_name = msg_type
                                            .split('{')
                                            .next()
                                            .unwrap_or(&msg_type)
                                            .trim()
                                            .to_string();
                                        logging::log(
                                            "WARN",
                                            &format!("Unhandled message type: {}", type_name),
                                        );
                                        Some(PromptMessage::UnhandledMessage {
                                            message_type: type_name,
                                        })
                                    }
                                };

                                if let Some(prompt_msg) = prompt_msg {
                                    if tx.send_blocking(prompt_msg).is_err() {
                                        logging::log(
                                            "EXEC",
                                            "Prompt channel closed, reader exiting",
                                        );
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {
                                logging::log("EXEC", "Script stdout closed (EOF)");

                                // Check if process exited with error
                                let exit_code = match keep_alive_child.try_wait() {
                                    Ok(Some(status)) => status.code(),
                                    Ok(None) => {
                                        // Process still running, wait for it
                                        match keep_alive_child.wait() {
                                            Ok(status) => status.code(),
                                            Err(_) => None,
                                        }
                                    }
                                    Err(_) => None,
                                };

                                logging::log("EXEC", &format!("Script exit code: {:?}", exit_code));

                                // If non-zero exit code, capture stderr and send error
                                if let Some(code) = exit_code {
                                    if code != 0 {
                                        // Read stderr if available
                                        let stderr_output =
                                            if let Some(mut stderr) = stderr_for_errors.take() {
                                                use std::io::Read;
                                                let mut stderr_str = String::new();
                                                if stderr.read_to_string(&mut stderr_str).is_ok()
                                                    && !stderr_str.is_empty()
                                                {
                                                    Some(stderr_str)
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                        if let Some(ref stderr_text) = stderr_output {
                                            logging::log(
                                                "EXEC",
                                                &format!(
                                                    "Captured stderr ({} bytes)",
                                                    stderr_text.len()
                                                ),
                                            );

                                            // Parse error info and generate suggestions
                                            let error_message =
                                                executor::extract_error_message(stderr_text);
                                            let stack_trace =
                                                executor::parse_stack_trace(stderr_text);
                                            let suggestions = executor::generate_suggestions(
                                                stderr_text,
                                                Some(code),
                                            );

                                            // Send script error message
                                            let _ = tx.send_blocking(PromptMessage::ScriptError {
                                                error_message,
                                                stderr_output: Some(stderr_text.clone()),
                                                exit_code: Some(code),
                                                stack_trace,
                                                script_path: script_path.clone(),
                                                suggestions,
                                            });
                                        } else {
                                            // No stderr, send generic error
                                            let _ = tx.send_blocking(PromptMessage::ScriptError {
                                                error_message: format!(
                                                    "Script exited with code {}",
                                                    code
                                                ),
                                                stderr_output: None,
                                                exit_code: Some(code),
                                                stack_trace: None,
                                                script_path: script_path.clone(),
                                                suggestions: vec![
                                                    "Check the script for errors".to_string()
                                                ],
                                            });
                                        }
                                    }
                                }

                                let _ = tx.send_blocking(PromptMessage::ScriptExit);
                                break;
                            }
                            Err(e) => {
                                logging::log("EXEC", &format!("Error reading from script: {}", e));

                                // Try to read stderr for error details
                                let stderr_output =
                                    if let Some(mut stderr) = stderr_for_errors.take() {
                                        use std::io::Read;
                                        let mut stderr_str = String::new();
                                        if stderr.read_to_string(&mut stderr_str).is_ok()
                                            && !stderr_str.is_empty()
                                        {
                                            Some(stderr_str)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    };

                                if let Some(ref stderr_text) = stderr_output {
                                    let error_message =
                                        executor::extract_error_message(stderr_text);
                                    let stack_trace = executor::parse_stack_trace(stderr_text);
                                    let suggestions =
                                        executor::generate_suggestions(stderr_text, None);

                                    let _ = tx.send_blocking(PromptMessage::ScriptError {
                                        error_message,
                                        stderr_output: Some(stderr_text.clone()),
                                        exit_code: None,
                                        stack_trace,
                                        script_path: script_path.clone(),
                                        suggestions,
                                    });
                                }

                                let _ = tx.send_blocking(PromptMessage::ScriptExit);
                                break;
                            }
                        }
                    }
                    logging::log(
                        "EXEC",
                        "Reader thread exited, process handle will now be dropped",
                    );
                });

                // Store the response sender for the UI to use
                self.response_sender = Some(response_tx);
            }
            Err(e) => {
                logging::log(
                    "EXEC",
                    &format!("Failed to start interactive session: {}", e),
                );
                self.last_output = Some(SharedString::from(format!("✗ Error: {}", e)));
                cx.notify();
            }
        }
    }
}
