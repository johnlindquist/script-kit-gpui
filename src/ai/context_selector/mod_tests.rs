#[cfg(test)]
mod launcher_catalog_snapshot_tests {
    use super::*;
    use crate::spine::catalog_subsearch::ContextSubsearchSource;
    use crate::spine::SpineListAction;

    fn script(name: &str) -> Arc<crate::scripts::Script> {
        Arc::new(crate::scripts::Script {
            name: name.to_owned(),
            path: std::path::PathBuf::from(format!("/synthetic/{name}.ts")),
            plugin_id: "main".to_owned(),
            ..crate::scripts::Script::default()
        })
    }

    fn scriptlet(name: &str) -> Arc<crate::scripts::Scriptlet> {
        Arc::new(crate::scripts::Scriptlet {
            name: name.to_owned(),
            description: Some(format!("Synthetic {name}")),
            code: "// synthetic".to_owned(),
            tool: "ts".to_owned(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: "main".to_owned(),
            plugin_title: None,
            file_path: Some(format!("/synthetic/scriptlets.md#{name}")),
            command: Some(name.to_owned()),
            alias: None,
            icon: None,
        })
    }

    fn skill(name: &str) -> Arc<crate::plugins::PluginSkill> {
        Arc::new(crate::plugins::PluginSkill {
            plugin_id: "synthetic".to_owned(),
            plugin_title: "Synthetic".to_owned(),
            skill_id: name.to_owned(),
            path: std::path::PathBuf::from(format!("/synthetic/skills/{name}/SKILL.md")),
            title: name.to_owned(),
            description: format!("Synthetic {name}"),
        })
    }

    fn section(
        snapshot: &LauncherCatalogSnapshot,
        source: ContextSubsearchSource,
        query: &str,
    ) -> crate::spine::attach::ComposerSubsearchSection {
        composer_catalog_subsearch_section_from_snapshot(source, query, 3, 5..17, snapshot)
            .expect("launcher command sources produce real composer sections")
    }

    #[test]
    fn cold_launcher_catalog_is_empty_without_loading_or_discovery() {
        let store = LauncherCatalogStore::default();
        let snapshot = store.snapshot();

        assert!(snapshot.scripts.is_empty());
        assert!(snapshot.scriptlets.is_empty());
        assert!(snapshot.skills.is_empty());
        for source in [
            ContextSubsearchSource::Scripts,
            ContextSubsearchSource::Scriptlets,
            ContextSubsearchSource::Skills,
        ] {
            assert!(section(&snapshot, source, "private query").rows.is_empty());
        }
    }

    #[test]
    fn published_launcher_catalog_resolves_all_three_real_composer_families() {
        let store = LauncherCatalogStore::default();
        let published_script = script("alpha-script");
        let published_scriptlet = scriptlet("beta-scriptlet");
        let published_skill = skill("gamma-skill");
        store.publish(
            std::slice::from_ref(&published_script),
            std::slice::from_ref(&published_scriptlet),
            std::slice::from_ref(&published_skill),
        );

        let snapshot = store.snapshot();
        assert!(Arc::ptr_eq(&snapshot.scripts[0], &published_script));
        assert!(Arc::ptr_eq(&snapshot.scriptlets[0], &published_scriptlet));
        assert!(Arc::ptr_eq(&snapshot.skills[0], &published_skill));

        for (source, query, label) in [
            (
                ContextSubsearchSource::Scripts,
                "alpha-script",
                "alpha-script",
            ),
            (
                ContextSubsearchSource::Scriptlets,
                "beta-scriptlet",
                "beta-scriptlet",
            ),
            (ContextSubsearchSource::Skills, "gamma-skill", "gamma-skill"),
        ] {
            let result = section(&snapshot, source, query);
            assert_eq!(result.rows.len(), 1);
            assert_eq!(result.rows[0].row.title.as_ref(), label);
            assert!(result.rows[0].alias.is_some());
            match &result.rows[0].row.action {
                SpineListAction::ResolveSegment {
                    segment_index,
                    segment_byte_range,
                    resolution_source,
                    ..
                } => {
                    assert_eq!(*segment_index, 3);
                    assert_eq!(segment_byte_range, &(5..17));
                    assert_eq!(resolution_source.as_ref(), source.prefix());
                }
                _ => panic!("catalog row did not resolve the actual composer segment"),
            }
        }
    }

    #[test]
    fn launcher_catalog_replacement_exposes_added_edited_and_deleted_commands() {
        let store = LauncherCatalogStore::default();
        store.publish(&[script("old-script")], &[scriptlet("old-note")], &[]);
        let previous = store.snapshot();

        store.publish(
            &[script("new-script")],
            &[scriptlet("edited-note")],
            &[skill("new-skill")],
        );
        let current = store.snapshot();

        assert_eq!(previous.scripts[0].name, "old-script");
        assert_eq!(previous.scriptlets[0].name, "old-note");
        assert!(previous.skills.is_empty());
        assert!(
            section(&current, ContextSubsearchSource::Scripts, "old-script")
                .rows
                .is_empty()
        );
        assert_eq!(
            section(&current, ContextSubsearchSource::Scripts, "new-script")
                .rows
                .len(),
            1
        );
        assert!(
            section(&current, ContextSubsearchSource::Scriptlets, "old-note")
                .rows
                .is_empty()
        );
        assert_eq!(
            section(&current, ContextSubsearchSource::Scriptlets, "edited-note")
                .rows
                .len(),
            1
        );
        assert_eq!(
            section(&current, ContextSubsearchSource::Skills, "new-skill")
                .rows
                .len(),
            1
        );
    }

    #[test]
    fn inline_portals_follow_host_replacements_instead_of_process_lifetime_file_caches() {
        let store = LauncherCatalogStore::default();
        store.publish(
            &[script("old-script")],
            &[scriptlet("old-note")],
            &[skill("old-skill")],
        );
        let previous = store.snapshot();
        store.publish(
            &[script("new-script")],
            &[scriptlet("new-note")],
            &[skill("new-skill")],
        );
        let current = store.snapshot();
        for (kind, old_label, current_label) in [
            (ContextPortalKind::ScriptSearch, "old-script", "new-script"),
            (ContextPortalKind::ScriptletSearch, "old-note", "new-note"),
            (ContextPortalKind::SkillSearch, "old-skill", "new-skill"),
        ] {
            let query = InlinePortalQuery {
                kind,
                prefix: portal_prefix_for_kind(kind),
                query: String::new(),
            };
            let mut old_rows = Vec::new();
            collect_script_list_inline_items_from_snapshot(&query, &mut old_rows, &previous);
            assert_eq!(old_rows.len(), 1);
            assert_eq!(old_rows[0].label.as_ref(), old_label);
            let mut rows = Vec::new();
            collect_script_list_inline_items_from_snapshot(&query, &mut rows, &current);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].label.as_ref(), current_label);
        }
    }

    #[test]
    fn duplicate_launcher_skill_labels_attach_the_exact_selected_owner() {
        let first = Arc::new(crate::plugins::PluginSkill {
            plugin_id: "first-owner".to_owned(),
            plugin_title: "First".to_owned(),
            skill_id: "deploy".to_owned(),
            path: std::path::PathBuf::from("/synthetic/first/deploy/SKILL.md"),
            title: "Deploy".to_owned(),
            description: String::new(),
        });
        let second = Arc::new(crate::plugins::PluginSkill {
            plugin_id: "second-owner".to_owned(),
            plugin_title: "Second".to_owned(),
            skill_id: "deploy".to_owned(),
            path: std::path::PathBuf::from("/synthetic/second/deploy/SKILL.md"),
            title: "Deploy".to_owned(),
            description: String::new(),
        });
        let store = LauncherCatalogStore::default();
        store.publish(&[], &[], &[first, second]);
        let snapshot = store.snapshot();
        let projected = section(&snapshot, ContextSubsearchSource::Skills, "deploy");

        assert_eq!(projected.rows.len(), 2);
        for expected_path in [
            "/synthetic/first/deploy/SKILL.md",
            "/synthetic/second/deploy/SKILL.md",
        ] {
            let expected_identity = format!("skills/{expected_path}");
            let alias = crate::spine::attach::composer_subsearch_alias_for_resolution(
                section(&snapshot, ContextSubsearchSource::Skills, "deploy"),
                "@skills:Deploy",
                &expected_identity,
            )
            .expect("duplicate labels must resolve by their complete canonical identity");
            match alias {
                crate::ai::message_parts::AiContextPart::FilePath { path, .. } => {
                    assert_eq!(path, expected_path)
                }
                _ => panic!("skill attachment did not retain its owning file"),
            }
        }
        assert!(
            crate::spine::attach::composer_subsearch_alias_for_resolution(
                section(&snapshot, ContextSubsearchSource::Skills, "deploy"),
                "@skills:Deploy",
                "skills//synthetic/unknown/deploy/SKILL.md",
            )
            .is_none()
        );
    }

    #[test]
    fn launcher_catalog_ranking_keeps_existing_canonical_slash_tie_break() {
        let forward =
            slash_command_rows_with_descriptions("", [("zeta", "last"), ("alpha", "first")]);
        let reverse =
            slash_command_rows_with_descriptions("", [("alpha", "first"), ("zeta", "last")]);

        let forward_ids: Vec<&str> = forward.iter().map(|row| row.id.as_ref()).collect();
        let reverse_ids: Vec<&str> = reverse.iter().map(|row| row.id.as_ref()).collect();
        assert_eq!(forward_ids, reverse_ids);
        assert_eq!(forward[0].label.as_ref(), "alpha");
        assert_eq!(forward[1].label.as_ref(), "zeta");
    }
}
