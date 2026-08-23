pub fn apply_mcp_notes_mutation_on_main_thread(
    request: NotesMutationRequest,
    cx: &mut App,
) -> Result<NotesMutationResult, NotesMutationError> {
    storage::init_notes_db().map_err(internal_notes_error)?;
    save_open_notes_window_if_dirty(cx)?;

    let result = match request {
        NotesMutationRequest::Create(args) => create_note_from_mcp(args)?,
        NotesMutationRequest::Update(args) => update_note_from_mcp(args)?,
        NotesMutationRequest::Delete(args) => delete_note_from_mcp(args)?,
    };

    refresh_or_open_notes_window_after_mcp_mutation(
        result.id,
        result.open_after && !result.deleted,
        cx,
    )?;
    Ok(NotesMutationResult {
        id: result.id.as_str(),
        uri: format!("kit://notes/{}", result.id),
        title: result.title,
        deleted: result.deleted,
        permanent: result.permanent,
    })
}

struct AppliedMcpNoteMutation {
    id: NoteId,
    title: Option<String>,
    deleted: bool,
    permanent: bool,
    open_after: bool,
}

fn create_note_from_mcp(
    args: NotesCreateArgs,
) -> Result<AppliedMcpNoteMutation, NotesMutationError> {
    let id = match args.id {
        Some(id) => NoteId::parse(&id).ok_or_else(|| {
            NotesMutationError::new(
                NotesMutationErrorCode::InvalidParams,
                format!("Invalid note id: {id}"),
            )
        })?,
        None => NoteId::new(),
    };

    let body = crate::notes::metadata::merge_frontmatter(
        &args.body,
        crate::notes::metadata::MetadataFrontmatterPatch {
            tags: args.tags,
            aliases: args.aliases,
            source: args.source,
        },
    );
    validate_mcp_note_content_len(&body)?;
    let mut note = Note::with_content(body);
    note.id = id;
    if storage::get_note(id)
        .map_err(internal_notes_error)?
        .is_some()
    {
        return Err(NotesMutationError::new(
            NotesMutationErrorCode::Conflict,
            format!("Note already exists: {id}"),
        ));
    }
    if let Some(title) = args.title.filter(|title| !title.trim().is_empty()) {
        note.title = title;
    } else if note.title.trim().is_empty() {
        note.title = title_from_body(&note.content);
    }
    note.is_pinned = args.is_pinned;
    if let Some(sort_order) = args.sort_order {
        note.sort_order = sort_order;
    }
    storage::save_note(&note).map_err(internal_notes_error)?;

    Ok(AppliedMcpNoteMutation {
        id,
        title: Some(note.title),
        deleted: false,
        permanent: false,
        open_after: args.open || args.select,
    })
}

fn update_note_from_mcp(
    args: NotesUpdateArgs,
) -> Result<AppliedMcpNoteMutation, NotesMutationError> {
    let id = NoteId::parse(&args.id).ok_or_else(|| {
        NotesMutationError::new(
            NotesMutationErrorCode::InvalidParams,
            format!("Invalid note id: {}", args.id),
        )
    })?;
    let mut note = storage::get_note(id)
        .map_err(internal_notes_error)?
        .ok_or_else(|| {
            NotesMutationError::new(
                NotesMutationErrorCode::NotFound,
                format!("Note not found: {id}"),
            )
        })?;

    if let Some(body) = args.body {
        note.content = crate::notes::metadata::merge_frontmatter(
            &body,
            crate::notes::metadata::MetadataFrontmatterPatch {
                tags: args.tags.clone(),
                aliases: args.aliases.clone(),
                source: None,
            },
        );
        validate_mcp_note_content_len(&note.content)?;
        if args.title.is_none() {
            note.title = title_from_body(&note.content);
        }
    } else if !args.tags.is_empty() || !args.aliases.is_empty() {
        note.content = crate::notes::metadata::merge_frontmatter(
            &note.content,
            crate::notes::metadata::MetadataFrontmatterPatch {
                tags: args.tags.clone(),
                aliases: args.aliases.clone(),
                source: None,
            },
        );
        validate_mcp_note_content_len(&note.content)?;
    }
    if let Some(title) = args.title {
        note.title = if title.trim().is_empty() {
            title_from_body(&note.content)
        } else {
            title
        };
    }
    if let Some(is_pinned) = args.is_pinned {
        note.is_pinned = is_pinned;
    }
    if let Some(sort_order) = args.sort_order {
        note.sort_order = sort_order;
    }
    note.updated_at = chrono::Utc::now();
    note.deleted_at = None;

    storage::save_note(&note).map_err(internal_notes_error)?;

    Ok(AppliedMcpNoteMutation {
        id,
        title: Some(note.title),
        deleted: false,
        permanent: false,
        open_after: args.open || args.select,
    })
}

fn delete_note_from_mcp(
    args: NotesDeleteArgs,
) -> Result<AppliedMcpNoteMutation, NotesMutationError> {
    let id = NoteId::parse(&args.id).ok_or_else(|| {
        NotesMutationError::new(
            NotesMutationErrorCode::InvalidParams,
            format!("Invalid note id: {}", args.id),
        )
    })?;

    if args.permanent {
        if !args.confirm {
            return Err(NotesMutationError::new(
                NotesMutationErrorCode::ConfirmRequired,
                "Permanent note delete requires confirm:true",
            ));
        }
        storage::get_note(id)
            .map_err(internal_notes_error)?
            .ok_or_else(|| {
                NotesMutationError::new(
                    NotesMutationErrorCode::NotFound,
                    format!("Note not found: {id}"),
                )
            })?;
        storage::delete_note_permanently(id).map_err(internal_notes_error)?;
        return Ok(AppliedMcpNoteMutation {
            id,
            title: None,
            deleted: true,
            permanent: true,
            open_after: false,
        });
    }

    let mut note = storage::get_note(id)
        .map_err(internal_notes_error)?
        .ok_or_else(|| {
            NotesMutationError::new(
                NotesMutationErrorCode::NotFound,
                format!("Note not found: {id}"),
            )
        })?;
    let title = note.title.clone();
    note.soft_delete();
    note.updated_at = chrono::Utc::now();
    storage::save_note(&note).map_err(internal_notes_error)?;

    Ok(AppliedMcpNoteMutation {
        id,
        title: Some(title),
        deleted: true,
        permanent: false,
        open_after: false,
    })
}

fn save_open_notes_window_if_dirty(cx: &mut App) -> Result<(), NotesMutationError> {
    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app) {
        update_notes_window_detached(handle, cx, |_window, cx| {
            notes_app.update(cx, |app, _cx| app.save_current_note())
        })
        .map_err(|error| {
            NotesMutationError::new(
                NotesMutationErrorCode::Internal,
                format!("Failed to save open Notes window before MCP mutation: {error}"),
            )
        })?
        .then_some(())
        .ok_or_else(|| {
            NotesMutationError::new(
                NotesMutationErrorCode::Conflict,
                "Failed to save dirty Notes editor before MCP mutation",
            )
        })?;
    }
    Ok(())
}

fn refresh_or_open_notes_window_after_mcp_mutation(
    note_id: NoteId,
    open_or_select: bool,
    cx: &mut App,
) -> Result<(), NotesMutationError> {
    if open_or_select {
        open_note_in_notes_window(cx, note_id).map_err(internal_notes_error)?;
        return Ok(());
    }

    let existing_handle = {
        let slot = NOTES_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| *g)
    };
    let existing_app = {
        let slot = NOTES_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
        slot.lock().ok().and_then(|g| g.clone())
    };

    if let (Some(handle), Some(notes_app)) = (existing_handle, existing_app) {
        update_notes_window_detached(handle, cx, |window, cx| {
            notes_app.update(cx, |app, cx| {
                app.reload_after_external_note_mutation(note_id, window, cx)
            })
        })
        .map_err(|error| {
            NotesMutationError::new(
                NotesMutationErrorCode::Internal,
                format!("Failed to refresh Notes window after MCP mutation: {error}"),
            )
        })?
        .map_err(internal_notes_error)?;
    }

    Ok(())
}

fn title_from_body(body: &str) -> String {
    crate::notes::metadata::strip_frontmatter(body)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().trim_start_matches('#').trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Untitled Note".to_string())
}

fn validate_mcp_note_content_len(content: &str) -> Result<(), NotesMutationError> {
    if content.len() > NOTE_BODY_MAX_BYTES {
        return Err(NotesMutationError::new(
            NotesMutationErrorCode::InvalidParams,
            format!(
                "notes content exceeds max byte length of {NOTE_BODY_MAX_BYTES} after metadata merge"
            ),
        ));
    }
    Ok(())
}

fn internal_notes_error(error: impl std::fmt::Display) -> NotesMutationError {
    NotesMutationError::new(NotesMutationErrorCode::Internal, error.to_string())
}
