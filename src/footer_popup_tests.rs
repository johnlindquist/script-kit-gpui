mod footer_layout_tests {
    use super::{
        footer_active_dot_hex, footer_dot_hex, footer_hint_content_layout,
        footer_hint_content_layout_for_button, footer_hint_item_gap, footer_hint_label_widths,
        footer_hint_legacy_extra_padding, footer_hint_max_item_width, footer_hint_slot_width,
        footer_identifier_uses_keycap_border, main_window_detached_footer_regions_appkit,
        main_window_detached_footer_regions_gpui, native_footer_left_hit_target_flags,
        native_footer_visual_event_changed, native_footer_visual_root_state,
        resolved_native_footer_button_state, should_use_gpui_footer_overlay, FooterAction,
        FooterButtonConfig, FooterDotStatus, NativeFooterLeftHitTargetFlags,
        FOOTER_HINT_KEY_LABEL_GAP, FOOTER_HINT_PADDING_X,
        FOOTER_RUN_HINT_PADDING_X,
    };
    #[cfg(target_os = "macos")]
    use super::{native_footer_visual_theme_from_parts, resolve_native_footer_visual_theme};

    fn assert_partitions_host(regions: &super::MainWindowDetachedFooterRegions) {
        let partition_height =
            regions.main_content.height + regions.transparent_gap.height + regions.footer.height;
        assert!((partition_height - regions.host.height).abs() < f32::EPSILON);
        assert!(regions.main_content.height >= 0.0);
        assert!(regions.transparent_gap.height >= 0.0);
        assert!(regions.footer.height >= 0.0);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn floating_footer_hints_are_edge_flush_while_legacy_keeps_its_inset() {
        let width = 750.0;
        let floating_inset = super::footer_hint_side_inset(true);
        let legacy_inset = super::footer_hint_side_inset(false);
        assert_eq!(floating_inset, 0.0);
        assert_eq!(
            legacy_inset,
            crate::window_resize::main_layout::HINT_STRIP_PADDING_X as f64
        );
        assert_eq!(width - floating_inset * 2.0, width);
        assert!(width - legacy_inset * 2.0 < width);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn edge_flush_left_info_capsule_reaches_the_window_edge_without_a_lead_gap() {
        // No left-pinned chip in edge-flush (floating glass) mode: the
        // left-info content sits at PAD_X so its visual capsule (which
        // extends back by PAD_X) starts exactly at x = 0.
        let lane = super::resolve_native_footer_lanes_with_mode(718.0, 0.0, 506.0, true);
        assert_eq!(lane.left_info_x, super::FOOTER_LEFT_INFO_CAPSULE_PAD_X);
        // A preceding left-pinned chip keeps the separating gap.
        let with_chip = super::resolve_native_footer_lanes_with_mode(718.0, 120.0, 506.0, true);
        assert_eq!(
            with_chip.left_info_x,
            120.0
                + f64::from(crate::components::footer_chrome::FOOTER_LEFT_RIGHT_MIN_GAP_PX)
                + super::FOOTER_LEFT_INFO_CAPSULE_PAD_X
        );
        // Legacy (non-flush) mode keeps the frozen lane math.
        let legacy = super::resolve_native_footer_lanes_with_mode(718.0, 0.0, 506.0, false);
        assert_eq!(
            legacy.left_info_x,
            f64::from(crate::components::footer_chrome::FOOTER_LEFT_RIGHT_MIN_GAP_PX)
                + super::FOOTER_LEFT_INFO_CAPSULE_PAD_X
        );
    }

    #[test]
    fn native_glass_capsules_use_the_shared_open_gap() {
        assert_eq!(footer_hint_item_gap(true, 2.0), 6.0);
        assert_eq!(footer_hint_item_gap(false, 2.0), 2.0);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn entry_defocus_targets_only_clipped_capsule_views() {
        for identifier in [
            "script-kit-footer-capsule-footer-action:actions",
            "script-kit-footer-capsule-footer-action:run",
            super::FOOTER_LEFT_INFO_CAPSULE_ID,
        ] {
            assert!(super::footer_identifier_is_entry_capsule(identifier));
        }
        for identifier in [
            super::FOOTER_GLASS_CONTAINER_ID,
            super::FOOTER_HINTS_ID,
            "script-kit-footer-capsule-content-footer-action:actions",
            "script-kit-footer-state-layer-footer-action:actions",
            super::FOOTER_LEFT_INFO_CAPSULE_CONTENT_ID,
        ] {
            assert!(!super::footer_identifier_is_entry_capsule(identifier));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_footer_lane_keeps_left_capsule_clear_of_trailing_actions() {
        let lane = super::resolve_native_footer_lanes(718.0, 120.0, 506.0);
        let capsule_min_x = lane.left_info_x - super::FOOTER_LEFT_INFO_CAPSULE_PAD_X;
        let capsule_max_x =
            lane.left_info_x + lane.left_info_width + super::FOOTER_LEFT_INFO_CAPSULE_PAD_X;
        let gap = f64::from(crate::components::footer_chrome::FOOTER_LEFT_RIGHT_MIN_GAP_PX);

        assert!(capsule_min_x >= lane.left_pinned_end_x + gap);
        assert!(capsule_max_x <= lane.trailing_start_x - gap);
        assert!(!lane.trailing_overflow);
    }

    #[cfg(target_os = "macos")]
    fn representative_left_info_measurements() -> super::FooterLeftInfoMeasurements {
        super::FooterLeftInfoMeasurements {
            cwd_fixed_width: 40.0,
            cwd_label_width: 80.0,
            primary_fixed_width: 30.0,
            primary_label_width: 100.0,
            has_cwd: true,
            primary_visible_without_label: true,
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_footer_lane_hides_left_info_when_clusters_exhaust_width() {
        let lane = super::resolve_native_footer_lanes(250.0, 132.0, 136.0);
        let allocation = super::resolve_footer_left_info_allocation(
            lane.left_info_width,
            representative_left_info_measurements(),
        );

        assert!(lane.trailing_overflow);
        assert_eq!(lane.left_info_width, 0.0);
        assert_eq!(
            allocation.degradation,
            super::FooterLeftInfoDegradation::Hidden
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn footer_left_info_allocation_degrades_monotonically() {
        use super::FooterLeftInfoDegradation::*;
        let measured = representative_left_info_measurements();

        assert_eq!(
            super::resolve_footer_left_info_allocation(300.0, measured).degradation,
            Full
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(200.0, measured).degradation,
            TruncatedLabels
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(110.0, measured).degradation,
            CwdAffordanceOnly
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(80.0, measured).degradation,
            PrimaryOnly
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(40.0, measured).degradation,
            PrimaryAffordanceOnly
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(20.0, measured).degradation,
            Hidden
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn footer_left_info_allocator_handles_no_cwd_and_long_labels() {
        use super::FooterLeftInfoDegradation::*;

        let no_cwd = super::FooterLeftInfoMeasurements {
            cwd_fixed_width: 0.0,
            cwd_label_width: 0.0,
            primary_fixed_width: 30.0,
            primary_label_width: 100.0,
            has_cwd: false,
            primary_visible_without_label: true,
        };
        assert_eq!(
            super::resolve_footer_left_info_allocation(90.0, no_cwd).degradation,
            PrimaryOnly
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(30.0, no_cwd).degradation,
            PrimaryAffordanceOnly
        );

        let long = super::FooterLeftInfoMeasurements {
            cwd_label_width: 480.0,
            primary_label_width: 640.0,
            ..representative_left_info_measurements()
        };
        let allocation = super::resolve_footer_left_info_allocation(180.0, long);
        assert_eq!(allocation.degradation, TruncatedLabels);
        assert!(allocation.cwd_label_width >= 24.0);
        assert!(allocation.primary_label_width >= 32.0);
        assert!(
            long.cwd_fixed_width
                + long.primary_fixed_width
                + allocation.cwd_label_width
                + allocation.primary_label_width
                <= allocation.available_width + f64::EPSILON
        );
    }

    #[test]
    fn native_glass_mode_always_owns_the_main_footer_without_an_overlay() {
        assert!(!should_use_gpui_footer_overlay(true, true));
        assert!(!should_use_gpui_footer_overlay(true, false));
        assert!(should_use_gpui_footer_overlay(false, true));
        assert!(!should_use_gpui_footer_overlay(false, false));
    }

    #[test]
    fn detached_footer_regions_partition_host_without_overlap() {
        let gpui = main_window_detached_footer_regions_gpui(750.0, 480.0, 32.0, 8.0, 2.0);
        let appkit = main_window_detached_footer_regions_appkit(750.0, 480.0, 32.0, 8.0, 2.0);

        assert_partitions_host(&gpui);
        assert_partitions_host(&appkit);
        assert_eq!(
            gpui.main_content.y + gpui.main_content.height,
            gpui.transparent_gap.y
        );
        assert_eq!(
            gpui.transparent_gap.y + gpui.transparent_gap.height,
            gpui.footer.y
        );
        assert_eq!(
            appkit.footer.y + appkit.footer.height,
            appkit.transparent_gap.y
        );
        assert_eq!(
            appkit.transparent_gap.y + appkit.transparent_gap.height,
            appkit.main_content.y
        );
    }

    #[test]
    fn detached_footer_regions_preserve_main_top_edge() {
        let short = main_window_detached_footer_regions_appkit(750.0, 480.0, 32.0, 8.0, 2.0);
        let tall = main_window_detached_footer_regions_appkit(750.0, 620.0, 32.0, 8.0, 2.0);

        assert_eq!(
            short.main_content.y + short.main_content.height,
            short.host.height
        );
        assert_eq!(
            tall.main_content.y + tall.main_content.height,
            tall.host.height
        );
        assert_eq!(short.main_content.y, tall.main_content.y);
    }

    #[test]
    fn detached_regions_round_to_two_x_backing_scale() {
        let regions = main_window_detached_footer_regions_gpui(749.74, 480.24, 31.76, 8.24, 2.0);

        for value in [
            regions.host.width,
            regions.host.height,
            regions.main_content.height,
            regions.transparent_gap.y,
            regions.transparent_gap.height,
            regions.footer.y,
            regions.footer.height,
        ] {
            assert_eq!(value * 2.0, (value * 2.0).round());
        }
        assert_partitions_host(&regions);
    }

    #[test]
    fn legacy_zero_strip_geometry_is_unchanged() {
        let regions = main_window_detached_footer_regions_gpui(750.0, 480.0, 0.0, 0.0, 2.0);

        assert_eq!(regions.main_content, regions.host);
        assert_eq!(regions.transparent_gap.height, 0.0);
        assert_eq!(regions.footer.height, 0.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn appkit_footer_rect_converts_to_top_left_screenshot_coordinates() {
        use cocoa::foundation::{NSPoint, NSRect, NSSize};

        let converted = super::appkit_screenshot_bounds(
            NSRect::new(NSPoint::new(10.0, 4.0), NSSize::new(100.0, 28.0)),
            480.0,
        );

        assert_eq!(converted.x, 10.0);
        assert_eq!(converted.y, 448.0);
        assert_eq!(converted.width, 100.0);
        assert_eq!(converted.height, 28.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn tips_keeps_a_distinct_native_action_selector() {
        use objc::{sel, sel_impl};

        assert_eq!(
            super::footer_action_selector(super::FooterAction::Tips),
            sel!(tipsFooterAction:)
        );
        assert_ne!(
            super::footer_action_selector(super::FooterAction::Tips),
            super::footer_action_selector(super::FooterAction::Actions)
        );
    }

    #[test]
    fn appkit_inventory_fails_closed_for_empty_or_duplicate_ids() {
        use crate::protocol::{AppKitFidelityNode, FidelityCaptureStatus};

        assert_eq!(
            super::appkit_fidelity_inventory_blocker(&[]),
            Some(FidelityCaptureStatus::EmptyInventory)
        );

        let duplicate = AppKitFidelityNode {
            id: "script-kit-footer-effect".to_string(),
            ..Default::default()
        };
        assert_eq!(
            super::appkit_fidelity_inventory_blocker(&[duplicate.clone(), duplicate]),
            Some(FidelityCaptureStatus::DuplicateIdentifiers)
        );

        let unique = AppKitFidelityNode {
            id: "script-kit-footer-divider".to_string(),
            ..Default::default()
        };
        assert_eq!(super::appkit_fidelity_inventory_blocker(&[unique]), None);
    }

    struct FooterTestParent;

    impl gpui::Render for FooterTestParent {
        fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    fn footer_test_parent(cx: &mut gpui::App) -> (gpui::WindowHandle<FooterTestParent>, crate::protocol::AutomationWindowInfo) {
        use gpui::AppContext as _;
        let parent = cx.open_window(Default::default(), |_, cx| cx.new(|_| FooterTestParent)).unwrap();
        let info = crate::windows::register_runtime_window_instance(crate::protocol::AutomationWindowInfo {
            id: format!("footer-test-parent-{}", super::next_footer_lifetime()),
            kind: crate::protocol::AutomationWindowKind::Main,
            title: None, focused: false, visible: true, semantic_surface: Some("mainMenu".into()),
            bounds: None, parent_window_id: None, parent_window_generation: None, parent_kind: None,
            pid: Some(std::process::id()), generation: None,
        }, parent.into(), cx).unwrap();
        (parent, info)
    }

    fn footer_test_overlay(
        parent: gpui::WindowHandle<FooterTestParent>, info: &crate::protocol::AutomationWindowInfo,
        bounds: gpui::Bounds<gpui::Pixels>, cx: &mut gpui::App,
    ) -> crate::protocol::AutomationWindowInfo {
        let state = super::footer_runtime_state(&info.id, info.generation.unwrap()).unwrap();
        let bounds = super::gpui_footer_overlay_bounds(bounds);
        let policy = crate::runtime_policy::WindowHostPolicy::Interactive;
        // Exercise the production renderer and lifetime publication without
        // configuring AppKit peers on GPUI's non-native test platform.
        let handle = super::open_footer_overlay_window(state.config, state.binding, bounds, None, policy, cx).unwrap();
        super::publish_footer_overlay(parent.into(), info, handle, bounds, policy, cx).unwrap()
    }

    #[gpui::test]
    fn footer_overlay_fidelity_is_a_separate_paint_target(cx: &mut gpui::TestAppContext) {
        let _theme_guard = crate::test_utils::lock_theme_cache_test();
        let _registry_guard = crate::windows::automation_registry::tests::registry_guard();
        let (parent, info, overlay) = cx.update(|cx| {
            gpui_component::init(cx);
            let (parent, info) = footer_test_parent(cx);
            let mut config = super::MainWindowFooterConfig::new("agent_chat", vec![super::FooterButtonConfig::new(super::FooterAction::Run, "↵", "Send")]);
            config.left_info = Some(super::FooterLeftInfo { model_name: "GPT-5.6 SOL".into(), ..Default::default() });
            let bounds = parent.update(cx, |_, window, _| { super::sync_footer_binding(window.window_handle(), Some(&config)); window.bounds() }).unwrap();
            let overlay = footer_test_overlay(parent, &info, bounds, cx);
            (parent, info, overlay)
        });
        let handle = crate::windows::get_runtime_window_handle_for_generation(&overlay.id, overlay.generation.unwrap()).unwrap();
        cx.update(|cx| handle.update(cx, |_, window, _| {
            window.set_fidelity_capture_target_for_test(Some("agent-chat"));
            window.refresh();
        }).unwrap());
        cx.run_until_parked();
        let snapshot = super::FOOTER_HOSTS.lock().unwrap().get(&parent.window_id()).unwrap().fidelity.clone().expect("completed footer frame");
        assert_eq!(snapshot.target_id, "gpui-footer-overlay");
        assert_eq!(snapshot.target_kind, "footerOverlay");
        assert_eq!(snapshot.parent_target_id.as_deref(), Some(info.id.as_str()));
        assert!(snapshot.frame_generation > 0);
        assert!(snapshot.nodes.iter().any(|node| node.id == "agent-chat.footer-overlay.footer-action:run" && node.primitive_count > 0));
        assert!(snapshot.nodes.iter().any(|node| node.id == "agent-chat.footer-overlay.model" && node.primitive_count > 0));
        assert!(snapshot.nodes.iter().all(|node| node.measurement_frame_generation == snapshot.frame_generation && node.measurement_provenance == "paint-time"));
        cx.update(|cx| {
            let elements = super::footer_fixture_elements(&overlay.id, overlay.generation.unwrap(), cx).unwrap();
            assert_eq!(elements[0].semantic_id, "footer-action:run");
            assert_eq!(elements[0].selectable, Some(true));
            let layout = super::footer_fixture_layout(&overlay.id, overlay.generation.unwrap(), cx).unwrap();
            assert_eq!(layout.prompt_type, "footerOverlay");
            for component in &layout.components {
                let painted = snapshot.nodes.iter().find(|node| node.id == component.name).expect("actual painted selector");
                assert_eq!(component.bounds, painted.bounds);
                assert_eq!(component.measurement_frame_generation, Some(snapshot.frame_generation));
            }
            let changed = super::MainWindowFooterConfig::new("agent_chat", vec![super::FooterButtonConfig::new(super::FooterAction::Run, "↵", "Send again")]);
            parent.update(cx, |_, window, _| super::sync_footer_binding(window.window_handle(), Some(&changed))).unwrap();
            assert!(super::footer_fixture_elements(&overlay.id, overlay.generation.unwrap(), cx).is_err());
            assert!(super::footer_fixture_layout(&overlay.id, overlay.generation.unwrap(), cx).is_err());
            super::retire_footer_owner(parent.into(), cx);
            crate::windows::remove_runtime_window_instance(&info.id, info.generation.unwrap());
            parent.update(cx, |_, window, _| window.remove_window()).unwrap();
        });
    }

    #[gpui::test]
    fn same_config_footer_hosts_dispatch_once_and_reject_stale_or_disabled(cx: &mut gpui::TestAppContext) {
        let _theme_guard = crate::test_utils::lock_theme_cache_test();
        let _registry_guard = crate::windows::automation_registry::tests::registry_guard();
        cx.update(|cx| {
            let (first, first_info) = footer_test_parent(cx);
            let (second, mut second_info) = footer_test_parent(cx);
            let config = super::MainWindowFooterConfig::new("about", vec![super::FooterButtonConfig::new(super::FooterAction::Close, "Esc", "Back")]);
            let (first_binding, first_rx) = first.update(cx, |_, window, _| (super::sync_footer_binding(window.window_handle(), Some(&config)).unwrap(), super::footer_action_receiver(window))).unwrap();
            let (second_binding, second_rx) = second.update(cx, |_, window, _| (super::sync_footer_binding(window.window_handle(), Some(&config)).unwrap(), super::footer_action_receiver(window))).unwrap();
            assert_ne!(first_binding.host_generation, second_binding.host_generation);
            assert!(super::dispatch_bound_footer_action(&first_binding, super::FooterAction::Close));
            assert!(second_rx.try_recv().is_err());
            let event = first_rx.try_recv().unwrap();
            second.update(cx, |_, window, _| assert!(event.accept(window).is_none())).unwrap();
            first.update(cx, |_, window, _| {
                assert_eq!(event.accept(window), Some(super::FooterAction::Close));
                event.complete(window);
                assert!(event.accept(window).is_none());
                event.complete(window);
            }).unwrap();
            assert_eq!(super::footer_runtime_state(&first_info.id, first_info.generation.unwrap()).unwrap().completed_action_count, 1);
            assert_eq!(super::footer_runtime_state(&second_info.id, second_info.generation.unwrap()).unwrap().completed_action_count, 0);
            assert!(super::dispatch_bound_footer_action(&first_binding, super::FooterAction::Close));
            let stale = first_rx.try_recv().unwrap();
            let disabled = super::MainWindowFooterConfig::new("about", vec![super::FooterButtonConfig::new(super::FooterAction::Close, "Esc", "Back").disabled_reason("Not available")]);
            let disabled_binding = first.update(cx, |_, window, _| {
                let binding = super::sync_footer_binding(window.window_handle(), Some(&disabled)).unwrap();
                assert!(stale.accept(window).is_none());
                binding
            }).unwrap();
            assert!(!super::dispatch_bound_footer_action(&disabled_binding, super::FooterAction::Close));
            assert!(!super::dispatch_bound_footer_action(&first_binding, super::FooterAction::Close));
            first.update(cx, |_, window, _| stale.complete(window)).unwrap();
            assert_eq!(super::footer_runtime_state(&first_info.id, first_info.generation.unwrap()).unwrap().completed_action_count, 1);
            assert!(first_rx.try_recv().is_err());
            // Re-enabling cannot authorize an envelope queued before the change.
            let enabled = first.update(cx, |_, window, _| super::sync_footer_binding(window.window_handle(), Some(&config)).unwrap()).unwrap();
            first.update(cx, |_, window, _| assert!(stale.accept(window).is_none())).unwrap();
            assert!(super::dispatch_bound_footer_action(&enabled, super::FooterAction::Close));
            let enabled_event = first_rx.try_recv().unwrap();
            first.update(cx, |_, window, _| {
                assert_eq!(enabled_event.accept(window), Some(super::FooterAction::Close));
                enabled_event.complete(window);
                assert!(enabled_event.accept(window).is_none());
                enabled_event.complete(window);
            }).unwrap();
            assert_eq!(super::footer_runtime_state(&first_info.id, first_info.generation.unwrap()).unwrap().completed_action_count, 2);
            assert!(super::dispatch_bound_footer_action(&enabled, super::FooterAction::Close));
            let enabled_stale = first_rx.try_recv().unwrap();
            let mut renamed = config.clone();
            renamed.buttons[0].label = "Return".into();
            first.update(cx, |_, window, _| {
                super::sync_footer_binding(window.window_handle(), Some(&renamed));
                assert!(enabled_stale.accept(window).is_none());
                enabled_stale.complete(window);
            }).unwrap();
            assert_eq!(super::footer_runtime_state(&first_info.id, first_info.generation.unwrap()).unwrap().completed_action_count, 2);
            assert!(super::dispatch_bound_footer_action(&second_binding, super::FooterAction::Close));
            let retired = second_rx.try_recv().unwrap();
            super::retire_footer_owner(second.into(), cx);
            assert!(second_rx.is_closed());
            crate::windows::remove_runtime_window_instance(&second_info.id, second_info.generation.unwrap());
            second.update(cx, |_, window, _| assert!(retired.accept(window).is_none())).unwrap();
            second_info.generation = None;
            second_info = crate::windows::register_runtime_window_instance(second_info, second.into(), cx).unwrap();
            let recreated = second.update(cx, |_, window, _| {
                let binding = super::sync_footer_binding(window.window_handle(), Some(&config)).unwrap();
                assert!(retired.accept(window).is_none());
                binding
            }).unwrap();
            assert_ne!(recreated.window_generation, second_binding.window_generation);
            assert_ne!(recreated.host_generation, second_binding.host_generation);
            assert!(!super::dispatch_bound_footer_action(&second_binding, super::FooterAction::Close));
            assert!(super::dispatch_bound_footer_action(&recreated, super::FooterAction::Close));
            let recreated_rx = second.update(cx, |_, window, _| super::footer_action_receiver(window)).unwrap();
            assert!(second_rx.try_recv().is_err());
            assert!(first_rx.try_recv().is_err());
            let current = recreated_rx.try_recv().unwrap();
            second.update(cx, |_, window, _| {
                assert!(retired.accept(window).is_none());
                retired.complete(window);
                assert_eq!(super::footer_runtime_state(&second_info.id, second_info.generation.unwrap()).unwrap().completed_action_count, 0);
                assert_eq!(current.accept(window), Some(super::FooterAction::Close));
                current.complete(window);
                assert!(current.accept(window).is_none());
                current.complete(window);
            }).unwrap();
            assert_eq!(super::footer_runtime_state(&second_info.id, second_info.generation.unwrap()).unwrap().completed_action_count, 1);
            for (handle, info) in [(first, first_info), (second, second_info)] {
                super::retire_footer_owner(handle.into(), cx);
                crate::windows::remove_runtime_window_instance(&info.id, info.generation.unwrap());
                handle.update(cx, |_, window, _| window.remove_window()).unwrap();
            }
        });
    }

    #[cfg(target_os = "macos")]
    #[gpui::test]
    fn footer_refresh_commits_only_to_exact_successful_host(cx: &mut gpui::TestAppContext) {
        let _theme_guard = crate::test_utils::lock_theme_cache_test();
        let _registry_guard = crate::windows::automation_registry::tests::registry_guard();
        cx.update(|cx| {
            let (main, main_info) = footer_test_parent(cx);
            let (secondary, secondary_info) = footer_test_parent(cx);
            let config = super::MainWindowFooterConfig::new("about", vec![FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")]);
            let main_binding = super::sync_footer_binding(main.into(), Some(&config)).unwrap();
            let secondary_binding = super::sync_footer_binding(secondary.into(), Some(&config)).unwrap();
            let theme = crate::theme::get_theme_snapshot();
            let signature = super::native_footer_refresh_signature(&config, &theme, 750.0, false);
            assert!(!super::commit_footer_refresh(main.into(), &secondary_binding, signature.clone()));
            assert!(!super::commit_footer_refresh(secondary.into(), &main_binding, signature.clone()));
            // The real refresh failure path must not populate either cache.
            // SAFETY: nil is an Objective-C null receiver, never a fabricated live pointer.
            assert!(!unsafe { super::refresh_footer_host_impl(cocoa::base::nil, cocoa::base::nil, &config, false) });
            {
                let hosts = super::FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
                assert!(hosts[&main.window_id()].refresh_signature.is_none());
                assert!(hosts[&secondary.window_id()].refresh_signature.is_none());
            }
            assert!(super::commit_footer_refresh(main.into(), &main_binding, signature.clone()));
            let main_snapshot = super::footer_host_snapshot(main.into());
            assert!(super::FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner())[&secondary.window_id()].refresh_signature.is_none());
            assert!(super::commit_footer_refresh(secondary.into(), &secondary_binding, signature.clone()));
            let mut changed = config.clone();
            changed.buttons[0].label = "Return".into();
            let next = super::sync_footer_binding(secondary.into(), Some(&changed)).unwrap();
            let changed_signature = super::native_footer_refresh_signature(&changed, &theme, 750.0, false);
            assert!(!super::commit_footer_refresh(secondary.into(), &secondary_binding, signature.clone()));
            assert!(!super::commit_footer_refresh(secondary.into(), &next, signature.clone()));
            assert!(super::commit_footer_refresh(secondary.into(), &next, changed_signature));
            {
                let hosts = super::FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
                assert_eq!(hosts[&main.window_id()].refresh_signature.as_ref(), Some(&signature));
                assert_eq!(hosts[&main.window_id()].binding.as_ref(), Some(&main_binding));
                assert_eq!(hosts[&main.window_id()].snapshot, main_snapshot);
                assert_eq!(hosts[&main.window_id()].config.as_ref(), Some(&config));
            }
            // A revision alone invalidates the signature, even if all colors are equal.
            let next_theme = crate::theme::live_edit::PublishedTheme {
                revision: theme.revision + 1, theme: theme.theme.clone(), resolved: theme.resolved.clone(),
            };
            let next_signature = super::native_footer_refresh_signature(&config, &next_theme, 750.0, false);
            assert_ne!(signature, next_signature);
            assert!(!super::commit_footer_refresh(main.into(), &main_binding, next_signature));
            super::retire_footer_owner(secondary.into(), cx);
            let recreated = super::sync_footer_binding(secondary.into(), Some(&config)).unwrap();
            assert_ne!(recreated.host_generation, next.host_generation);
            assert!(super::FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner())[&secondary.window_id()].refresh_signature.is_none());
            assert!(!super::commit_footer_refresh(secondary.into(), &secondary_binding, signature.clone()));
            assert!(super::commit_footer_refresh(secondary.into(), &recreated, signature));
            for (handle, info) in [(main, main_info), (secondary, secondary_info)] {
                super::retire_footer_owner(handle.into(), cx);
                crate::windows::remove_runtime_window_instance(&info.id, info.generation.unwrap());
                handle.update(cx, |_, window, _| window.remove_window()).unwrap();
            }
        });
    }

    #[gpui::test]
    fn theme_publication_refuses_queued_footer_actions_before_either_host_redraws(cx: &mut gpui::TestAppContext) {
        let _theme_guard = crate::test_utils::lock_theme_cache_test();
        let _registry_guard = crate::windows::automation_registry::tests::registry_guard();
        let observations = cx.update(|cx| {
            gpui_component::init(cx);
            let baseline = crate::theme::get_theme_snapshot();
            let config = super::MainWindowFooterConfig::new("about", vec![FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")]);
            let mut owners = Vec::new();
            for _ in 0..2 {
                let (handle, info) = footer_test_parent(cx);
                let (binding, rx) = handle.update(cx, |_, window, _| (
                    super::sync_footer_binding(window.window_handle(), Some(&config)).unwrap(),
                    super::footer_action_receiver(window),
                )).unwrap();
                assert!(super::dispatch_bound_footer_action(&binding, FooterAction::Close));
                owners.push((handle, info, binding, rx));
            }
            let publication = crate::theme::service::publish_runtime_theme(
                cx, baseline.revision,
                crate::theme::live_edit::prepare_theme((*baseline.theme).clone()).unwrap(),
                crate::theme::service::ThemePublicationSource::LivePreview,
            ).unwrap();
            let mut observations = Vec::new();
            // No foreground pump or draw between publication and receiver
            // validation: the two cached host bindings still carry the old theme.
            for (handle, info, binding, rx) in owners {
                let stale = rx.try_recv().unwrap();
                let accepted_stale = handle.update(cx, |_, window, _| {
                    let accepted = stale.accept(window);
                    stale.complete(window);
                    accepted
                }).unwrap();
                let stale_count = super::footer_runtime_state(&info.id, info.generation.unwrap()).unwrap().completed_action_count;
                let old_binding_enqueued = super::dispatch_bound_footer_action(&binding, FooterAction::Close);
                let current = super::sync_footer_binding(handle.into(), Some(&config)).unwrap();
                let enqueued = super::dispatch_bound_footer_action(&current, FooterAction::Close);
                // If old enqueue incorrectly succeeded, drain its exact stale
                // event so the current action can still be observed separately.
                if old_binding_enqueued {
                    let old = rx.try_recv().unwrap();
                    handle.update(cx, |_, window, _| { let _ = old.accept(window); old.complete(window); }).unwrap();
                }
                let accepted_current = if enqueued {
                    let event = rx.try_recv().unwrap();
                    handle.update(cx, |_, window, _| {
                        let first = event.accept(window);
                        event.complete(window);
                        let duplicate = event.accept(window);
                        event.complete(window);
                        (first, duplicate)
                    }).unwrap()
                } else { (None, None) };
                let completed_count = super::footer_runtime_state(&info.id, info.generation.unwrap()).unwrap().completed_action_count;
                observations.push((accepted_stale, stale_count, old_binding_enqueued, accepted_current, completed_count,
                    current.host_generation == binding.host_generation,
                    current.theme_revision == publication.revision));
                super::retire_footer_owner(handle.into(), cx);
                crate::windows::remove_runtime_window_instance(&info.id, info.generation.unwrap());
                handle.update(cx, |_, window, _| window.remove_window()).unwrap();
            }
            // Restore through the same service before checking observations, so
            // a regression assertion cannot leak an edited global theme.
            crate::theme::service::publish_runtime_theme(
                cx, publication.revision,
                crate::theme::live_edit::prepare_theme((*baseline.theme).clone()).unwrap(),
                crate::theme::service::ThemePublicationSource::LivePreview,
            ).unwrap();
            observations
        });
        assert_eq!(observations.len(), 2);
        for observation in observations {
            assert_eq!(observation, (None, 0, false, (Some(FooterAction::Close), None), 1, true, true));
        }
    }

    #[gpui::test]
    fn equal_config_overlays_keep_independent_frames_and_parent_teardown(cx: &mut gpui::TestAppContext) {
        let _theme_guard = crate::test_utils::lock_theme_cache_test();
        let _registry_guard = crate::windows::automation_registry::tests::registry_guard();
        let (first, first_info, first_overlay, second, second_info, second_overlay) = cx.update(|cx| {
            gpui_component::init(cx);
            let (first, first_info) = footer_test_parent(cx);
            let (second, second_info) = footer_test_parent(cx);
            let config = super::MainWindowFooterConfig::new("about", vec![super::FooterButtonConfig::new(super::FooterAction::Close, "Esc", "Back")]);
            let first_bounds = first.update(cx, |_, window, _| { super::sync_footer_binding(window.window_handle(), Some(&config)); window.bounds() }).unwrap();
            let second_bounds = second.update(cx, |_, window, _| { super::sync_footer_binding(window.window_handle(), Some(&config)); window.bounds() }).unwrap();
            let first_overlay = footer_test_overlay(first, &first_info, first_bounds, cx);
            let second_overlay = footer_test_overlay(second, &second_info, second_bounds, cx);
            assert_ne!(first_overlay.id, second_overlay.id);
            assert_ne!(crate::windows::get_runtime_window_handle(&first_overlay.id), crate::windows::get_runtime_window_handle(&second_overlay.id));
            (first, first_info, first_overlay, second, second_info, second_overlay)
        });
        cx.run_until_parked();
        let first_before = super::footer_runtime_state(&first_overlay.id, first_overlay.generation.unwrap()).unwrap();
        let second_before = super::footer_runtime_state(&second_overlay.id, second_overlay.generation.unwrap()).unwrap();
        assert_eq!(first_before.config, second_before.config);
        assert_eq!(first_before.applied_theme_revision, crate::theme::get_theme_snapshot().revision);
        assert_eq!(second_before.applied_theme_revision, first_before.applied_theme_revision);
        cx.update(|cx| {
            let changed = super::MainWindowFooterConfig::new("about", vec![super::FooterButtonConfig::new(super::FooterAction::Close, "Esc", "Return")]);
            first.update(cx, |_, window, cx| { super::sync_footer_binding(window.window_handle(), Some(&changed)); super::notify_changed_footer_overlay(window.window_handle(), cx); }).unwrap();
        });
        cx.run_until_parked();
        assert!(super::footer_runtime_state(&first_overlay.id, first_overlay.generation.unwrap()).unwrap().presentation_revision > first_before.presentation_revision);
        assert_eq!(super::footer_runtime_state(&second_overlay.id, second_overlay.generation.unwrap()).unwrap().config, second_before.config);
        cx.update(|cx| first.update(cx, |_, window, _| window.remove_window()).unwrap());
        cx.run_until_parked();
        assert!(crate::windows::get_runtime_window_handle_for_generation(&first_overlay.id, first_overlay.generation.unwrap()).is_none());
        assert!(crate::windows::get_runtime_window_handle_for_generation(&second_overlay.id, second_overlay.generation.unwrap()).is_some());
        cx.update(|cx| {
            super::retire_footer_owner(second.into(), cx);
            crate::windows::remove_runtime_window_instance(&first_info.id, first_info.generation.unwrap());
            crate::windows::remove_runtime_window_instance(&second_info.id, second_info.generation.unwrap());
            second.update(cx, |_, window, _| window.remove_window()).unwrap();
        });
    }

    #[test]
    fn footer_semantics_preserve_disabled_selected_and_held_states() {
        let config = super::MainWindowFooterConfig::new("fixture", vec![
            FooterButtonConfig::new(FooterAction::Run, "↵", "Run").disabled_reason("Choose an item"),
            FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions").selected(true),
            FooterButtonConfig::new(FooterAction::Close, "Esc", "Close"),
        ]);
        let elements = super::footer_elements(&config, Some(FooterAction::Close));
        assert_eq!(elements[0].action_disabled.as_deref(), Some("Choose an item"));
        assert_eq!(elements[0].status_kind.as_deref(), Some("disabled"));
        assert_eq!(elements[0].selectable, Some(false));
        assert_eq!(elements[1].selected, Some(true));
        assert_eq!(elements[1].status_kind.as_deref(), Some("selected"));
        assert_eq!(elements[2].selected, Some(false));
        assert_eq!(elements[2].status_kind.as_deref(), Some("held"));
    }

    #[gpui::test]
    fn footer_completion_ticket_requires_its_exact_accepted_handler(cx: &mut gpui::TestAppContext) {
        let _theme_guard = crate::test_utils::lock_theme_cache_test();
        let _registry_guard = crate::windows::automation_registry::tests::registry_guard();
        cx.update(|cx| {
            let (parent, info) = footer_test_parent(cx);
            let config = super::MainWindowFooterConfig::new("fixture", vec![FooterButtonConfig::new(FooterAction::Close, "Esc", "Close")]);
            let (binding, events) = parent.update(cx, |_, window, _| (super::sync_footer_binding(window.window_handle(), Some(&config)).unwrap(), super::footer_action_receiver(window))).unwrap();
            assert!(super::dispatch_bound_footer_action(&binding, FooterAction::Close));
            let unrelated = events.try_recv().unwrap();
            let (sender, receiver) = async_channel::bounded(1);
            let ticket = super::FooterActionCompletion { receiver, completed: std::cell::Cell::new(false) };
            super::enqueue_bound_footer_action(&binding, FooterAction::Close, Some(sender)).unwrap();
            let selected = events.try_recv().unwrap();
            assert!(!ticket.poll().unwrap());
            parent.update(cx, |_, window, _| {
                unrelated.accept(window).unwrap(); unrelated.complete(window);
                assert!(!ticket.poll().unwrap(), "another event cannot complete this ticket");
                selected.complete(window);
                assert!(!ticket.poll().unwrap(), "unaccepted events cannot complete");
                selected.accept(window).unwrap();
                assert!(!ticket.poll().unwrap(), "acceptance alone is not completion");
                selected.complete(window);
            }).unwrap();
            assert!(ticket.poll().unwrap());
            assert!(ticket.poll().unwrap(), "completed ticket remains completed");
            let (sender, receiver) = async_channel::bounded(1);
            let stale_ticket = super::FooterActionCompletion { receiver, completed: std::cell::Cell::new(false) };
            super::enqueue_bound_footer_action(&binding, FooterAction::Close, Some(sender)).unwrap();
            let stale = events.try_recv().unwrap();
            let disabled = super::MainWindowFooterConfig::new("fixture", vec![FooterButtonConfig::new(FooterAction::Close, "Esc", "Close").disabled_reason("Unavailable")]);
            parent.update(cx, |_, window, _| {
                super::sync_footer_binding(window.window_handle(), Some(&disabled));
                assert!(stale.accept(window).is_none());
                stale.complete(window);
            }).unwrap();
            assert!(stale_ticket.poll().is_err());
            super::retire_footer_owner(parent.into(), cx);
            crate::windows::remove_runtime_window_instance(&info.id, info.generation.unwrap());
            parent.update(cx, |_, window, _| window.remove_window()).unwrap();
        });
    }

    #[test]
    fn footer_descriptor_caches_semantic_identity_and_canonical_shortcut() {
        let descriptor = FooterButtonConfig::new(FooterAction::Actions, "cmd+k", "Open Menu");

        assert_eq!(descriptor.id, "footer-action:actions");
        assert_eq!(descriptor.action, FooterAction::Actions);
        assert_eq!(descriptor.label, "Open Menu");
        assert_eq!(descriptor.shortcut_tokens, vec!["⌘", "K"]);
        assert_eq!(descriptor.canonical_shortcut.as_deref(), Some("cmd+k"));
        assert!(descriptor.shortcut_routable);
        assert_eq!(descriptor.placement, super::FooterPlacement::Trailing);
    }

    #[test]
    fn disabled_footer_descriptor_hides_its_key_route_but_keeps_reason() {
        let descriptor = FooterButtonConfig::new(FooterAction::Run, "↵", "Run")
            .disabled_reason("Requires a selection");
        let config = super::MainWindowFooterConfig::new("test", vec![descriptor]);
        let descriptor = &config.buttons[0];

        assert!(!descriptor.enabled);
        assert_eq!(
            descriptor
                .disabled_reason
                .as_ref()
                .map(|reason| reason.as_ref()),
            Some("Requires a selection")
        );
        assert!(!descriptor.shortcut_routable);
        assert_eq!(config.action_for_canonical_shortcut("enter"), None);
    }

    #[test]
    fn footer_dispatch_authorization_requires_enabled_current_button() {
        let config = super::MainWindowFooterConfig::new(
            "script_list",
            vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Run"),
                FooterButtonConfig::new(FooterAction::Ai, "⌘↵", "Agent")
                    .disabled_reason("Resolve the permission request first."),
            ],
        );

        assert_eq!(
            config.action_dispatch_authorization(FooterAction::Run, false),
            super::FooterActionDispatchAuthorization::PresentedButton
        );
        assert_eq!(
            config.action_dispatch_authorization(FooterAction::Ai, true),
            super::FooterActionDispatchAuthorization::Disabled {
                reason: Some("Resolve the permission request first."),
            }
        );
    }

    #[test]
    fn footer_dispatch_authorization_rejects_stale_invisible_sensitive_actions() {
        let previous = super::MainWindowFooterConfig::new(
            "previous_surface",
            vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Run"),
                FooterButtonConfig::new(FooterAction::Ai, "⌘↵", "Agent"),
                FooterButtonConfig::new(FooterAction::Stop, "⌘.", "Stop"),
            ],
        );
        let current = super::MainWindowFooterConfig::new(
            "current_surface",
            vec![FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")],
        );

        for action in [FooterAction::Run, FooterAction::Ai, FooterAction::Stop] {
            assert_eq!(
                previous.action_dispatch_authorization(action, false),
                super::FooterActionDispatchAuthorization::PresentedButton,
                "old surface displayed {action:?}"
            );
            assert_eq!(
                current.action_dispatch_authorization(action, false),
                super::FooterActionDispatchAuthorization::NotPresented,
                "a stale callback must not execute {action:?} on the new surface"
            );
        }
    }

    #[test]
    fn footer_dispatch_authorization_preserves_only_exact_live_left_affordances() {
        let mut config = super::MainWindowFooterConfig::new("script_list", Vec::new());
        config.left_info = Some(super::FooterLeftInfo {
            model_name: "Discover tips".to_owned(),
            action: Some(FooterAction::Tips),
            ..Default::default()
        });
        assert_eq!(
            config.action_dispatch_authorization(FooterAction::Tips, false),
            super::FooterActionDispatchAuthorization::PresentedLeftAffordance
        );
        assert_eq!(
            config.action_dispatch_authorization(FooterAction::Ai, false),
            super::FooterActionDispatchAuthorization::NotPresented
        );

        config.left_info = Some(super::FooterLeftInfo {
            model_name: "Selected model".to_owned(),
            profile_name: Some("Selected agent".to_owned()),
            cwd_chip: Some(super::FooterCwdChip {
                label: "Project".to_owned(),
                icon_token: "folder".to_owned(),
                key: None,
                tooltip: None,
            }),
            ..Default::default()
        });
        for action in [FooterAction::Cwd, FooterAction::AgentModel] {
            assert_eq!(
                config.action_dispatch_authorization(action, false),
                super::FooterActionDispatchAuthorization::PresentedLeftAffordance
            );
        }
        assert_eq!(
            config.action_dispatch_authorization(FooterAction::Tips, false),
            super::FooterActionDispatchAuthorization::NotPresented
        );

        config.left_info.as_mut().unwrap().model_name.clear();
        assert_eq!(
            config.action_dispatch_authorization(FooterAction::AgentModel, false),
            super::FooterActionDispatchAuthorization::NotPresented
        );
    }

    #[test]
    fn footer_dispatch_authorization_limits_header_override_to_live_context_chips() {
        let config = super::MainWindowFooterConfig::new("script_list", Vec::new());

        for action in [FooterAction::Cwd, FooterAction::AgentModel] {
            assert_eq!(
                config.action_dispatch_authorization(action, false),
                super::FooterActionDispatchAuthorization::NotPresented
            );
            assert_eq!(
                config.action_dispatch_authorization(action, true),
                super::FooterActionDispatchAuthorization::PresentedHeaderAffordance
            );
        }

        for action in [FooterAction::Run, FooterAction::Ai, FooterAction::Stop] {
            assert_eq!(
                config.action_dispatch_authorization(action, true),
                super::FooterActionDispatchAuthorization::NotPresented,
                "header authorization must never grant invisible {action:?}"
            );
        }
    }

    #[test]
    fn duplicate_footer_shortcuts_are_retained_for_diagnostics_but_not_routable() {
        let config = super::MainWindowFooterConfig::new(
            "test",
            vec![
                FooterButtonConfig::new(FooterAction::Run, "⌘↵", "Run"),
                FooterButtonConfig::new(FooterAction::Ai, "cmd+enter", "Agent"),
            ],
        );
        let model = config.slot_model();

        assert_eq!(model.duplicate_shortcut_keys, vec!["cmd+enter"]);
        assert!(config
            .buttons
            .iter()
            .all(|button| !button.shortcut_routable));
        assert_eq!(config.action_for_canonical_shortcut("cmd+enter"), None);
        assert_eq!(config.slot_contract_violation(), None);
    }

    #[test]
    fn footer_descriptor_runtime_fixture_preserves_identity_and_fails_closed() {
        let base = || {
            vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Run"),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions"),
                FooterButtonConfig::new(FooterAction::Ai, "⌘↵", "Agent"),
            ]
        };

        let mut disabled = base();
        super::apply_footer_descriptor_test_fixture_mode(&mut disabled, "disabled");
        let disabled = super::MainWindowFooterConfig::new("test", disabled);
        let actions = disabled
            .descriptor_for_action(FooterAction::Actions)
            .expect("Actions descriptor");
        assert_eq!(actions.id, "footer-action:actions");
        assert!(!actions.enabled);
        assert!(!actions.shortcut_routable);
        assert!(actions.disabled_reason.is_some());
        assert_eq!(disabled.action_for_canonical_shortcut("cmd+k"), None);

        let mut collision = base();
        super::apply_footer_descriptor_test_fixture_mode(&mut collision, "collision");
        let collision = super::MainWindowFooterConfig::new("test", collision);
        assert_eq!(
            collision.slot_model().duplicate_shortcut_keys,
            vec!["cmd+k"]
        );
        assert_eq!(collision.action_for_canonical_shortcut("cmd+k"), None);
        assert!(collision
            .buttons
            .iter()
            .filter(|button| button.canonical_shortcut.as_deref() == Some("cmd+k"))
            .all(|button| !button.shortcut_routable));

        let mut renamed = base();
        super::apply_footer_descriptor_test_fixture_mode(&mut renamed, "renamed");
        let renamed = super::MainWindowFooterConfig::new("test", renamed);
        let actions = renamed
            .descriptor_for_action(FooterAction::Actions)
            .expect("renamed Actions descriptor");
        assert_eq!(actions.id, "footer-action:actions");
        assert_eq!(actions.label, "More Actions");
        assert_eq!(actions.canonical_shortcut.as_deref(), Some("cmd+k"));
        assert_eq!(
            renamed.action_for_canonical_shortcut("cmd+k"),
            Some(FooterAction::Actions)
        );
    }

    #[test]
    #[should_panic(expected = "main window footer action IDs must be unique")]
    fn duplicate_footer_descriptor_ids_fail_validation() {
        let _ = super::MainWindowFooterConfig::new(
            "test",
            vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Run"),
                FooterButtonConfig::new(FooterAction::Run, "⌘↵", "Run Again"),
            ],
        );
    }

    #[test]
    fn footer_dispatch_identity_does_not_depend_on_the_visible_verb() {
        let first = FooterButtonConfig::new(FooterAction::Run, "↵", "Run");
        let renamed = FooterButtonConfig::new(FooterAction::Run, "↵", "Continue");

        assert_eq!(first.id, renamed.id);
        assert_eq!(first.action, renamed.action);
        assert_ne!(first.label, renamed.label);
    }

    #[test]
    fn left_pinned_buttons_do_not_receive_legacy_extra_padding() {
        // The left chips and the Run button are start-anchored, so trailing
        // padding would show up as a visibly wider gap before the next item.
        assert_eq!(
            footer_hint_legacy_extra_padding(&FooterButtonConfig::new(
                FooterAction::Cwd,
                "⇥",
                "~/ai_completion"
            )),
            0.0
        );
        assert_eq!(
            footer_hint_legacy_extra_padding(&FooterButtonConfig::new(
                FooterAction::AgentModel,
                "⇧⇥",
                "Codex · GPT-5.6 SOL"
            )),
            0.0
        );
        assert_eq!(
            footer_hint_legacy_extra_padding(&FooterButtonConfig::new(
                FooterAction::Run,
                "↵",
                "Send"
            )),
            0.0
        );
        // Trailing action buttons keep the comfortable 12px reserve.
        assert_eq!(
            footer_hint_legacy_extra_padding(&FooterButtonConfig::new(
                FooterAction::Actions,
                "⌘K",
                "Actions"
            )),
            12.0
        );
    }

    #[test]
    fn left_pinned_cwd_uses_same_label_to_key_gap_as_trailing_buttons() {
        let button = FooterButtonConfig::new(FooterAction::Cwd, "⇥", "~/ai_completion");
        let label_width = 92.0;
        let key_width = 20.0;
        let item_width =
            label_width + FOOTER_HINT_KEY_LABEL_GAP + key_width + FOOTER_RUN_HINT_PADDING_X * 2.0;
        let (label_x, key_x, _) = footer_hint_content_layout_for_button(
            &button,
            item_width,
            label_width,
            key_width,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_HINT_PADDING_X,
            FOOTER_RUN_HINT_PADDING_X,
        );
        // Label anchored at the leading padding, keycap exactly one content-gap
        // after the label — identical spacing to the right-side buttons.
        assert_eq!(label_x, FOOTER_RUN_HINT_PADDING_X.round());
        assert_eq!(key_x - (label_x + label_width), FOOTER_HINT_KEY_LABEL_GAP);
    }

    #[test]
    fn footer_hint_slot_widths_are_stable_per_action() {
        assert_eq!(footer_hint_slot_width(FooterAction::Run), 92.0);
        assert_eq!(footer_hint_slot_width(FooterAction::Actions), 92.0);
        assert_eq!(footer_hint_slot_width(FooterAction::Ai), 52.0);
        assert_eq!(footer_hint_slot_width(FooterAction::Stop), 76.0);
        assert_eq!(footer_hint_slot_width(FooterAction::PasteResponse), 140.0);
    }

    #[test]
    fn run_slot_remains_at_least_as_wide_as_actions_and_wider_than_ai() {
        assert!(
            footer_hint_slot_width(FooterAction::Run)
                >= footer_hint_slot_width(FooterAction::Actions)
        );
        assert!(
            footer_hint_slot_width(FooterAction::Run) > footer_hint_slot_width(FooterAction::Ai)
        );
    }

    #[test]
    fn footer_hint_content_group_is_centered_within_slot() {
        let item_width = 92.0;
        let label_width = 34.0;
        let key_width = 18.0;

        let (label_x, key_x, content_width) = footer_hint_content_layout(
            FooterAction::Actions,
            item_width,
            label_width,
            key_width,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_RUN_HINT_PADDING_X,
        );
        let left_padding = label_x;
        let right_padding = item_width - (key_x + key_width);

        assert_eq!(
            content_width,
            label_width + FOOTER_HINT_KEY_LABEL_GAP + key_width
        );
        assert!((left_padding - right_padding).abs() <= 1.0);
    }

    #[test]
    fn run_hint_keeps_key_glyph_anchored_to_trailing_padding() {
        let short = footer_hint_content_layout(
            FooterAction::Run,
            92.0,
            20.0,
            18.0,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_RUN_HINT_PADDING_X,
        );
        let long = footer_hint_content_layout(
            FooterAction::Run,
            140.0,
            64.0,
            18.0,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_RUN_HINT_PADDING_X,
        );

        assert_eq!(short.1, 68.0);
        assert_eq!(long.1, 116.0);
        assert_eq!(92.0 - (short.1 + 18.0), 6.0);
        assert_eq!(140.0 - (long.1 + 18.0), 6.0);
    }

    #[test]
    fn run_hint_native_layout_can_balance_short_label_padding() {
        let label_width = 26.0;
        let key_width = 20.0;
        let item_width =
            label_width + FOOTER_HINT_KEY_LABEL_GAP + key_width + FOOTER_RUN_HINT_PADDING_X * 2.0;
        let (label_x, key_x, _) = footer_hint_content_layout(
            FooterAction::Run,
            item_width,
            label_width,
            key_width,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_RUN_HINT_PADDING_X,
        );

        assert_eq!(label_x, FOOTER_RUN_HINT_PADDING_X);
        assert_eq!(item_width - (key_x + key_width), FOOTER_RUN_HINT_PADDING_X);
    }

    #[test]
    fn all_selected_footer_actions_use_the_main_menu_active_row_fill() {
        let theme = crate::theme::Theme::dark_default();
        let active = crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
            .row_states
            .active
            .background_rgba
            .expect("active rows have a fill");

        for _action in [FooterAction::Actions, FooterAction::Run, FooterAction::Ai] {
            assert_eq!(
                crate::components::footer_chrome::themed_footer_button_active_rgba(&theme),
                active
            );
        }
    }

    #[test]
    fn native_footer_state_keeps_active_precedence_over_hover() {
        use crate::theme::MainMenuRowState::{Active, Hover, Rest};

        assert_eq!(
            resolved_native_footer_button_state(false, false, false, false),
            Rest
        );
        assert_eq!(
            resolved_native_footer_button_state(false, true, false, false),
            Hover
        );
        assert_eq!(
            resolved_native_footer_button_state(true, true, false, false),
            Active
        );
        assert_eq!(
            resolved_native_footer_button_state(false, true, true, true),
            Active
        );
        assert_eq!(
            resolved_native_footer_button_state(false, false, false, true),
            Rest
        );
    }

    #[test]
    fn left_info_hit_target_carries_selected_state() {
        assert_eq!(
            native_footer_left_hit_target_flags(true, true),
            NativeFooterLeftHitTargetFlags {
                selected: true,
                enabled: true,
            }
        );
    }

    #[test]
    fn cwd_chip_hit_target_carries_selected_state() {
        let flags = native_footer_left_hit_target_flags(true, true);
        assert!(flags.selected);
        assert!(flags.enabled);
    }

    #[test]
    fn left_visual_root_prefers_active_over_sibling_hover() {
        use crate::theme::MainMenuRowState::{Active, Hover, Rest};

        assert_eq!(native_footer_visual_root_state(None, Rest), Rest);
        assert_eq!(native_footer_visual_root_state(Some(Rest), Hover), Hover);
        assert_eq!(native_footer_visual_root_state(Some(Hover), Active), Active);
        assert_eq!(native_footer_visual_root_state(Some(Active), Hover), Active);
    }

    #[test]
    fn reused_left_hit_target_receives_fresh_state() {
        let initial = native_footer_left_hit_target_flags(false, true);
        let reused = native_footer_left_hit_target_flags(true, false);
        assert_ne!(initial, reused);
        assert!(reused.selected);
        assert!(!reused.enabled);
    }

    #[test]
    fn native_footer_visual_event_reports_only_signature_changes() {
        let id = "unit-native-footer-visual-event";
        assert!(native_footer_visual_event_changed(id, 1, 10, 0x112233));
        assert!(!native_footer_visual_event_changed(id, 1, 10, 0x112233));
        assert!(native_footer_visual_event_changed(id, 2, 10, 0x112233));
        assert!(!native_footer_visual_event_changed(id, 2, 10, 0x112233));
        assert!(native_footer_visual_event_changed(id, 2, 11, 0x112233));
        assert!(native_footer_visual_event_changed(id, 2, 11, 0x445566));
    }

    #[test]
    fn native_footer_hover_uses_hover_keycap_border_alpha() {
        let theme = crate::theme::Theme::dark_default();
        let visual_theme = resolve_native_footer_visual_theme(&theme);
        let rest = visual_theme.border_alpha(crate::theme::MainMenuRowState::Rest);
        let hover = visual_theme.border_alpha(crate::theme::MainMenuRowState::Hover);
        let active = visual_theme.border_alpha(crate::theme::MainMenuRowState::Active);

        assert_eq!(
            hover,
            crate::components::footer_chrome::footer_keycap_border_alpha_for_state(
                &theme,
                crate::theme::MainMenuRowState::Hover,
            )
        );
        assert!(hover >= rest);
        assert!(active >= rest);
    }

    #[test]
    fn native_footer_refresh_signature_tracks_canonical_palette() {
        let theme = crate::theme::Theme::dark_default();
        let palette =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
                .row_states;
        let baseline = native_footer_visual_theme_from_parts(palette, 0x112233, 0.1, 0.2, 0.3);

        let mut changed = palette;
        changed.hover.background_rgba = Some(0x44556677);
        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(changed, 0x112233, 0.1, 0.2, 0.3)
        );
        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(palette, 0x445566, 0.1, 0.2, 0.3)
        );
        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(palette, 0x112233, 0.1, 0.25, 0.3)
        );
    }

    #[test]
    fn native_footer_refresh_signature_changes_with_text_name_alpha() {
        let theme = crate::theme::Theme::dark_default();
        let palette =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
                .row_states;
        let baseline = native_footer_visual_theme_from_parts(palette, 0xffffff, 0.1, 0.2, 0.3);
        let mut changed = palette;
        changed.rest.primary_foreground_rgba =
            (changed.rest.primary_foreground_rgba & 0xffffff00) | 0x7f;

        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(changed, 0xffffff, 0.1, 0.2, 0.3)
        );
    }

    #[test]
    fn native_footer_refresh_signature_changes_with_row_kind_and_accent() {
        let theme = crate::theme::Theme::dark_default();
        let palette =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
                .row_states;
        let baseline = native_footer_visual_theme_from_parts(palette, 0xffffff, 0.1, 0.2, 0.3);
        let mut accent = palette;
        accent.active.background_rgba = Some(0x18a0fbff);
        accent.active.primary_foreground_rgba = 0x001122ff;

        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(accent, 0xffffff, 0.1, 0.2, 0.3)
        );
    }

    #[test]
    fn native_footer_theme_refresh_preserves_hover_state() {
        use crate::theme::MainMenuRowState::Hover;

        let theme = crate::theme::Theme::dark_default();
        let old_visual = resolve_native_footer_visual_theme(&theme);
        let mut new_palette = old_visual.row_palette;
        new_palette.hover.background_rgba = Some(0x44556612);
        let new_visual =
            native_footer_visual_theme_from_parts(new_palette, 0xffffff, 0.11, 0.37, 0.73);

        assert_eq!(
            resolved_native_footer_button_state(false, true, false, false),
            Hover
        );
        assert_eq!(
            new_visual.row_palette.for_state(Hover).background_rgba,
            Some(0x44556612)
        );
    }

    #[test]
    fn native_footer_theme_refresh_preserves_active_state() {
        use crate::theme::MainMenuRowState::Active;

        let theme = crate::theme::Theme::dark_default();
        let old_visual = resolve_native_footer_visual_theme(&theme);
        let mut new_palette = old_visual.row_palette;
        new_palette.active.background_rgba = Some(0x77889920);
        let new_visual =
            native_footer_visual_theme_from_parts(new_palette, 0xffffff, 0.11, 0.37, 0.73);

        assert_eq!(
            resolved_native_footer_button_state(true, true, false, false),
            Active
        );
        assert_eq!(
            new_visual.row_palette.for_state(Active).background_rgba,
            Some(0x77889920)
        );
    }

    #[test]
    fn footer_state_recolors_only_keycap_borders_not_glass_capsule_rims() {
        for identifier in [
            "script-kit-footer-keycap-actions-0",
            "script-kit-footer-left-info-keycap-0",
            "script-kit-footer-cwd-chip-keycap-0",
        ] {
            assert!(footer_identifier_uses_keycap_border(identifier));
        }
        for identifier in [
            "script-kit-footer-capsule-content-actions",
            "script-kit-footer-left-info-capsule-content",
            "script-kit-footer-state-layer-actions",
        ] {
            assert!(!footer_identifier_uses_keycap_border(identifier));
        }
    }

    #[test]
    fn run_hint_width_is_capped_to_stable_slot() {
        let buttons = vec![
            FooterButtonConfig::new(
                FooterAction::Run,
                "↵",
                "Open Screen Recording Permission Assistant",
            ),
            FooterButtonConfig::new(FooterAction::Ai, "⌘↵", "Agent"),
            FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions"),
        ];

        assert_eq!(
            footer_hint_max_item_width(FooterAction::Run, 480.0, &buttons),
            Some(242.0)
        );
        assert_eq!(
            footer_hint_max_item_width(FooterAction::Run, 640.0, &buttons),
            Some(242.0)
        );
        assert_eq!(
            footer_hint_max_item_width(FooterAction::Run, 120.0, &buttons),
            Some(92.0)
        );
        assert_eq!(
            footer_hint_max_item_width(FooterAction::Ai, 480.0, &buttons),
            None
        );
    }

    // The GPUI footer overlay no longer estimates label widths in Rust: the
    // Run button takes its intrinsic (text-measured) width via flexbox,
    // floored at the slot minimum and capped at FOOTER_RUN_SLOT_MAX_WIDTH_PX.
    // See tests/main_window_footer_surface_owner_contract.rs for the contract.

    #[test]
    fn run_hint_label_text_width_truncates_inside_remaining_slot() {
        let (chip_width, text_width) =
            footer_hint_label_widths(360.0, 5.0, 18.0, Some(180.0), 20.0, FOOTER_HINT_PADDING_X);

        // Derived from the shared chrome tokens so token tuning does not
        // invalidate the truncation contract being tested here.
        let expected_chip = 180.0 - FOOTER_HINT_PADDING_X * 2.0 - FOOTER_HINT_KEY_LABEL_GAP - 20.0;
        assert_eq!(chip_width, expected_chip);
        assert_eq!(text_width, expected_chip - 10.0);
        assert!(text_width < 360.0);
    }

    #[test]
    fn footer_buttons_keep_two_pixel_vertical_inset() {
        assert_eq!(
            crate::components::footer_chrome::FOOTER_BUTTON_VERTICAL_INSET_PX,
            2.0
        );
        assert_eq!(
            crate::components::footer_chrome::footer_button_height(32.0),
            28.0
        );
    }

    #[test]
    fn active_dot_prefers_the_most_contrasting_theme_color() {
        let mut theme = crate::theme::Theme::dark_default();
        theme.colors.background.main = 0x101114;
        theme.colors.accent.selected = 0x3a4250;
        theme.colors.text.primary = 0xf5f7fa;

        assert_eq!(
            footer_active_dot_hex(&theme, false),
            theme.colors.text.primary
        );

        theme.colors.accent.selected = 0xffc600;
        theme.colors.text.primary = 0x8892a0;
        assert_eq!(
            footer_active_dot_hex(&theme, false),
            theme.colors.accent.selected
        );
    }

    #[test]
    fn active_dot_can_force_accent_for_agent_chat_states() {
        let mut theme = crate::theme::Theme::dark_default();
        theme.colors.background.main = 0x101114;
        theme.colors.accent.selected = 0x3a4250;
        theme.colors.text.primary = 0xf5f7fa;

        assert_eq!(
            footer_active_dot_hex(&theme, true),
            theme.colors.accent.selected
        );
    }

    #[test]
    fn footer_dot_colors_follow_theme_tokens() {
        let mut theme = crate::theme::Theme::dark_default();
        theme.colors.text.secondary = 0x778899;
        theme.colors.ui.error = 0xaa3344;

        assert_eq!(
            footer_dot_hex(FooterDotStatus::Idle, &theme, false),
            theme.colors.text.secondary
        );
        assert_eq!(
            footer_dot_hex(FooterDotStatus::Error, &theme, false),
            theme.colors.ui.error
        );
    }
}
