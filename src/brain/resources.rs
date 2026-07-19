//! `kit://brain` MCP resources: the brain's transparency and agent-access
//! surface.
//!
//! - `kit://brain` — status + health: doc/embedding/signal counts, per-source
//!   breakdown, model and helper presence, last index/curator/prune runs.
//! - `kit://brain/recall?q=...` — hybrid retrieval rendered as a context
//!   block (what Agent Chat stages per turn, available to any MCP consumer).
//!   Add `format=json` for qmd-style source refs.
//! - `kit://brain/doc?source=...&sourceId=...` — retrieve one indexed brain
//!   document, optionally with `lines=start-end`.
//! - `kit://brain/docs?refs=source:sourceId,source:sourceId` — retrieve a
//!   batch of indexed brain documents while preserving request order.
//! - `kit://brain/focus[?refresh=1]` — the curator's latest focus review.
//! - `kit://brain/signals?limit=...` — recent attention signals ("why does
//!   the brain think I care about X").

use super::store::{self, BrainDoc, DocSource};

pub const BRAIN_RESOURCE_URI: &str = "kit://brain";

pub fn is_brain_resource_uri(uri: &str) -> bool {
    uri == BRAIN_RESOURCE_URI || uri.starts_with("kit://brain?") || uri.starts_with("kit://brain/")
}

fn query_param(uri: &str, key: &str) -> Option<String> {
    let query = uri.split_once('?')?.1;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == key {
            let decoded = v.replace('+', " ");
            let decoded = percent_decode(&decoded);
            return Some(decoded);
        }
    }
    None
}

/// Minimal percent-decoding (enough for q= text); invalid sequences pass through.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        // Decode over raw bytes, never `&value[i + 1..i + 3]`: a `%` followed by a
        // multibyte UTF-8 char put `i + 3` mid-char and panicked on the `str` slice.
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Value of a single ASCII hex digit (case-insensitive), or `None` if not hex.
fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_source(value: &str) -> Result<DocSource, String> {
    DocSource::parse(value).ok_or_else(|| format!("unknown brain source: {value}"))
}

fn parse_lines(value: &str) -> Option<(usize, usize)> {
    let (start, end) = value.split_once('-').unwrap_or((value, value));
    let start = start.trim().parse::<usize>().ok()?;
    let end = end.trim().parse::<usize>().ok()?;
    if start == 0 || end < start {
        return None;
    }
    Some((start, end))
}

fn lines_param(uri: &str) -> Result<Option<(usize, usize)>, String> {
    match query_param(uri, "lines") {
        Some(value) => parse_lines(&value).map(Some).ok_or_else(|| {
            format!(
                "invalid kit://brain/doc lines parameter: {value}; expected start-end with start >= 1 and end >= start"
            )
        }),
        None => Ok(None),
    }
}

fn line_slice(content: &str, range: Option<(usize, usize)>) -> String {
    let Some((start, end)) = range else {
        return content.to_string();
    };
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            (line_number >= start && line_number <= end).then_some(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn brain_doc_json(doc: &BrainDoc, lines: Option<(usize, usize)>) -> serde_json::Value {
    let source_ref = super::search::source_ref_for_doc(doc);
    let content = line_slice(&doc.content, lines);
    serde_json::json!({
        "source": doc.source.as_str(),
        "sourceId": &doc.source_id,
        "title": &doc.title,
        "updatedAt": doc.updated_at,
        "citationUri": source_ref.citation_uri,
        "canonicalPath": source_ref.canonical_path,
        "lineStart": lines.map(|(start, _)| start).or(source_ref.line_start),
        "lineEnd": lines.map(|(_, end)| end).or(source_ref.line_end),
        "content": content,
    })
}

fn read_brain_doc_resource(uri: &str) -> Result<(String, String), String> {
    let source = query_param(uri, "source").ok_or("kit://brain/doc requires ?source=<source>")?;
    let source_id =
        query_param(uri, "sourceId").ok_or("kit://brain/doc requires ?sourceId=<source_id>")?;
    let source = parse_source(&source)?;
    let lines = lines_param(uri)?;
    let doc = store::get_doc(source, &source_id)
        .map_err(|error| format!("brain doc read failed: {error}"))?
        .ok_or_else(|| {
            format!(
                "Brain document not found: {}:{}",
                source.as_str(),
                source_id
            )
        })?;
    if query_param(uri, "format").as_deref() == Some("json") {
        let body = serde_json::json!({
            "schemaVersion": 1,
            "found": true,
            "doc": brain_doc_json(&doc, lines),
        });
        return Ok(("application/json".to_string(), body.to_string()));
    }
    let content = line_slice(&doc.content, lines);
    let body = format!(
        "# {}\n\nSource: brain://{}/{}\nUpdated: {}\n\n{}",
        if doc.title.trim().is_empty() {
            "(untitled)"
        } else {
            doc.title.trim()
        },
        doc.source.as_str(),
        doc.source_id,
        doc.updated_at,
        content
    );
    Ok(("text/markdown".to_string(), body))
}

fn parse_doc_ref(value: &str) -> Result<(DocSource, String), String> {
    let (source, source_id) = value
        .split_once(':')
        .ok_or_else(|| format!("invalid brain doc ref: {value}"))?;
    Ok((parse_source(source)?, source_id.to_string()))
}

fn read_brain_docs_resource(uri: &str) -> Result<(String, String), String> {
    let refs = query_param(uri, "refs").ok_or("kit://brain/docs requires ?refs=<refs>")?;
    let docs = refs
        .split(',')
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            let raw = value.trim();
            match parse_doc_ref(raw) {
                Ok((source, source_id)) => match store::get_doc(source, &source_id) {
                    Ok(Some(doc)) => serde_json::json!({
                        "ref": raw,
                        "found": true,
                        "doc": brain_doc_json(&doc, None),
                    }),
                    Ok(None) => serde_json::json!({
                        "ref": raw,
                        "found": false,
                        "error": "not_found",
                    }),
                    Err(error) => serde_json::json!({
                        "ref": raw,
                        "found": false,
                        "error": error.to_string(),
                    }),
                },
                Err(error) => serde_json::json!({
                    "ref": raw,
                    "found": false,
                    "error": error,
                }),
            }
        })
        .collect::<Vec<_>>();
    let body = serde_json::json!({
        "schemaVersion": 1,
        "docs": docs,
    });
    Ok(("application/json".to_string(), body.to_string()))
}

/// Resolve a brain resource URI to (mime_type, body).
pub fn read_brain_resource(uri: &str) -> Result<(String, String), String> {
    store::init_brain_db().map_err(|error| format!("brain init failed: {error}"))?;
    if uri == BRAIN_RESOURCE_URI || uri.starts_with("kit://brain?") {
        let (docs, embedded, signals) =
            store::doc_stats().map_err(|error| format!("brain stats failed: {error}"))?;
        let model = super::embedder::resolve_embed_model();
        let docs_by_source: serde_json::Map<String, serde_json::Value> = store::source_counts()
            .unwrap_or_default()
            .into_iter()
            .map(|(source, count)| (source, serde_json::json!(count)))
            .collect();
        // Meta timestamps double as health checks: a stale lastIndexCycle
        // means the indexer thread died or the app hasn't run lately.
        let meta_ts = |key: &str| -> serde_json::Value {
            store::meta_get(key)
                .ok()
                .flatten()
                .and_then(|value| value.parse::<i64>().ok())
                .map(|ts| serde_json::json!(ts))
                .unwrap_or(serde_json::Value::Null)
        };
        // BrainHealth snapshot recorded by the indexer after every cycle, so
        // swallowed failures surface here. Null until the first cycle runs.
        let health = super::health::read_health()
            .and_then(|health| serde_json::to_value(health).ok())
            .unwrap_or(serde_json::Value::Null);
        let body = serde_json::json!({
            "schemaVersion": 1,
            "docs": docs,
            "docsBySource": docs_by_source,
            "embedded": embedded,
            "signals": signals,
            "health": health,
            "semanticSearch": model.is_some(),
            "embedModel": model.map(|m| m.model_id),
            "embedHelperFound": super::embedder::helper_available(),
            "lastIndexCycle": meta_ts("last_index_cycle"),
            "lastCuratorRun": meta_ts("curator_last_run"),
            "lastAmbientPrune": meta_ts("ambient_prune_last"),
            "lastModelDownloadAttempt": meta_ts("embed_model_download_attempt"),
            "ftsVersion": store::meta_get("fts_version").ok().flatten(),
            "dbSizeBytes": store::db_size_bytes(),
            "canonicalRoots": {
                "brain": "~/.scriptkit/brain",
                "days": "~/.scriptkit/brain/days",
                "fragments": "~/.scriptkit/brain/fragments",
                "notes": "~/.scriptkit/brain/notes",
                "trash": "~/.scriptkit/brain/trash",
            },
            "indexStore": "~/.scriptkit/db/brain.sqlite",
            "store": "~/.scriptkit/db/brain.sqlite",
        });
        return Ok(("application/json".to_string(), body.to_string()));
    }
    if uri.starts_with("kit://brain/recall") {
        let query = query_param(uri, "q").unwrap_or_default();
        if query.trim().is_empty() {
            return Err("kit://brain/recall requires ?q=<query>".to_string());
        }
        super::record_ask_signals(&query);
        if query_param(uri, "format").as_deref() == Some("json") {
            let refresh_start = std::time::Instant::now();
            let source_sync = super::indexer::sync_file_sources_for_recall();
            tracing::info!(
                target: "script_kit::brain",
                event = "brain_recall_file_sources_synced",
                query_len = query.chars().count(),
                notes = source_sync.notes,
                day_pages = source_sync.day_pages,
                fragments = source_sync.fragments,
                failed_sources = ?source_sync.failed_sources,
                elapsed_ms = refresh_start.elapsed().as_secs_f64() * 1000.0,
            );
            let query_embedding = super::indexer::embed_query_within_budget(&query);
            let hits = match &query_embedding {
                Some((model_id, vector)) => super::brain_search(
                    &query,
                    Some(vector),
                    Some(model_id),
                    super::BRAIN_CONTEXT_HITS,
                ),
                None => super::brain_search(&query, None, None, super::BRAIN_CONTEXT_HITS),
            }
            .map_err(|error| format!("brain recall failed: {error}"))?;
            let body = super::search::recall_hits_json(&query, &hits);
            return Ok(("application/json".to_string(), body.to_string()));
        }
        let block = super::recall_context_block(&query)
            .map_err(|error| format!("brain recall failed: {error}"))?
            .unwrap_or_else(|| "(no relevant memories)".to_string());
        return Ok(("text/markdown".to_string(), block));
    }
    if uri.starts_with("kit://brain/docs") {
        return read_brain_docs_resource(uri);
    }
    if uri.starts_with("kit://brain/doc") {
        return read_brain_doc_resource(uri);
    }
    if uri.starts_with("kit://brain/focus") {
        if query_param(uri, "refresh").as_deref() == Some("1") {
            super::curator::run_focus_review()
                .map_err(|error| format!("focus review failed: {error}"))?;
        }
        let review = store::get_doc(super::store::DocSource::Activity, "focus-review")
            .map_err(|error| format!("focus read failed: {error}"))?;
        let body = review.map(|doc| doc.content).unwrap_or_else(|| {
            "(no focus review yet — add ?refresh=1 to generate one)".to_string()
        });
        return Ok(("text/markdown".to_string(), body));
    }
    if uri.starts_with("kit://brain/signals") {
        let limit = query_param(uri, "limit")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(50)
            .min(500);
        let signals = store::recent_signals(limit)
            .map_err(|error| format!("brain signals failed: {error}"))?;
        let body = serde_json::json!(signals
            .iter()
            .map(|s| {
                serde_json::json!({
                    "topic": s.topic,
                    "weight": s.weight,
                    "source": s.source,
                    "createdAt": s.created_at,
                })
            })
            .collect::<Vec<_>>());
        return Ok(("application/json".to_string(), body.to_string()));
    }
    Err(format!("Brain resource not found: {uri}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_matching() {
        assert!(is_brain_resource_uri("kit://brain"));
        assert!(is_brain_resource_uri("kit://brain/recall?q=hello"));
        assert!(is_brain_resource_uri("kit://brain/signals?limit=10"));
        assert!(!is_brain_resource_uri("kit://notes"));
    }

    #[test]
    fn query_param_decoding() {
        assert_eq!(
            query_param("kit://brain/recall?q=hello+world", "q").as_deref(),
            Some("hello world")
        );
        assert_eq!(
            query_param("kit://brain/recall?q=a%20b%26c", "q").as_deref(),
            Some("a b&c")
        );
        assert_eq!(query_param("kit://brain/recall", "q"), None);
    }

    #[test]
    fn percent_decode_survives_multibyte_after_percent() {
        // Regression: `&value[i + 1..i + 3]` was byte-sliced after only a
        // byte-length guard (`i + 2 < bytes.len()`). A `%` immediately followed by
        // a 3+ byte UTF-8 char (e.g. `€` = E2 82 AC) makes `i + 3` land mid-char and
        // panicked ("byte index is not a char boundary"). An invalid escape must
        // pass through untouched instead of crashing.
        assert_eq!(percent_decode("%€"), "%€");
        assert_eq!(percent_decode("a%€b"), "a%€b");
        assert_eq!(percent_decode("%🌊%20x"), "%🌊 x");
        // Valid escapes still decode (case-insensitive), unchanged behavior.
        assert_eq!(percent_decode("a%20b%26c"), "a b&c");
        assert_eq!(percent_decode("%2f%2F"), "//");
        // Reached through the real MCP resource query-param path.
        assert_eq!(
            query_param("kit://brain/recall?q=%€", "q").as_deref(),
            Some("%€")
        );
    }
}
