use super::fixture_ids::{DAY_PAGE_FIXTURE_IDS, NOTES_AUXILIARY_FIXTURE_IDS, NOTES_FIXTURE_IDS};
use anyhow::{Context as _, Result};
use gpui::{AnyWindowHandle, App, AppContext, Entity, Window};

use crate::runtime_policy::{owned_evaluation, WindowHostPolicy};

fn fixture_now() -> Result<chrono::DateTime<chrono::Utc>> {
    Ok(chrono::DateTime::parse_from_rfc3339("2026-08-28T12:00:00Z")?.with_timezone(&chrono::Utc))
}

/// Seeds canonical markdown + the real SQLite index, never an in-memory stand-in.
/// Existing documents survive reopen so save/reload remains observable.
pub(crate) fn prepare_notes_storage() -> Result<()> {
    let policy = owned_evaluation().context("Notes fixtures require an owned evaluation")?;
    policy.require_owned_path(&policy.root().join("notes"))?;
    crate::notes::init_notes_db()?;
    let now = fixture_now()?;
    for (id, title, content) in [
        ("d0197594-1111-4000-8000-000000000001", "Fixture Alpha", "# Fixture Alpha\n\n- [ ] Review the document\n- [x] Keep the completed task\n\n[Reference](https://example.invalid/reference)\n"),
        ("d0197594-1111-4000-8000-000000000002", "Fixture Beta", "# Fixture Beta\n\nSecond document for search and switcher.\n"),
    ] {
        let id = crate::notes::NoteId::parse(id).context("invalid fixture note identity")?;
        if crate::notes::get_note(id)?.is_none() {
            crate::notes::save_note(&crate::notes::Note {
                id, title: title.into(), content: content.into(), created_at: now,
                updated_at: now, deleted_at: None, is_pinned: title == "Fixture Alpha", sort_order: 0,
            })?;
            crate::runtime_policy::record_completed_fixture_effect();
        }
    }
    // Both recent switchers search the canonical Notes/day corpus. Seed the
    // same actual Day document used by the Day root before either host opens.
    let substrate = script_kit_gpui::brain::substrate::BrainSubstrate::default_kit();
    policy.require_owned_path(substrate.paths().base())?;
    let mut session = script_kit_gpui::day_page::DayPageDocumentSession::new(substrate.clone());
    session.bind_today(now)?;
    if session.disk_content().is_empty() {
        session.apply_editor_content("# Today\n\n- [ ] Review the document\n\n09:15 [Fixture fragment](../fragments/fixture-fragment.md)\n09:20 [Clipboard entry](kit://clipboard-history?id=fixture-entry)\n");
        session.save(now)?;
    }
    let fragment = substrate
        .paths()
        .fragments_dir()
        .join("fixture-fragment.md");
    policy.require_owned_path(&fragment)?;
    if !fragment.exists() {
        crate::atomic_file::ensure_private_directory(&substrate.paths().fragments_dir())?;
        crate::brain::substrate::io::atomic_write(
            &fragment,
            "# Fixture fragment\n\nA source-line return target.\n",
        )?;
        crate::runtime_policy::record_completed_fixture_effect();
    }
    Ok(())
}

pub(crate) fn create_notes_fixture(
    fixture_id: &str,
    window: &mut Window,
    cx: &mut App,
) -> Result<Entity<crate::notes::NotesApp>> {
    anyhow::ensure!(
        NOTES_FIXTURE_IDS.contains(&fixture_id)
            || NOTES_AUXILIARY_FIXTURE_IDS.contains(&fixture_id),
        "unknown_notes_fixture"
    );
    prepare_notes_storage()?;
    let initial = crate::notes::window::init::NotesInitialData {
        notes: crate::notes::get_all_notes()?,
        deleted_notes: crate::notes::get_deleted_notes()?,
        host_policy: WindowHostPolicy::OwnedHidden,
        ghost_clipboard: Some(vec![crate::notes::ghost::NotesGhostClipboardText {
            text: "Review the document before saving the fixture".into(),
        }]),
        now: Some(fixture_now()?),
    };
    Ok(cx.new(|cx| crate::notes::NotesApp::from_initial_data(initial, window, cx)))
}

/// DayPageView remains a child of the actual ScriptListApp; Main installs the
/// returned entity in AppView::DayPage, retaining the real Day footer owner.
pub(crate) fn create_day_page_fixture(
    fixture_id: &str,
    app: Entity<crate::ScriptListApp>,
    window: &mut Window,
    cx: &mut App,
) -> Result<Entity<crate::DayPageView>> {
    anyhow::ensure!(
        DAY_PAGE_FIXTURE_IDS.contains(&fixture_id),
        "unknown_day_page_fixture"
    );
    prepare_notes_storage()?;
    let substrate = script_kit_gpui::brain::substrate::BrainSubstrate::default_kit();
    let now = fixture_now()?;
    let mut session = script_kit_gpui::day_page::DayPageDocumentSession::new(substrate.clone());
    session.bind_today(now)?;
    let fragment = substrate
        .paths()
        .fragments_dir()
        .join("fixture-fragment.md");
    if fixture_id == "day-page.fragment" {
        session.bind_fragment(fragment, now)?;
    }
    let initial = crate::DayPageInitialData {
        session,
        now: Some(now),
        shelf_previews: Some(std::collections::HashMap::from([(
            "fixture-entry".into(),
            "Owned clipboard shelf fixture".into(),
        )])),
        host_policy: WindowHostPolicy::OwnedHidden,
    };
    Ok(cx.new(|cx| {
        let mut view = crate::DayPageView::from_initial_data(app, initial, window, cx);
        view.clipboard_shelf_expanded = fixture_id == "day-page.shelf";
        view
    }))
}

fn require_fixture_parent(
    parent: &crate::protocol::AutomationWindowInfo,
    handle: AnyWindowHandle,
) -> Result<u64> {
    WindowHostPolicy::OwnedHidden.validate()?;
    let generation = parent
        .generation
        .filter(|generation| *generation != 0)
        .context("fixture_parent_generation_missing")?;
    anyhow::ensure!(
        crate::windows::get_runtime_window_handle_for_generation(&parent.id, generation)
            == Some(handle),
        "fixture_parent_stale"
    );
    anyhow::ensure!(
        crate::windows::runtime_window_host_policy(&parent.id, generation)?
            == WindowHostPolicy::OwnedHidden,
        "fixture_parent_not_owned"
    );
    Ok(generation)
}

fn mounted_command_bar_target(
    parent: &crate::protocol::AutomationWindowInfo,
    dialog: Entity<crate::actions::ActionsDialog>,
    cx: &App,
) -> Result<crate::protocol::AutomationWindowInfo> {
    crate::windows::list_automation_windows()
        .into_iter()
        .find(|child| {
            child.parent_window_id.as_deref() == Some(parent.id.as_str())
                && child.parent_window_generation == parent.generation
                && crate::windows::automation_surface_collector::exact_actions_dialog_entity(
                    child, cx,
                )
                .is_some_and(|actual| actual.entity_id() == dialog.entity_id())
        })
        .context("fixture_command_bar_not_registered")
}

fn require_notes_fixture_parent(
    entity: &Entity<crate::notes::NotesApp>,
    parent: &crate::protocol::AutomationWindowInfo,
    handle: AnyWindowHandle,
    cx: &App,
) -> Result<()> {
    let generation = require_fixture_parent(parent, handle)?;
    anyhow::ensure!(parent.id == "notes", "fixture_parent_is_not_notes");
    let (actual, actual_handle) =
        crate::notes::get_notes_app_entity_and_handle_for_generation(generation, cx)
            .context("fixture_notes_owner_stale")?;
    anyhow::ensure!(
        actual.entity_id() == entity.entity_id() && AnyWindowHandle::from(actual_handle) == handle,
        "fixture_notes_owner_mismatch"
    );
    Ok(())
}

/// Called only after the exact hidden Notes Root is registered and bound.
/// Detached bars return their real registered child, not the parent descriptor.
pub(crate) fn mount_notes_fixture_presentation(
    fixture_id: &str,
    entity: &Entity<crate::notes::NotesApp>,
    parent: &crate::protocol::AutomationWindowInfo,
    parent_handle: AnyWindowHandle,
    cx: &mut App,
) -> Result<crate::protocol::AutomationWindowInfo> {
    use gpui::{ParentElement as _, Styled as _};
    use gpui_component::{Root, WindowExt as _};
    anyhow::ensure!(
        NOTES_AUXILIARY_FIXTURE_IDS.contains(&fixture_id),
        "unknown_notes_auxiliary_fixture"
    );
    require_notes_fixture_parent(entity, parent, parent_handle, cx)?;
    let dialog = parent_handle.update(cx, |_, window, cx| -> Result<_> {
        match fixture_id {
            "notes.actions" => {
                entity.update(cx, |notes, cx| notes.open_actions_panel(window, cx));
                Ok(Some(
                    entity
                        .read(cx)
                        .actions_dialog()
                        .context("notes_actions_not_open")?,
                ))
            }
            "notes.recent-switcher" => {
                entity.update(cx, |notes, cx| notes.open_browse_panel(window, cx));
                Ok(Some(
                    entity
                        .read(cx)
                        .note_switcher_dialog()
                        .context("notes_recent_switcher_not_open")?,
                ))
            }
            "notes.root-dialog" => {
                window.open_dialog(cx, |dialog, _, _| {
                    dialog
                        .title("Review note")
                        .child("Dismiss this dialog to continue editing the owned note.")
                        .confirm()
                });
                anyhow::ensure!(
                    !Root::read(window, cx).layer_snapshot(cx).dialogs.is_empty(),
                    "notes_root_dialog_not_mounted"
                );
                Ok(None)
            }
            "notes.root-notification" => {
                let background = crate::ui_foundation::get_vibrancy_surface_background(0.55);
                window.push_notification(
                    gpui_component::notification::Notification::success("Fixture note saved")
                        .bg(background),
                    cx,
                );
                anyhow::ensure!(
                    !Root::read(window, cx)
                        .layer_snapshot(cx)
                        .notifications
                        .is_empty(),
                    "notes_root_notification_not_mounted"
                );
                Ok(None)
            }
            _ => unreachable!("closed fixture identifier checked above"),
        }
    })??;
    match dialog {
        Some(dialog) => mounted_command_bar_target(parent, dialog, cx),
        None => Ok(parent.clone()),
    }
}

pub(crate) fn observe_notes_fixture_presentation(
    entity: &Entity<crate::notes::NotesApp>,
    parent: &crate::protocol::AutomationWindowInfo,
    parent_handle: AnyWindowHandle,
    cx: &App,
) -> Result<serde_json::Value> {
    require_notes_fixture_parent(entity, parent, parent_handle, cx)?;
    parent_handle.read(cx, |root: Entity<gpui_component::Root>, cx| {
        serde_json::json!({
            "notes": entity.read(cx).automation_state(cx),
            "rootLayers": root.read(cx).layer_snapshot(cx),
        })
    })
}

/// Programmatic dismissal through the actual layer owner, not a fabricated
/// completed flag. The evaluator applies its surface-revision guard first.
pub(crate) fn dismiss_notes_fixture_layer(
    fixture_id: &str,
    entity: &Entity<crate::notes::NotesApp>,
    parent: &crate::protocol::AutomationWindowInfo,
    parent_handle: AnyWindowHandle,
    cx: &mut App,
) -> Result<serde_json::Value> {
    require_notes_fixture_parent(entity, parent, parent_handle, cx)?;
    parent_handle.update(cx, |_, window, cx| -> Result<()> {
        let snapshot = gpui_component::Root::read(window, cx).layer_snapshot(cx);
        let dismissed = match fixture_id {
            "notes.root-dialog" => gpui_component::Root::close_dialog_if_current(
                *snapshot
                    .dialogs
                    .last()
                    .context("notes_root_dialog_closed")?,
                window,
                cx,
            ),
            "notes.root-notification" => gpui_component::Root::dismiss_notification_if_current(
                snapshot
                    .notifications
                    .first()
                    .context("notes_root_notification_closed")?
                    .entity_id,
                window,
                cx,
            ),
            _ => anyhow::bail!("fixture_is_not_a_notes_root_layer"),
        };
        anyhow::ensure!(dismissed, "notes_root_layer_stale_or_closing");
        crate::runtime_policy::record_completed_fixture_effect();
        Ok(())
    })??;
    observe_notes_fixture_presentation(entity, parent, parent_handle, cx)
}

pub(crate) fn mount_day_page_fixture_presentation(
    fixture_id: &str,
    entity: &Entity<crate::DayPageView>,
    parent: &crate::protocol::AutomationWindowInfo,
    parent_handle: AnyWindowHandle,
    cx: &mut App,
) -> Result<crate::protocol::AutomationWindowInfo> {
    anyhow::ensure!(
        fixture_id == "day-page.switcher",
        "unknown_day_page_auxiliary_fixture"
    );
    require_fixture_parent(parent, parent_handle)?;
    let app = entity
        .read(cx)
        .app
        .upgrade()
        .context("day_fixture_app_released")?;
    anyhow::ensure!(
        parent_handle.read(cx, |root: Entity<gpui_component::Root>, cx| root
            .read(cx)
            .view()
            .entity_id()
            == app.entity_id())?,
        "day_fixture_root_mismatch"
    );
    anyhow::ensure!(
        matches!(&app.read(cx).current_view, crate::AppView::DayPage { entity: current } if current.entity_id() == entity.entity_id()),
        "day_fixture_owner_not_installed"
    );
    parent_handle.update(cx, |_, window, cx| {
        entity.update(cx, |day, cx| day.open_day_switcher(window, cx))
    })?;
    let dialog = entity
        .read(cx)
        .note_switcher
        .dialog()
        .cloned()
        .context("day_fixture_switcher_not_open")?;
    mounted_command_bar_target(parent, dialog, cx)
}
