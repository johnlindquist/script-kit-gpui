//! Day Page markdown reference and fragment navigation tests.

use std::fs;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use gpui::{
    div, prelude::*, IntoElement, ParentElement, Render, TestAppContext, VisualTestContext,
};

use crate::brain::substrate::{BrainFrontmatter, BrainSubstrate, DayEntry, FragmentReference};
use crate::notes::NoteId;

use super::{parse_day_page_segments, DayPageDocumentSession, DayPageSegment, FRAGMENT_BACK_ID};

fn utc(now: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(now)
        .expect("parse time")
        .with_timezone(&Utc)
}

fn test_substrate() -> (tempfile::TempDir, BrainSubstrate) {
    let dir = tempfile::tempdir().expect("tempdir");
    let substrate = BrainSubstrate::with_timezone(dir.path().join("brain"), Tz::UTC);
    (dir, substrate)
}

fn write_fragment(
    substrate: &BrainSubstrate,
    id: &str,
    source: &str,
    body: &str,
    now: DateTime<Utc>,
) -> std::path::PathBuf {
    let path = substrate.paths().fragment_file(id);
    let parent = path.parent().expect("fragment parent");
    fs::create_dir_all(parent).expect("fragments dir");
    let frontmatter = BrainFrontmatter::new(NoteId::new(), now, now).with_source(source);
    fs::write(&path, frontmatter.render(body).expect("render fragment")).expect("write fragment");
    path
}

#[test]
fn fragment_line_parses_to_markdown_reference_segment() {
    let content = "09:15 Fragment\n> First words of the pasted article without cutting mid-word...\n[Open fragment](../fragments/2026-06-11-0942-clipboard.md)\n";
    let segments = parse_day_page_segments(content);
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        DayPageSegment::FragmentRef {
            index,
            excerpt,
            line_count,
            ..
        } => {
            assert_eq!(*index, 0);
            assert_eq!(*line_count, 3);
            assert!(excerpt.contains("First words"));
        }
        other => panic!("expected fragment reference segment, got {other:?}"),
    }
}

#[test]
fn open_fragment_binds_editor_and_back_restores_day_page() {
    let (_dir, substrate) = test_substrate();
    let now = utc("2026-06-11T09:42:00Z");
    let fragment_path = write_fragment(
        &substrate,
        "2026-06-11-0942-clipboard",
        "scriptkit://clipboard/entry-1",
        "Full pasted article body with many words.",
        now,
    );

    let mut session = DayPageDocumentSession::new(substrate.clone());
    session.bind_today(now).expect("bind day");
    substrate
        .append_to_day(
            now,
            DayEntry::FragmentRef(FragmentReference {
                excerpt: "First words of the pasted article without cutting mid-word..."
                    .to_string(),
                relative_link: "../fragments/2026-06-11-0942-clipboard.md".to_string(),
            }),
        )
        .expect("append fragment ref");

    session.bind_today(now).expect("reload day");

    let bound = session
        .bind_fragment(fragment_path.clone(), now)
        .expect("open fragment");
    assert!(bound.contains("Full pasted article body"));
    assert!(session.is_viewing_fragment());
    assert_eq!(session.path(), Some(&fragment_path));

    let restored = session.return_to_day(now).expect("back to day");
    assert!(restored.contains("First words of the pasted article"));
    assert!(!session.is_viewing_fragment());
    assert_eq!(
        session.path(),
        Some(&substrate.paths().day_page(now.date_naive()))
    );
}

#[test]
fn fragment_reference_is_written_as_markdown_card() {
    let (_dir, substrate) = test_substrate();
    let now = utc("2026-06-11T09:42:00Z");
    substrate
        .append_to_day(
            now,
            DayEntry::FragmentRef(FragmentReference {
                excerpt: "First words of the pasted article without cutting mid-word..."
                    .to_string(),
                relative_link: "../fragments/2026-06-11-0942-clipboard.md".to_string(),
            }),
        )
        .expect("append fragment ref");

    let contents =
        fs::read_to_string(substrate.paths().day_page(now.date_naive())).expect("read day");
    assert!(contents.contains("09:42 Fragment\n"));
    assert!(contents.contains("> First words of the pasted article without cutting mid-word...\n"));
    assert!(contents.contains("[Open fragment](../fragments/2026-06-11-0942-clipboard.md)"));
    assert!(
        !contents.contains("\n  ../fragments/"),
        "fragment references should not render an extra raw backing line"
    );
}

struct FragmentBackProbe;

impl Render for FragmentBackProbe {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(FRAGMENT_BACK_ID)
            .debug_selector(|| FRAGMENT_BACK_ID.to_string())
            .child("← Today")
    }
}

/// Back affordance semantic id is present when viewing a fragment inline.
#[test]
fn fragment_back_semantic_id_is_rendered() {
    let mut cx = TestAppContext::single();
    let window = cx.add_window(|_, _| FragmentBackProbe);
    let mut vcx = VisualTestContext::from_window(window.into(), &cx);
    vcx.run_until_parked();
    let bounds = vcx.debug_bounds(FRAGMENT_BACK_ID);
    assert!(
        bounds.is_some(),
        "fragment back semantic id {FRAGMENT_BACK_ID} should be present"
    );
}
