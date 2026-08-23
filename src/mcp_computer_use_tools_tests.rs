#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        AutomationInspectSnapshot, AutomationWindowBounds, AutomationWindowInfo,
        AutomationWindowKind, AutomationWindowTarget, SemanticQuality, TargetWindowBounds,
        AUTOMATION_INSPECT_SCHEMA_VERSION, AUTOMATION_WINDOW_SCHEMA_VERSION,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeComputerUseRuntime;

    impl ComputerUseRuntimeBridge for FakeComputerUseRuntime {
        fn inspect_automation_window(
            &self,
            request: ComputerUseInspectRequest,
        ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
            assert_eq!(request.target, Some(AutomationWindowTarget::Focused));
            assert_eq!(request.hi_dpi, Some(true));
            assert_eq!(
                request.probes,
                vec![
                    crate::protocol::PixelProbe { x: 10, y: 20 },
                    crate::protocol::PixelProbe { x: 30, y: 40 },
                ]
            );

            Ok(AutomationInspectSnapshot {
                schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
                window_id: "main:0".to_string(),
                window_kind: "Main".to_string(),
                surface_kind: Some("ScriptList".to_string()),
                app_view_variant: Some("ScriptList".to_string()),
                native_footer_surface: Some("script_list".to_string()),
                target_generation: Some(1),
                surface_generation: Some(1),
                data_generation: Some(1),
                title: Some("Script Kit".to_string()),
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
                semantic_quality: Some(SemanticQuality::Full),
                warnings: Vec::new(),
                pid: Some(1234),
            })
        }

        fn list_running_apps(
            &self,
            request: ComputerUseListAppsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
            ComputerUseRuntimeError,
        > {
            assert!(request.include_hidden);
            assert!(request.include_background);

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                    apps: vec![
                        ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal".to_string(),
                            is_active: true,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        },
                        ComputerUseRunningAppInfo {
                            pid: 202,
                            bundle_id: None,
                            name: "Background Utility".to_string(),
                            is_active: false,
                            is_hidden: true,
                            activation_policy: "accessory".to_string(),
                        },
                    ],
                    frontmost_pid: Some(101),
                },
            )
        }

        fn list_app_windows(
            &self,
            request: ComputerUseListAppWindowsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
            ComputerUseRuntimeError,
        > {
            assert_eq!(request.pid, 101);

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                    app: Some(ComputerUseRunningAppInfo {
                        pid: 101,
                        bundle_id: Some("com.apple.Terminal".to_string()),
                        name: "Terminal".to_string(),
                        is_active: true,
                        is_hidden: false,
                        activation_policy: "regular".to_string(),
                    }),
                    windows: vec![ComputerUseAppWindowInfo {
                        native_window_id: 98765,
                        title: Some("Terminal".to_string()),
                        bounds: TargetWindowBounds {
                            x: 10,
                            y: 20,
                            width: 300,
                            height: 200,
                        },
                        is_on_screen: true,
                        layer: 0,
                        z_order: 0,
                        observation: None,
                    }],
                    warnings: Vec::new(),
                },
            )
        }
    }

    struct PanickingComputerUseRuntime;

    impl ComputerUseRuntimeBridge for PanickingComputerUseRuntime {
        fn inspect_automation_window(
            &self,
            _request: ComputerUseInspectRequest,
        ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
            panic!("computer/list_tray_menu must not inspect automation windows")
        }

        fn list_running_apps(
            &self,
            _request: ComputerUseListAppsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
            ComputerUseRuntimeError,
        > {
            panic!("computer/list_tray_menu must not list running apps")
        }

        fn list_app_windows(
            &self,
            _request: ComputerUseListAppWindowsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
            ComputerUseRuntimeError,
        > {
            panic!("computer/list_tray_menu must not list app windows")
        }
    }

    struct BundleIdAppsRuntime {
        fail_apps: bool,
    }

    impl ComputerUseRuntimeBridge for BundleIdAppsRuntime {
        fn inspect_automation_window(
            &self,
            _request: ComputerUseInspectRequest,
        ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
            panic!("computer/list_apps_by_bundle_id must not inspect automation windows")
        }

        fn list_running_apps(
            &self,
            request: ComputerUseListAppsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
            ComputerUseRuntimeError,
        > {
            if self.fail_apps {
                return Err(ComputerUseRuntimeError::Failed(
                    "failed to list running apps".to_string(),
                ));
            }

            assert!(request.include_hidden);
            assert!(request.include_background);

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                    apps: vec![
                        ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal".to_string(),
                            is_active: true,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        },
                        ComputerUseRunningAppInfo {
                            pid: 202,
                            bundle_id: Some("com.apple.TextEdit".to_string()),
                            name: "TextEdit".to_string(),
                            is_active: false,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        },
                        ComputerUseRunningAppInfo {
                            pid: 303,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal Helper".to_string(),
                            is_active: false,
                            is_hidden: true,
                            activation_policy: "accessory".to_string(),
                        },
                        ComputerUseRunningAppInfo {
                            pid: 404,
                            bundle_id: None,
                            name: "No Bundle".to_string(),
                            is_active: false,
                            is_hidden: true,
                            activation_policy: "accessory".to_string(),
                        },
                    ],
                    frontmost_pid: Some(101),
                },
            )
        }

        fn list_app_windows(
            &self,
            _request: ComputerUseListAppWindowsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
            ComputerUseRuntimeError,
        > {
            panic!("computer/list_apps_by_bundle_id must not list app windows")
        }
    }

    struct GroupedNativeWindowsRuntime {
        fail_pid: Option<i32>,
        missing_pid: Option<i32>,
    }

    impl ComputerUseRuntimeBridge for GroupedNativeWindowsRuntime {
        fn inspect_automation_window(
            &self,
            _request: ComputerUseInspectRequest,
        ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
            panic!("computer/list_native_windows must not inspect automation windows")
        }

        fn list_running_apps(
            &self,
            request: ComputerUseListAppsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
            ComputerUseRuntimeError,
        > {
            assert!(request.include_hidden);
            assert!(!request.include_background);

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                    apps: vec![
                        ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal".to_string(),
                            is_active: true,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        },
                        ComputerUseRunningAppInfo {
                            pid: 202,
                            bundle_id: Some("com.apple.TextEdit".to_string()),
                            name: "TextEdit".to_string(),
                            is_active: false,
                            is_hidden: true,
                            activation_policy: "regular".to_string(),
                        },
                    ],
                    frontmost_pid: Some(101),
                },
            )
        }

        fn list_app_windows(
            &self,
            request: ComputerUseListAppWindowsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
            ComputerUseRuntimeError,
        > {
            if self.fail_pid == Some(request.pid) {
                return Err(ComputerUseRuntimeError::Failed(format!(
                    "failed to list windows for pid {}",
                    request.pid
                )));
            }

            if self.missing_pid == Some(request.pid) {
                return Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: None,
                        windows: Vec::new(),
                        warnings: Vec::new(),
                    },
                );
            }

            let app = match request.pid {
                101 => ComputerUseRunningAppInfo {
                    pid: 101,
                    bundle_id: Some("com.apple.Terminal".to_string()),
                    name: "Terminal".to_string(),
                    is_active: true,
                    is_hidden: false,
                    activation_policy: "regular".to_string(),
                },
                202 => ComputerUseRunningAppInfo {
                    pid: 202,
                    bundle_id: Some("com.apple.TextEdit".to_string()),
                    name: "TextEdit".to_string(),
                    is_active: false,
                    is_hidden: true,
                    activation_policy: "regular".to_string(),
                },
                other => panic!("unexpected list_app_windows pid {other}"),
            };

            let windows = match request.pid {
                101 => vec![
                    ComputerUseAppWindowInfo {
                        native_window_id: 98765,
                        title: Some("Terminal".to_string()),
                        bounds: TargetWindowBounds {
                            x: 10,
                            y: 20,
                            width: 300,
                            height: 200,
                        },
                        is_on_screen: true,
                        layer: 0,
                        z_order: 0,
                        observation: None,
                    },
                    ComputerUseAppWindowInfo {
                        native_window_id: 98766,
                        title: Some("Terminal Settings".to_string()),
                        bounds: TargetWindowBounds {
                            x: 30,
                            y: 40,
                            width: 500,
                            height: 400,
                        },
                        is_on_screen: true,
                        layer: 0,
                        z_order: 1,
                        observation: None,
                    },
                ],
                202 => Vec::new(),
                _ => unreachable!(),
            };

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                    app: Some(app),
                    windows,
                    warnings: Vec::new(),
                },
            )
        }
    }

    struct BundleIdAppWindowsRuntime {
        fail_apps: bool,
        fail_pid: Option<i32>,
        missing_pid: Option<i32>,
    }

    impl ComputerUseRuntimeBridge for BundleIdAppWindowsRuntime {
        fn inspect_automation_window(
            &self,
            _request: ComputerUseInspectRequest,
        ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
            panic!("computer/list_app_windows_by_bundle_id must not inspect automation windows")
        }

        fn list_running_apps(
            &self,
            request: ComputerUseListAppsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
            ComputerUseRuntimeError,
        > {
            if self.fail_apps {
                return Err(ComputerUseRuntimeError::Failed(
                    "failed to list running apps".to_string(),
                ));
            }

            assert!(request.include_hidden);
            assert!(request.include_background);

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                    apps: vec![
                        ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal".to_string(),
                            is_active: true,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        },
                        ComputerUseRunningAppInfo {
                            pid: 202,
                            bundle_id: Some("com.apple.TextEdit".to_string()),
                            name: "TextEdit".to_string(),
                            is_active: false,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        },
                        ComputerUseRunningAppInfo {
                            pid: 303,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal Helper".to_string(),
                            is_active: false,
                            is_hidden: true,
                            activation_policy: "accessory".to_string(),
                        },
                    ],
                    frontmost_pid: Some(101),
                },
            )
        }

        fn list_app_windows(
            &self,
            request: ComputerUseListAppWindowsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
            ComputerUseRuntimeError,
        > {
            assert_ne!(
                request.pid, 202,
                "bundle-id lookup must not enumerate windows for non-matching bundle ids"
            );

            if self.fail_pid == Some(request.pid) {
                return Err(ComputerUseRuntimeError::Failed(format!(
                    "failed to list windows for pid {}",
                    request.pid
                )));
            }

            if self.missing_pid == Some(request.pid) {
                return Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: None,
                        windows: Vec::new(),
                        warnings: Vec::new(),
                    },
                );
            }

            let (app, windows, warnings) = match request.pid {
                101 => (
                    ComputerUseRunningAppInfo {
                        pid: 101,
                        bundle_id: Some("com.apple.Terminal".to_string()),
                        name: "Terminal".to_string(),
                        is_active: true,
                        is_hidden: false,
                        activation_policy: "regular".to_string(),
                    },
                    vec![
                        test_native_window(98765, 0, "Terminal"),
                        test_native_window(98766, 1, "Terminal Settings"),
                    ],
                    vec!["ignored offscreen windows".to_string()],
                ),
                303 => (
                    ComputerUseRunningAppInfo {
                        pid: 303,
                        bundle_id: Some("com.apple.Terminal".to_string()),
                        name: "Terminal Helper".to_string(),
                        is_active: false,
                        is_hidden: true,
                        activation_policy: "accessory".to_string(),
                    },
                    vec![test_native_window(98767, 0, "Terminal Helper")],
                    Vec::new(),
                ),
                other => panic!("unexpected list_app_windows pid {other}"),
            };

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                    app: Some(app),
                    windows,
                    warnings,
                },
            )
        }
    }

    struct NativeWindowLookupRuntime {
        fail_apps: bool,
        fail_pid: Option<i32>,
        missing_pid: Option<i32>,
    }

    impl ComputerUseRuntimeBridge for NativeWindowLookupRuntime {
        fn inspect_automation_window(
            &self,
            _request: ComputerUseInspectRequest,
        ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
            panic!("computer/get_native_window must not inspect automation windows")
        }

        fn list_running_apps(
            &self,
            request: ComputerUseListAppsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
            ComputerUseRuntimeError,
        > {
            if self.fail_apps {
                return Err(ComputerUseRuntimeError::Failed(
                    "failed to list running apps".to_string(),
                ));
            }

            assert!(request.include_hidden);
            assert!(request.include_background);

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                    apps: vec![
                        ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal".to_string(),
                            is_active: true,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        },
                        ComputerUseRunningAppInfo {
                            pid: 202,
                            bundle_id: Some("com.apple.TextEdit".to_string()),
                            name: "TextEdit".to_string(),
                            is_active: false,
                            is_hidden: true,
                            activation_policy: "regular".to_string(),
                        },
                    ],
                    frontmost_pid: Some(101),
                },
            )
        }

        fn list_app_windows(
            &self,
            request: ComputerUseListAppWindowsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
            ComputerUseRuntimeError,
        > {
            if self.fail_pid == Some(request.pid) {
                return Err(ComputerUseRuntimeError::Failed(format!(
                    "failed to list windows for pid {}",
                    request.pid
                )));
            }

            if self.missing_pid == Some(request.pid) {
                return Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: None,
                        windows: Vec::new(),
                        warnings: Vec::new(),
                    },
                );
            }

            let (app, windows) = match request.pid {
                101 => (
                    ComputerUseRunningAppInfo {
                        pid: 101,
                        bundle_id: Some("com.apple.Terminal".to_string()),
                        name: "Terminal".to_string(),
                        is_active: true,
                        is_hidden: false,
                        activation_policy: "regular".to_string(),
                    },
                    vec![ComputerUseAppWindowInfo {
                        native_window_id: 98765,
                        title: Some("Terminal".to_string()),
                        bounds: TargetWindowBounds {
                            x: 10,
                            y: 20,
                            width: 300,
                            height: 200,
                        },
                        is_on_screen: true,
                        layer: 0,
                        z_order: 0,
                        observation: None,
                    }],
                ),
                202 => (
                    ComputerUseRunningAppInfo {
                        pid: 202,
                        bundle_id: Some("com.apple.TextEdit".to_string()),
                        name: "TextEdit".to_string(),
                        is_active: false,
                        is_hidden: true,
                        activation_policy: "regular".to_string(),
                    },
                    Vec::new(),
                ),
                other => panic!("unexpected list_app_windows pid {other}"),
            };

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                    app: Some(app),
                    windows,
                    warnings: Vec::new(),
                },
            )
        }
    }

    struct FrontmostNativeWindowRuntime {
        frontmost_pid: Option<i32>,
        missing_app_window_pid: Option<i32>,
        windows: Vec<ComputerUseAppWindowInfo>,
    }

    impl ComputerUseRuntimeBridge for FrontmostNativeWindowRuntime {
        fn inspect_automation_window(
            &self,
            _request: ComputerUseInspectRequest,
        ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
            panic!("computer/get_frontmost_native_window must not inspect automation windows")
        }

        fn list_running_apps(
            &self,
            request: ComputerUseListAppsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
            ComputerUseRuntimeError,
        > {
            assert!(request.include_hidden);
            assert!(request.include_background);

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                    apps: vec![ComputerUseRunningAppInfo {
                        pid: 101,
                        bundle_id: Some("com.apple.Terminal".to_string()),
                        name: "Terminal".to_string(),
                        is_active: true,
                        is_hidden: false,
                        activation_policy: "regular".to_string(),
                    }],
                    frontmost_pid: self.frontmost_pid,
                },
            )
        }

        fn list_app_windows(
            &self,
            request: ComputerUseListAppWindowsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
            ComputerUseRuntimeError,
        > {
            assert_eq!(
                Some(request.pid),
                self.frontmost_pid,
                "frontmost native-window lookup must query only frontmostPid"
            );

            if self.missing_app_window_pid == Some(request.pid) {
                return Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: None,
                        windows: Vec::new(),
                        warnings: Vec::new(),
                    },
                );
            }

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                    app: Some(ComputerUseRunningAppInfo {
                        pid: request.pid,
                        bundle_id: Some("com.apple.Terminal".to_string()),
                        name: "Terminal".to_string(),
                        is_active: true,
                        is_hidden: false,
                        activation_policy: "regular".to_string(),
                    }),
                    windows: self.windows.clone(),
                    warnings: Vec::new(),
                },
            )
        }
    }

    struct ListFrontmostAppWindowsRuntime {
        frontmost_pid: Option<i32>,
        missing_app_window_pid: Option<i32>,
        fail_apps: bool,
        fail_windows: bool,
        windows: Vec<ComputerUseAppWindowInfo>,
        warnings: Vec<String>,
    }

    impl ComputerUseRuntimeBridge for ListFrontmostAppWindowsRuntime {
        fn inspect_automation_window(
            &self,
            _request: ComputerUseInspectRequest,
        ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
            panic!("computer/list_frontmost_app_windows must not inspect automation windows")
        }

        fn list_running_apps(
            &self,
            request: ComputerUseListAppsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
            ComputerUseRuntimeError,
        > {
            if self.fail_apps {
                return Err(ComputerUseRuntimeError::Failed(
                    "failed to list running apps".to_string(),
                ));
            }

            assert!(request.include_hidden);
            assert!(request.include_background);

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                    apps: vec![ComputerUseRunningAppInfo {
                        pid: 101,
                        bundle_id: Some("com.apple.Terminal".to_string()),
                        name: "Terminal".to_string(),
                        is_active: true,
                        is_hidden: false,
                        activation_policy: "regular".to_string(),
                    }],
                    frontmost_pid: self.frontmost_pid,
                },
            )
        }

        fn list_app_windows(
            &self,
            request: ComputerUseListAppWindowsRequest,
        ) -> Result<
            crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
            ComputerUseRuntimeError,
        > {
            if self.fail_windows {
                return Err(ComputerUseRuntimeError::Failed(format!(
                    "failed to list windows for pid {}",
                    request.pid
                )));
            }

            assert_eq!(
                Some(request.pid),
                self.frontmost_pid,
                "frontmost app-window list must query only frontmostPid"
            );

            if self.missing_app_window_pid == Some(request.pid) {
                return Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: None,
                        windows: Vec::new(),
                        warnings: Vec::new(),
                    },
                );
            }

            Ok(
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                    app: Some(ComputerUseRunningAppInfo {
                        pid: request.pid,
                        bundle_id: Some("com.apple.Terminal".to_string()),
                        name: "Terminal".to_string(),
                        is_active: true,
                        is_hidden: false,
                        activation_policy: "regular".to_string(),
                    }),
                    windows: self.windows.clone(),
                    warnings: self.warnings.clone(),
                },
            )
        }
    }

    #[test]
    fn computer_use_tool_definitions_are_registered() {
        let names: Vec<String> = get_computer_use_tool_definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect();

        assert_eq!(
            names,
            vec![
                COMPUTER_SEE_TOOL.to_string(),
                COMPUTER_LIST_WINDOWS_TOOL.to_string(),
                COMPUTER_GET_WINDOW_TOOL.to_string(),
                COMPUTER_GET_FOCUSED_WINDOW_TOOL.to_string(),
                COMPUTER_LIST_APPS_TOOL.to_string(),
                COMPUTER_GET_APP_TOOL.to_string(),
                COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL.to_string(),
                COMPUTER_LIST_APP_WINDOWS_TOOL.to_string(),
                COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL.to_string(),
                COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL.to_string(),
                COMPUTER_CAPTURE_NATIVE_WINDOW_TOOL.to_string(),
                COMPUTER_CAPTURE_RENDER_WINDOW_TOOL.to_string(),
                COMPUTER_LIST_NATIVE_WINDOWS_TOOL.to_string(),
                COMPUTER_GET_NATIVE_WINDOW_TOOL.to_string(),
                COMPUTER_GET_APP_WINDOW_TOOL.to_string(),
                COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL.to_string(),
                COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL.to_string(),
                COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL.to_string(),
                COMPUTER_GET_FRONTMOST_APP_TOOL.to_string(),
                COMPUTER_LIST_MENUS_TOOL.to_string(),
                COMPUTER_LIST_MENU_ITEM_PATHS_TOOL.to_string(),
                COMPUTER_GET_MENU_ITEM_TOOL.to_string(),
                COMPUTER_GET_MENU_ITEM_BY_INDEX_PATH_TOOL.to_string(),
                COMPUTER_LIST_TRAY_MENU_TOOL.to_string(),
                COMPUTER_GET_TRAY_MENU_ITEM_TOOL.to_string(),
                COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL.to_string(),
                COMPUTER_LIST_SCREENS_TOOL.to_string(),
                COMPUTER_GET_SCREEN_TOOL.to_string(),
                COMPUTER_LIST_PERMISSIONS_TOOL.to_string(),
                COMPUTER_GET_PERMISSION_TOOL.to_string()
            ]
        );
    }

    #[test]
    fn computer_see_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_SEE_TOOL)
            .expect("computer/see tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn computer_capture_render_window_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_CAPTURE_RENDER_WINDOW_TOOL)
            .expect("computer/capture_render_window tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("required")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("target")
        );
        assert!(tool
            .description
            .contains("does not prove macOS WindowServer compositor"));
    }

    #[test]
    fn computer_capture_render_window_rejects_mutating_arguments() {
        let result = handle_computer_use_tool_call(
            COMPUTER_CAPTURE_RENDER_WINDOW_TOOL,
            &serde_json::json!({
                "target": { "type": "focused" },
                "focus": true
            }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid error json");
        assert_eq!(value["errorCode"], "invalid_arguments");
        assert!(value["message"]
            .as_str()
            .unwrap_or_default()
            .contains("unknown field"));
    }

    #[test]
    fn computer_capture_render_window_without_runtime_returns_unsupported_receipt() {
        let result = handle_computer_use_tool_call(
            COMPUTER_CAPTURE_RENDER_WINDOW_TOOL,
            &serde_json::json!({ "target": { "type": "focused" } }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid render receipt");
        assert_eq!(value["source"], "gpuiRenderReadback");
        assert_eq!(value["status"], "unsupported");
        assert_eq!(value["error"]["code"], "runtime_unavailable");
        assert_eq!(value["capture"], serde_json::Value::Null);
        assert!(value["limitation"]
            .as_str()
            .unwrap_or_default()
            .contains("does not prove macOS WindowServer compositor"));
    }

    #[test]
    fn computer_list_windows_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_WINDOWS_TOOL)
            .expect("computer/list_windows tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn computer_get_focused_window_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_FOCUSED_WINDOW_TOOL)
            .expect("computer/get_focused_window tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn computer_get_window_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_WINDOW_TOOL)
            .expect("computer/get_window tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(
            properties
                .get("id")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["id"]))
        );
    }

    #[test]
    fn computer_list_apps_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_APPS_TOOL)
            .expect("computer/list_apps tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert!(properties.contains_key("includeHidden"));
        assert!(properties.contains_key("includeBackground"));
    }

    #[test]
    fn computer_get_app_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_APP_TOOL)
            .expect("computer/get_app tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        assert_eq!(
            properties
                .get("pid")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["pid"]))
        );
    }

    #[test]
    fn computer_list_apps_by_bundle_id_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL)
            .expect("computer/list_apps_by_bundle_id tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        let bundle_id = properties.get("bundleId").expect("bundleId schema");
        assert_eq!(
            bundle_id.get("type").and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(bundle_id.get("minLength").and_then(Value::as_u64), Some(1));
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["bundleId"]))
        );
    }

    #[test]
    fn computer_list_app_windows_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_APP_WINDOWS_TOOL)
            .expect("computer/list_app_windows tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        assert_eq!(
            properties
                .get("pid")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["pid"]))
        );
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL)
            .expect("computer/list_app_windows_by_bundle_id tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        let bundle_id = properties.get("bundleId").expect("bundleId schema");
        assert_eq!(
            bundle_id.get("type").and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(bundle_id.get("minLength").and_then(Value::as_u64), Some(1));
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["bundleId"]))
        );
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL)
            .expect("computer/get_app_window_by_bundle_id tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 2);
        let bundle_id = properties.get("bundleId").expect("bundleId schema");
        assert_eq!(
            bundle_id.get("type").and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(bundle_id.get("minLength").and_then(Value::as_u64), Some(1));
        let native_window_id = properties
            .get("nativeWindowId")
            .expect("nativeWindowId schema");
        assert_eq!(
            native_window_id.get("type").and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            native_window_id.get("minimum").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            native_window_id.get("maximum").and_then(Value::as_u64),
            Some(u32::MAX as u64)
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["bundleId", "nativeWindowId"]))
        );
    }

    #[test]
    fn computer_list_native_windows_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_NATIVE_WINDOWS_TOOL)
            .expect("computer/list_native_windows tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 2);
        assert!(properties.contains_key("includeHidden"));
        assert!(properties.contains_key("includeBackground"));
        assert!(tool.input_schema.get("required").is_none());
    }

    #[test]
    fn computer_get_native_window_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_NATIVE_WINDOW_TOOL)
            .expect("computer/get_native_window tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        let native_window_id = properties
            .get("nativeWindowId")
            .expect("nativeWindowId schema");
        assert_eq!(
            native_window_id.get("type").and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            native_window_id.get("minimum").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            native_window_id.get("maximum").and_then(Value::as_u64),
            Some(u32::MAX as u64)
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["nativeWindowId"]))
        );
    }

    #[test]
    fn computer_get_app_window_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_APP_WINDOW_TOOL)
            .expect("computer/get_app_window tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 2);
        assert_eq!(
            properties
                .get("pid")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            properties
                .get("nativeWindowId")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            properties
                .get("nativeWindowId")
                .and_then(|value| value.get("minimum"))
                .and_then(Value::as_i64),
            Some(0)
        );
        assert_eq!(
            properties
                .get("nativeWindowId")
                .and_then(|value| value.get("maximum"))
                .and_then(Value::as_u64),
            Some(u32::MAX as u64)
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["pid", "nativeWindowId"]))
        );
    }

    #[test]
    fn computer_get_frontmost_native_window_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL)
            .expect("computer/get_frontmost_native_window tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
        assert!(tool.input_schema.get("required").is_none());
    }

    #[test]
    fn computer_list_frontmost_app_windows_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL)
            .expect("computer/list_frontmost_app_windows tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
        assert!(tool.input_schema.get("required").is_none());
    }

    #[test]
    fn computer_get_frontmost_app_window_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL)
            .expect("computer/get_frontmost_app_window tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        let native_window_id = properties
            .get("nativeWindowId")
            .expect("nativeWindowId schema");
        assert_eq!(
            native_window_id.get("type").and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            native_window_id.get("minimum").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            native_window_id.get("maximum").and_then(Value::as_u64),
            Some(u32::MAX as u64)
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["nativeWindowId"]))
        );
    }

    #[test]
    fn computer_get_frontmost_app_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_FRONTMOST_APP_TOOL)
            .expect("computer/get_frontmost_app tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn computer_list_menus_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_MENUS_TOOL)
            .expect("computer/list_menus tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn computer_list_menu_item_paths_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_MENU_ITEM_PATHS_TOOL)
            .expect("computer/list_menu_item_paths tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn computer_get_menu_item_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_MENU_ITEM_TOOL)
            .expect("computer/get_menu_item tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        assert_eq!(
            properties
                .get("path")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("array")
        );
        assert_eq!(
            properties
                .get("path")
                .and_then(|value| value.get("minItems"))
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            properties
                .get("path")
                .and_then(|value| value.get("items"))
                .and_then(|items| items.get("type"))
                .and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(
            properties
                .get("path")
                .and_then(|value| value.get("items"))
                .and_then(|items| items.get("minLength"))
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["path"]))
        );
    }

    #[test]
    fn computer_get_menu_item_by_index_path_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_MENU_ITEM_BY_INDEX_PATH_TOOL)
            .expect("computer/get_menu_item_by_index_path tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        let index_path = properties.get("indexPath").expect("indexPath property");
        assert_eq!(
            index_path.get("type").and_then(Value::as_str),
            Some("array")
        );
        assert_eq!(index_path.get("minItems").and_then(Value::as_u64), Some(1));
        assert_eq!(
            index_path
                .get("items")
                .and_then(|items| items.get("type"))
                .and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            index_path
                .get("items")
                .and_then(|items| items.get("minimum"))
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["indexPath"]))
        );
    }

    #[test]
    fn computer_list_tray_menu_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_TRAY_MENU_TOOL)
            .expect("computer/list_tray_menu tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn computer_get_tray_menu_item_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_TRAY_MENU_ITEM_TOOL)
            .expect("computer/get_tray_menu_item tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 2);
        for key in ["sectionIndex", "itemIndex"] {
            let property = properties.get(key).expect("index property");
            assert_eq!(
                property.get("type").and_then(Value::as_str),
                Some("integer")
            );
            assert_eq!(property.get("minimum").and_then(Value::as_u64), Some(0));
        }
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["sectionIndex", "itemIndex"]))
        );
    }

    #[test]
    fn computer_get_tray_menu_item_by_id_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL)
            .expect("computer/get_tray_menu_item_by_id tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        let id_schema = properties.get("id").expect("id property");
        assert_eq!(
            id_schema.get("type").and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(id_schema.get("minLength").and_then(Value::as_u64), Some(1));
        assert_eq!(
            tool.input_schema.get("required"),
            Some(&serde_json::json!(["id"]))
        );
    }

    #[test]
    fn computer_list_permissions_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_PERMISSIONS_TOOL)
            .expect("computer/list_permissions tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn computer_get_permission_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_PERMISSION_TOOL)
            .expect("computer/get_permission tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        let id_schema = properties.get("id").expect("id schema");
        assert_eq!(
            id_schema.get("type").and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(
            id_schema.get("enum").and_then(Value::as_array).cloned(),
            Some(vec![
                serde_json::json!("accessibility"),
                serde_json::json!("screenRecording"),
                serde_json::json!("eventSynthesizing"),
            ])
        );
        assert_eq!(
            tool.input_schema
                .get("required")
                .and_then(Value::as_array)
                .cloned(),
            Some(vec![serde_json::json!("id")])
        );
    }

    #[test]
    fn computer_list_screens_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_LIST_SCREENS_TOOL)
            .expect("computer/list_screens tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tool.input_schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn computer_get_screen_tool_definition_has_closed_schema() {
        let tool = get_computer_use_tool_definitions()
            .into_iter()
            .find(|tool| tool.name == COMPUTER_GET_SCREEN_TOOL)
            .expect("computer/get_screen tool");

        assert_eq!(
            tool.input_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");
        assert_eq!(properties.len(), 1);
        assert_eq!(
            properties
                .get("displayId")
                .and_then(|schema| schema.get("type"))
                .and_then(Value::as_str),
            Some("integer")
        );
        assert_eq!(
            properties
                .get("displayId")
                .and_then(|schema| schema.get("minimum"))
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            properties
                .get("displayId")
                .and_then(|schema| schema.get("maximum"))
                .and_then(Value::as_u64),
            Some(u32::MAX as u64)
        );
        assert_eq!(
            tool.input_schema
                .get("required")
                .and_then(Value::as_array)
                .cloned(),
            Some(vec![serde_json::json!("displayId")])
        );
    }

    #[test]
    fn is_computer_use_tool_matches_only_computer_namespace() {
        assert!(is_computer_use_tool("computer/see"));
        assert!(!is_computer_use_tool("computer-use/see"));
        assert!(!is_computer_use_tool("kit/state"));
    }

    #[test]
    fn computer_see_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(COMPUTER_SEE_TOOL, &serde_json::json!({}), None);

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_list_apps_without_runtime_returns_tool_error() {
        let result =
            handle_computer_use_tool_call(COMPUTER_LIST_APPS_TOOL, &serde_json::json!({}), None);

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_get_app_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_TOOL,
            &serde_json::json!({ "pid": 101 }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_list_apps_by_bundle_id_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_get_app_window_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_TOOL,
            &serde_json::json!({ "pid": 101, "nativeWindowId": 98765 }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765 }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_list_native_windows_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_NATIVE_WINDOWS_TOOL,
            &serde_json::json!({}),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_get_native_window_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_NATIVE_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_get_frontmost_native_window_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL,
            &serde_json::json!({}),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_list_frontmost_app_windows_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
            &serde_json::json!({}),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_get_frontmost_app_window_without_runtime_returns_tool_error() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_list_tray_menu_without_runtime_returns_snapshot() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_TRAY_MENU_TOOL,
            &serde_json::json!({}),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_tray_menu json");
        assert_eq!(value["schemaVersion"], serde_json::json!(1));
        assert_eq!(value["source"], "scriptKitTrayMenuModel");
        assert_eq!(value["owner"]["scope"], "ownTrayMenuOnly");
        assert!(value["sections"].is_array());
        assert!(value["warnings"].is_array());
    }

    #[test]
    fn computer_see_with_runtime_returns_raw_snapshot() {
        let runtime = FakeComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_SEE_TOOL,
            &serde_json::json!({
                "target": { "type": "focused" },
                "hiDpi": true,
                "probes": [
                    { "x": 10, "y": 20 },
                    { "x": 30, "y": 40 }
                ]
            }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);

        let snapshot: AutomationInspectSnapshot =
            serde_json::from_str(&result.content[0].text).expect("automation inspect snapshot");
        assert_eq!(snapshot.schema_version, AUTOMATION_INSPECT_SCHEMA_VERSION);
        assert_eq!(snapshot.window_id, "main:0");
        assert!(!result.content[0].text.contains("\"action\""));
    }

    #[test]
    fn computer_list_apps_with_runtime_returns_running_app_snapshot() {
        let runtime = FakeComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APPS_TOOL,
            &serde_json::json!({
                "includeHidden": true,
                "includeBackground": true
            }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_apps json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_APPS_SCHEMA_VERSION)
        );
        assert_eq!(value["frontmostPid"], 101);

        let apps = value["apps"].as_array().expect("apps array");
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0]["pid"], 101);
        assert_eq!(apps[0]["bundleId"], "com.apple.Terminal");
        assert_eq!(apps[0]["name"], "Terminal");
        assert_eq!(apps[0]["isActive"], true);
        assert_eq!(apps[0]["isHidden"], false);
        assert_eq!(apps[0]["activationPolicy"], "regular");
        assert_eq!(apps[1]["bundleId"], serde_json::Value::Null);
        assert!(!result.content[0].text.contains("\"launch\""));
        assert!(!result.content[0].text.contains("\"quit\""));
        assert!(!result.content[0].text.contains("\"focus\""));
    }

    #[test]
    fn computer_get_app_returns_running_app_by_pid() {
        let runtime = FakeComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_TOOL,
            &serde_json::json!({ "pid": 101 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_app json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_APPS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "nsWorkspaceRunningApplications");
        assert_eq!(value["scope"], "runningAppPid");
        assert_eq!(value["status"], "found");
        assert_eq!(value["app"]["pid"], 101);
        assert_eq!(value["app"]["bundleId"], "com.apple.Terminal");
        assert_eq!(value["app"]["name"], "Terminal");
        assert_eq!(value["app"]["isActive"], true);
        assert_eq!(value["app"]["isHidden"], false);
        assert_eq!(value["app"]["activationPolicy"], "regular");
        assert!(value["warnings"].is_array());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"terminate\"",
            "\"forceTerminate\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_app result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_app_returns_not_found_for_unknown_pid() {
        let runtime = FakeComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_TOOL,
            &serde_json::json!({ "pid": 999 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_app json");
        assert_eq!(value["source"], "nsWorkspaceRunningApplications");
        assert_eq!(value["scope"], "runningAppPid");
        assert_eq!(value["status"], "notFound");
        assert!(value["app"].is_null());
        assert!(value["warnings"]
            .as_array()
            .is_some_and(|warnings| warnings.is_empty()));
    }

    #[test]
    fn computer_list_apps_by_bundle_id_returns_exact_matches() {
        let runtime = BundleIdAppsRuntime { fail_apps: false };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_apps_by_bundle_id json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_APPS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "nsWorkspaceRunningApplications");
        assert_eq!(value["scope"], "runningAppBundleId");
        assert_eq!(value["status"], "listed");
        assert_eq!(value["bundleId"], "com.apple.Terminal");
        assert_eq!(value["appCount"], 2);
        assert_eq!(value["apps"][0]["pid"], 101);
        assert_eq!(value["apps"][0]["bundleId"], "com.apple.Terminal");
        assert_eq!(value["apps"][1]["pid"], 303);
        assert_eq!(value["apps"][1]["bundleId"], "com.apple.Terminal");
        assert!(value["warnings"].as_array().unwrap().is_empty());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"screenshot\"",
            "\"capture\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/list_apps_by_bundle_id result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_list_apps_by_bundle_id_returns_not_found() {
        let runtime = BundleIdAppsRuntime { fail_apps: false };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Missing" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_apps_by_bundle_id json");
        assert_eq!(value["source"], "nsWorkspaceRunningApplications");
        assert_eq!(value["scope"], "runningAppBundleId");
        assert_eq!(value["status"], "notFound");
        assert_eq!(value["bundleId"], "com.apple.Missing");
        assert_eq!(value["appCount"], 0);
        assert!(value["apps"].as_array().unwrap().is_empty());
        assert!(value["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_list_apps_by_bundle_id_propagates_runtime_failure() {
        let runtime = BundleIdAppsRuntime { fail_apps: true };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("inspection_failed"));
        assert!(result.content[0]
            .text
            .contains("failed to list running apps"));
    }

    #[test]
    fn computer_get_app_window_returns_window_by_pid_and_native_window_id() {
        let runtime = FakeComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_TOOL,
            &serde_json::json!({ "pid": 101, "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_app_window json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_APP_WINDOWS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "coreGraphicsWindowList");
        assert_eq!(value["scope"], "runningAppPidNativeWindowId");
        assert_eq!(value["status"], "found");
        assert_eq!(value["app"]["pid"], 101);
        assert_eq!(value["window"]["nativeWindowId"], 98765);
        assert_eq!(value["window"]["title"], "Terminal");

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_app_window result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_app_window_returns_window_not_found_for_unknown_native_window_id() {
        let runtime = FakeComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_TOOL,
            &serde_json::json!({ "pid": 101, "nativeWindowId": 11111 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_app_window json");
        assert_eq!(value["source"], "coreGraphicsWindowList");
        assert_eq!(value["scope"], "runningAppPidNativeWindowId");
        assert_eq!(value["status"], "windowNotFound");
        assert_eq!(value["app"]["pid"], 101);
        assert!(value["window"].is_null());
    }

    #[test]
    fn computer_get_app_window_returns_app_not_found_for_unknown_pid() {
        struct MissingAppWindowRuntime;

        impl ComputerUseRuntimeBridge for MissingAppWindowRuntime {
            fn inspect_automation_window(
                &self,
                _request: ComputerUseInspectRequest,
            ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
                panic!("computer/get_app_window must not inspect automation windows")
            }

            fn list_running_apps(
                &self,
                _request: ComputerUseListAppsRequest,
            ) -> Result<
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
                ComputerUseRuntimeError,
            > {
                panic!("computer/get_app_window must not list apps directly")
            }

            fn list_app_windows(
                &self,
                request: ComputerUseListAppWindowsRequest,
            ) -> Result<
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
                ComputerUseRuntimeError,
            > {
                assert_eq!(request.pid, 999);

                Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: None,
                        windows: Vec::new(),
                        warnings: Vec::new(),
                    },
                )
            }
        }

        let runtime = MissingAppWindowRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_TOOL,
            &serde_json::json!({ "pid": 999, "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_app_window json");
        assert_eq!(value["source"], "coreGraphicsWindowList");
        assert_eq!(value["scope"], "runningAppPidNativeWindowId");
        assert_eq!(value["status"], "appNotFound");
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
    }

    #[test]
    fn computer_see_rejects_max_elements_instead_of_truncating() {
        let result = handle_computer_use_tool_call(
            COMPUTER_SEE_TOOL,
            &serde_json::json!({ "maxElements": 1 }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("invalid_arguments"));
    }

    #[test]
    fn computer_see_rejects_bad_arguments() {
        let result = handle_computer_use_tool_call(
            COMPUTER_SEE_TOOL,
            &serde_json::json!({ "unknown": true }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("invalid_arguments"));
    }

    #[test]
    fn computer_list_windows_rejects_bad_arguments() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_WINDOWS_TOOL,
            &serde_json::json!({ "target": { "type": "focused" } }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("invalid_arguments"));
    }

    #[test]
    fn computer_get_focused_window_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!({ "target": { "type": "focused" } }),
            serde_json::json!({ "focus": true }),
            serde_json::json!({ "activate": true }),
            serde_json::json!({ "refresh": true }),
            serde_json::json!({ "click": true }),
            serde_json::json!({ "id": "main" }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_GET_FOCUSED_WINDOW_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_window_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "id": 123 }),
            serde_json::json!({ "id": null }),
            serde_json::json!({ "target": { "type": "focused" } }),
            serde_json::json!({ "id": "main", "focus": true }),
            serde_json::json!({ "id": "main", "activate": true }),
            serde_json::json!({ "id": "main", "refresh": true }),
            serde_json::json!({ "id": "main", "click": true }),
            serde_json::json!({ "id": "main", "includeElements": true }),
            serde_json::json!({ "id": "main", "screenshot": true }),
        ] {
            let result = handle_computer_use_tool_call(COMPUTER_GET_WINDOW_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_window_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_WINDOW_TOOL,
            &serde_json::json!({ "id": "missing-window-id-for-runtime-test" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_window json");
        assert_eq!(value["source"], "automationWindowRegistry");
        assert_eq!(value["status"], "notFound");
        assert!(value["window"].is_null());
    }

    #[test]
    fn computer_list_apps_rejects_bad_arguments() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APPS_TOOL,
            &serde_json::json!({ "launch": true }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("invalid_arguments"));
    }

    #[test]
    fn computer_get_app_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "pid": "101" }),
            serde_json::json!({ "pid": 101, "focus": true }),
            serde_json::json!({ "pid": 101, "activate": true }),
            serde_json::json!({ "pid": 101, "launch": true }),
            serde_json::json!({ "pid": 101, "quit": true }),
            serde_json::json!({ "pid": 101, "hide": true }),
            serde_json::json!({ "pid": 101, "includeWindows": true }),
        ] {
            let result = handle_computer_use_tool_call(COMPUTER_GET_APP_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_apps_by_bundle_id_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "bundleId": "" }),
            serde_json::json!({ "bundleId": 101 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "pid": 101 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "includeHidden": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "includeBackground": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "focus": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "activate": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "launch": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "quit": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "hide": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "move": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "resize": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "screenshot": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "capture": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "click": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "press": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "execute": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "input": "x" }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "typeText": "x" }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "key": "Enter" }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "includeGlobalStatusItems": true }),
        ] {
            let result = handle_computer_use_tool_call(
                COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL,
                &arguments,
                None,
            );

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_app_windows_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "pid": "101" }),
            serde_json::json!({ "pid": 101, "focus": true }),
            serde_json::json!({ "pid": 101, "activate": true }),
            serde_json::json!({ "pid": 101, "move": true }),
            serde_json::json!({ "pid": 101, "resize": true }),
            serde_json::json!({ "pid": 101, "screenshot": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_LIST_APP_WINDOWS_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "bundleId": "" }),
            serde_json::json!({ "bundleId": 101 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "pid": 101 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "includeHidden": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "includeBackground": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "focus": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "activate": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "launch": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "quit": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "hide": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "move": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "resize": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "setBounds": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "screenshot": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "capture": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "click": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "press": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "execute": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "AXPress": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "input": "x" }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "typeText": "x" }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "key": "Enter" }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "includeGlobalStatusItems": true }),
        ] {
            let result = handle_computer_use_tool_call(
                COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL,
                &arguments,
                None,
            );

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "bundleId": "" }),
            serde_json::json!({ "bundleId": 101, "nativeWindowId": 98765 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            serde_json::json!({ "nativeWindowId": 98765 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": "98765" }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": -1 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 4294967296u64 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "pid": 101 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "includeHidden": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "includeBackground": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "focus": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "activate": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "launch": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "quit": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "hide": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "move": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "resize": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "setBounds": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "screenshot": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "capture": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "click": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "press": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "execute": true }),
            serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765, "AXPress": true }),
        ] {
            let result = handle_computer_use_tool_call(
                COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
                &arguments,
                None,
            );

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_native_windows_rejects_bad_arguments() {
        // NOTE: `json!([])` is intentionally absent: serde deserializes JSON
        // arrays into structs positionally, and every field here has a
        // default, so `[]` parses as "no args" and reaches the
        // runtime_unavailable path instead of invalid_arguments.
        for arguments in [
            serde_json::json!(null),
            serde_json::json!({ "pid": 101 }),
            serde_json::json!({ "app": "Terminal" }),
            serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            serde_json::json!({ "includeHidden": "yes" }),
            serde_json::json!({ "includeBackground": "yes" }),
            serde_json::json!({ "focus": true }),
            serde_json::json!({ "activate": true }),
            serde_json::json!({ "launch": true }),
            serde_json::json!({ "quit": true }),
            serde_json::json!({ "hide": true }),
            serde_json::json!({ "move": true }),
            serde_json::json!({ "resize": true }),
            serde_json::json!({ "setBounds": true }),
            serde_json::json!({ "screenshot": true }),
            serde_json::json!({ "capture": true }),
            serde_json::json!({ "click": true }),
            serde_json::json!({ "press": true }),
            serde_json::json!({ "execute": true }),
            serde_json::json!({ "includeGlobalStatusItems": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_LIST_NATIVE_WINDOWS_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_native_window_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "nativeWindowId": "98765" }),
            serde_json::json!({ "nativeWindowId": -1 }),
            serde_json::json!({ "nativeWindowId": 4294967296u64 }),
            serde_json::json!({ "nativeWindowId": 98765, "pid": 101 }),
            serde_json::json!({ "nativeWindowId": 98765, "app": "Terminal" }),
            serde_json::json!({ "nativeWindowId": 98765, "bundleId": "com.apple.Terminal" }),
            serde_json::json!({ "nativeWindowId": 98765, "includeHidden": true }),
            serde_json::json!({ "nativeWindowId": 98765, "includeBackground": true }),
            serde_json::json!({ "nativeWindowId": 98765, "focus": true }),
            serde_json::json!({ "nativeWindowId": 98765, "activate": true }),
            serde_json::json!({ "nativeWindowId": 98765, "launch": true }),
            serde_json::json!({ "nativeWindowId": 98765, "quit": true }),
            serde_json::json!({ "nativeWindowId": 98765, "hide": true }),
            serde_json::json!({ "nativeWindowId": 98765, "move": true }),
            serde_json::json!({ "nativeWindowId": 98765, "resize": true }),
            serde_json::json!({ "nativeWindowId": 98765, "setBounds": true }),
            serde_json::json!({ "nativeWindowId": 98765, "screenshot": true }),
            serde_json::json!({ "nativeWindowId": 98765, "capture": true }),
            serde_json::json!({ "nativeWindowId": 98765, "click": true }),
            serde_json::json!({ "nativeWindowId": 98765, "press": true }),
            serde_json::json!({ "nativeWindowId": 98765, "execute": true }),
            serde_json::json!({ "nativeWindowId": 98765, "AXPress": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_GET_NATIVE_WINDOW_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_frontmost_native_window_rejects_bad_arguments() {
        // NOTE: `json!([])` is intentionally absent: serde deserializes JSON
        // arrays into structs positionally, and this tool takes no fields, so
        // `[]` parses as "no args" and reaches the runtime_unavailable path
        // instead of invalid_arguments.
        for arguments in [
            serde_json::json!(null),
            serde_json::json!({ "pid": 101 }),
            serde_json::json!({ "nativeWindowId": 98765 }),
            serde_json::json!({ "includeHidden": true }),
            serde_json::json!({ "includeBackground": true }),
            serde_json::json!({ "focus": true }),
            serde_json::json!({ "activate": true }),
            serde_json::json!({ "launch": true }),
            serde_json::json!({ "quit": true }),
            serde_json::json!({ "hide": true }),
            serde_json::json!({ "move": true }),
            serde_json::json!({ "resize": true }),
            serde_json::json!({ "setBounds": true }),
            serde_json::json!({ "screenshot": true }),
            serde_json::json!({ "capture": true }),
            serde_json::json!({ "click": true }),
            serde_json::json!({ "press": true }),
            serde_json::json!({ "execute": true }),
            serde_json::json!({ "AXPress": true }),
            serde_json::json!({ "includeGlobalStatusItems": true }),
        ] {
            let result = handle_computer_use_tool_call(
                COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL,
                &arguments,
                None,
            );

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_frontmost_app_windows_rejects_bad_arguments() {
        // NOTE: `json!([])` is intentionally absent: serde deserializes JSON
        // arrays into structs positionally, and this tool takes no fields, so
        // `[]` parses as "no args" and reaches the runtime_unavailable path
        // instead of invalid_arguments.
        for arguments in [
            serde_json::json!(null),
            serde_json::json!({ "pid": 101 }),
            serde_json::json!({ "nativeWindowId": 98765 }),
            serde_json::json!({ "includeHidden": true }),
            serde_json::json!({ "includeBackground": true }),
            serde_json::json!({ "focus": true }),
            serde_json::json!({ "activate": true }),
            serde_json::json!({ "launch": true }),
            serde_json::json!({ "quit": true }),
            serde_json::json!({ "hide": true }),
            serde_json::json!({ "move": true }),
            serde_json::json!({ "resize": true }),
            serde_json::json!({ "setBounds": true }),
            serde_json::json!({ "screenshot": true }),
            serde_json::json!({ "capture": true }),
            serde_json::json!({ "click": true }),
            serde_json::json!({ "press": true }),
            serde_json::json!({ "execute": true }),
            serde_json::json!({ "AXPress": true }),
            serde_json::json!({ "includeGlobalStatusItems": true }),
        ] {
            let result = handle_computer_use_tool_call(
                COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
                &arguments,
                None,
            );

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_frontmost_app_window_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "nativeWindowId": "98765" }),
            serde_json::json!({ "nativeWindowId": -1 }),
            serde_json::json!({ "nativeWindowId": 4294967296u64 }),
            serde_json::json!({ "nativeWindowId": 98765, "pid": 101 }),
            serde_json::json!({ "nativeWindowId": 98765, "includeHidden": true }),
            serde_json::json!({ "nativeWindowId": 98765, "includeBackground": true }),
            serde_json::json!({ "nativeWindowId": 98765, "focus": true }),
            serde_json::json!({ "nativeWindowId": 98765, "activate": true }),
            serde_json::json!({ "nativeWindowId": 98765, "launch": true }),
            serde_json::json!({ "nativeWindowId": 98765, "quit": true }),
            serde_json::json!({ "nativeWindowId": 98765, "hide": true }),
            serde_json::json!({ "nativeWindowId": 98765, "move": true }),
            serde_json::json!({ "nativeWindowId": 98765, "resize": true }),
            serde_json::json!({ "nativeWindowId": 98765, "setBounds": true }),
            serde_json::json!({ "nativeWindowId": 98765, "screenshot": true }),
            serde_json::json!({ "nativeWindowId": 98765, "capture": true }),
            serde_json::json!({ "nativeWindowId": 98765, "click": true }),
            serde_json::json!({ "nativeWindowId": 98765, "press": true }),
            serde_json::json!({ "nativeWindowId": 98765, "execute": true }),
            serde_json::json!({ "nativeWindowId": 98765, "AXPress": true }),
            serde_json::json!({ "nativeWindowId": 98765, "includeGlobalStatusItems": true }),
        ] {
            let result = handle_computer_use_tool_call(
                COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
                &arguments,
                None,
            );

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_app_window_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "pid": 101 }),
            serde_json::json!({ "nativeWindowId": 98765 }),
            serde_json::json!({ "pid": "101", "nativeWindowId": 98765 }),
            serde_json::json!({ "pid": 101, "nativeWindowId": "98765" }),
            serde_json::json!({ "pid": 101, "nativeWindowId": -1 }),
            serde_json::json!({ "pid": 101, "nativeWindowId": 4294967296u64 }),
            serde_json::json!({ "pid": 101, "nativeWindowId": 98765, "focus": true }),
            serde_json::json!({ "pid": 101, "nativeWindowId": 98765, "activate": true }),
            serde_json::json!({ "pid": 101, "nativeWindowId": 98765, "move": true }),
            serde_json::json!({ "pid": 101, "nativeWindowId": 98765, "resize": true }),
            serde_json::json!({ "pid": 101, "nativeWindowId": 98765, "screenshot": true }),
            serde_json::json!({ "pid": 101, "nativeWindowId": 98765, "click": true }),
            serde_json::json!({ "pid": 101, "nativeWindowId": 98765, "AXPress": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_GET_APP_WINDOW_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_frontmost_app_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!({ "refresh": true }),
            serde_json::json!({ "focus": true }),
            serde_json::json!({ "activate": true }),
            serde_json::json!({ "pid": 123 }),
            serde_json::json!({ "bundleId": "com.apple.Safari" }),
            serde_json::json!({ "includeMenus": true }),
            serde_json::json!({ "click": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_GET_FRONTMOST_APP_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_menus_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!({ "pid": 101 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            serde_json::json!({ "refresh": true }),
            serde_json::json!({ "target": "frontmost" }),
            serde_json::json!({ "click": true }),
            serde_json::json!({ "path": [0, 1] }),
            serde_json::json!({ "includeDisabled": true }),
        ] {
            let result = handle_computer_use_tool_call(COMPUTER_LIST_MENUS_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_menu_item_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "path": [] }),
            serde_json::json!({ "path": ["File", ""] }),
            serde_json::json!({ "path": [0, 1] }),
            serde_json::json!({ "path": "File" }),
            serde_json::json!({ "path": ["File"], "pid": 101 }),
            serde_json::json!({ "path": ["File"], "bundleId": "com.apple.Terminal" }),
            serde_json::json!({ "path": ["File"], "refresh": true }),
            serde_json::json!({ "path": ["File"], "focus": true }),
            serde_json::json!({ "path": ["File"], "activate": true }),
            serde_json::json!({ "path": ["File"], "click": true }),
            serde_json::json!({ "path": ["File"], "execute": true }),
            serde_json::json!({ "path": ["File"], "includeDisabled": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_GET_MENU_ITEM_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_menu_item_paths_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!({ "path": ["File"] }),
            serde_json::json!({ "indexPath": [0] }),
            serde_json::json!({ "pid": 123 }),
            serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            serde_json::json!({ "refresh": true }),
            serde_json::json!({ "focus": true }),
            serde_json::json!({ "activate": true }),
            serde_json::json!({ "click": true }),
            serde_json::json!({ "press": true }),
            serde_json::json!({ "execute": true }),
            serde_json::json!({ "includeDisabled": true }),
            serde_json::json!({ "includeGlobalStatusItems": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_LIST_MENU_ITEM_PATHS_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_menu_item_by_index_path_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "indexPath": [] }),
            serde_json::json!({ "indexPath": "0.1" }),
            serde_json::json!({ "indexPath": [0, "1"] }),
            serde_json::json!({ "indexPath": [-1] }),
            serde_json::json!({ "indexPath": [0], "click": true }),
            serde_json::json!({ "indexPath": [0], "press": true }),
            serde_json::json!({ "indexPath": [0], "execute": true }),
            serde_json::json!({ "indexPath": [0], "refresh": true }),
            serde_json::json!({ "indexPath": [0], "focus": true }),
            serde_json::json!({ "indexPath": [0], "activate": true }),
            serde_json::json!({ "indexPath": [0], "pid": 123 }),
        ] {
            let result = handle_computer_use_tool_call(
                COMPUTER_GET_MENU_ITEM_BY_INDEX_PATH_TOOL,
                &arguments,
                None,
            );

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_tray_menu_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!({ "click": true }),
            serde_json::json!({ "execute": true }),
            serde_json::json!({ "index": 0 }),
            serde_json::json!({ "itemName": "GitHub" }),
            serde_json::json!({ "actionId": "tray.open_github" }),
            serde_json::json!({ "open": true }),
            serde_json::json!({ "target": "menubar" }),
            serde_json::json!({ "includeGlobalStatusItems": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_LIST_TRAY_MENU_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_tray_menu_item_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "sectionIndex": 0 }),
            serde_json::json!({ "itemIndex": 0 }),
            serde_json::json!({ "sectionIndex": -1, "itemIndex": 0 }),
            serde_json::json!({ "sectionIndex": 0, "itemIndex": -1 }),
            serde_json::json!({ "sectionIndex": "0", "itemIndex": 0 }),
            serde_json::json!({ "sectionIndex": 0, "itemIndex": "0" }),
            serde_json::json!({ "sectionIndex": 0, "itemIndex": 0, "click": true }),
            serde_json::json!({ "sectionIndex": 0, "itemIndex": 0, "press": true }),
            serde_json::json!({ "sectionIndex": 0, "itemIndex": 0, "execute": true }),
            serde_json::json!({ "sectionIndex": 0, "itemIndex": 0, "open": true }),
            serde_json::json!({ "sectionIndex": 0, "itemIndex": 0, "refresh": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_GET_TRAY_MENU_ITEM_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_permissions_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!({ "request": true }),
            serde_json::json!({ "grant": true }),
            serde_json::json!({ "openSettings": true }),
            serde_json::json!({ "requestEventSynthesizing": true }),
            serde_json::json!({ "includeGrantInstructions": true }),
            serde_json::json!({ "click": true }),
            serde_json::json!({ "press": true }),
            serde_json::json!({ "execute": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_LIST_PERMISSIONS_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_permission_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "id": 123 }),
            serde_json::json!({ "id": "screenRecording", "request": true }),
            serde_json::json!({ "id": "screenRecording", "grant": true }),
            serde_json::json!({ "id": "screenRecording", "openSettings": true }),
            serde_json::json!({ "id": "screenRecording", "click": true }),
            serde_json::json!({ "id": "screenRecording", "press": true }),
            serde_json::json!({ "id": "screenRecording", "execute": true }),
        ] {
            let result =
                handle_computer_use_tool_call(COMPUTER_GET_PERMISSION_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_get_tray_menu_item_by_id_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "id": "" }),
            serde_json::json!({ "id": 123 }),
            serde_json::json!({ "id": "tray.open_script_kit", "click": true }),
            serde_json::json!({ "id": "tray.open_script_kit", "press": true }),
            serde_json::json!({ "id": "tray.open_script_kit", "execute": true }),
            serde_json::json!({ "id": "tray.open_script_kit", "open": true }),
            serde_json::json!({ "id": "tray.open_script_kit", "refresh": true }),
            serde_json::json!({ "id": "tray.open_script_kit", "includeGlobalStatusItems": true }),
        ] {
            let result = handle_computer_use_tool_call(
                COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL,
                &arguments,
                None,
            );

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_screens_rejects_bad_arguments() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_SCREENS_TOOL,
            &serde_json::json!({ "move": true }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("invalid_arguments"));
    }

    #[test]
    fn computer_get_screen_rejects_bad_arguments() {
        for arguments in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({ "displayId": "1" }),
            serde_json::json!({ "displayId": -1 }),
            serde_json::json!({ "displayId": 4_294_967_296u64 }),
            serde_json::json!({ "displayId": 0, "move": true }),
            serde_json::json!({ "displayId": 0, "resize": true }),
            serde_json::json!({ "displayId": 0, "screenshot": true }),
            serde_json::json!({ "displayId": 0, "capture": true }),
            serde_json::json!({ "displayId": 0, "requestPermission": true }),
            serde_json::json!({ "displayId": 0, "click": true }),
            serde_json::json!({ "displayId": 0, "press": true }),
            serde_json::json!({ "displayId": 0, "execute": true }),
        ] {
            let result = handle_computer_use_tool_call(COMPUTER_GET_SCREEN_TOOL, &arguments, None);

            assert_eq!(result.is_error, Some(true));
            assert!(result.content[0].text.contains("invalid_arguments"));
        }
    }

    #[test]
    fn computer_list_windows_returns_registry_snapshot_without_runtime() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let id = format!("mcp-list-windows-test-{nonce}");

        crate::windows::upsert_automation_window(AutomationWindowInfo {
            id: id.clone(),
            kind: AutomationWindowKind::Notes,
            title: Some("MCP List Windows Test".to_string()),
            focused: false,
            visible: true,
            semantic_surface: Some("notes".to_string()),
            bounds: Some(AutomationWindowBounds {
                x: 10.0,
                y: 20.0,
                width: 300.0,
                height: 200.0,
            }),
            parent_window_id: None,
            parent_kind: None,
            pid: Some(1234),
            generation: None,
        });

        let result =
            handle_computer_use_tool_call(COMPUTER_LIST_WINDOWS_TOOL, &serde_json::json!({}), None);

        crate::windows::remove_automation_window(&id);

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_windows json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(AUTOMATION_WINDOW_SCHEMA_VERSION)
        );
        assert!(value["focusedWindowId"].is_null() || value["focusedWindowId"].is_string());

        let windows = value["windows"].as_array().expect("windows array");
        let window = windows
            .iter()
            .find(|window| window["id"] == id)
            .expect("registered test window should be listed");
        assert_eq!(window["kind"], "notes");
        assert_eq!(window["visible"], true);
        assert_eq!(window["semanticSurface"], "notes");
    }

    #[test]
    fn computer_get_focused_window_returns_registry_snapshot_without_runtime() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let id = format!("mcp-focused-window-test-{nonce}");

        crate::windows::upsert_automation_window(AutomationWindowInfo {
            id: id.clone(),
            kind: AutomationWindowKind::Notes,
            title: Some("MCP Focused Window Test".to_string()),
            focused: false,
            visible: true,
            semantic_surface: Some("notes".to_string()),
            bounds: Some(AutomationWindowBounds {
                x: 10.0,
                y: 20.0,
                width: 300.0,
                height: 200.0,
            }),
            parent_window_id: None,
            parent_kind: None,
            pid: Some(1234),
            generation: None,
        });
        assert!(crate::windows::set_automation_focus(&id));

        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FOCUSED_WINDOW_TOOL,
            &serde_json::json!({}),
            None,
        );

        crate::windows::remove_automation_window(&id);

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_focused_window json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(AUTOMATION_WINDOW_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "automationWindowRegistry");
        assert_eq!(value["scope"], "focusedAutomationWindow");
        assert_eq!(value["status"], "focused");
        assert_eq!(value["focusedWindowId"], id);
        assert_eq!(value["window"]["id"], id);
        assert_eq!(value["window"]["kind"], "notes");
        assert_eq!(value["window"]["focused"], true);
        assert_eq!(value["window"]["visible"], true);
        assert_eq!(value["window"]["semanticSurface"], "notes");
        assert!(value["warnings"].is_array());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_focused_window result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_window_returns_registry_snapshot_by_id_without_runtime() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let id = format!("mcp-get-window-test-{nonce}");

        crate::windows::upsert_automation_window(AutomationWindowInfo {
            id: id.clone(),
            kind: AutomationWindowKind::Notes,
            title: Some("MCP Get Window Test".to_string()),
            focused: false,
            visible: true,
            semantic_surface: Some("notes".to_string()),
            bounds: Some(AutomationWindowBounds {
                x: 10.0,
                y: 20.0,
                width: 300.0,
                height: 200.0,
            }),
            parent_window_id: None,
            parent_kind: None,
            pid: Some(1234),
            generation: None,
        });

        let result = handle_computer_use_tool_call(
            COMPUTER_GET_WINDOW_TOOL,
            &serde_json::json!({ "id": id.clone() }),
            None,
        );

        crate::windows::remove_automation_window(&id);

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_window json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(AUTOMATION_WINDOW_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "automationWindowRegistry");
        assert_eq!(value["status"], "found");
        assert_eq!(value["window"]["id"], id);
        assert_eq!(value["window"]["kind"], "notes");
        assert_eq!(value["window"]["visible"], true);
        assert_eq!(value["window"]["semanticSurface"], "notes");
        assert!(value["warnings"].is_array());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_window result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_window_returns_not_found_for_unknown_id_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_WINDOW_TOOL,
            &serde_json::json!({ "id": "missing-window-id-for-test" }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_window json");
        assert_eq!(value["source"], "automationWindowRegistry");
        assert_eq!(value["status"], "notFound");
        assert!(value["window"].is_null());
        assert!(value["warnings"]
            .as_array()
            .is_some_and(|warnings| warnings.is_empty()));
    }

    #[test]
    fn computer_get_frontmost_app_returns_cached_snapshot_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_TOOL,
            &serde_json::json!({}),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_frontmost_app json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_FRONTMOST_APP_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "frontmostAppTrackerCache");
        assert_eq!(value["scope"], "lastNonScriptKitApp");
        assert!(value["status"] == "tracked" || value["status"] == "noTrackedApp");
        assert!(value["app"].is_null() || value["app"].is_object());
        assert!(value["warnings"].is_array());

        for forbidden in [
            "\"click\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_frontmost_app result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_list_app_windows_returns_runtime_snapshot() {
        let runtime = FakeComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_TOOL,
            &serde_json::json!({ "pid": 101 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_app_windows json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_APP_WINDOWS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "coreGraphicsWindowList");
        assert_eq!(value["scope"], "runningAppPid");
        assert_eq!(value["status"], "found");
        assert_eq!(value["app"]["pid"], 101);
        assert_eq!(value["windows"][0]["nativeWindowId"], 98765);
        assert_eq!(value["windows"][0]["title"], "Terminal");
        assert_eq!(value["windows"][0]["bounds"]["width"], 300);
        assert_eq!(value["windows"][0]["isOnScreen"], true);
        assert_eq!(value["windows"][0]["layer"], 0);
        assert_eq!(value["windows"][0]["zOrder"], 0);
        assert!(value["warnings"].is_array());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/list_app_windows result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_returns_grouped_windows() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_app_windows_by_bundle_id json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_APP_WINDOWS_SCHEMA_VERSION)
        );
        assert_eq!(
            value["source"],
            "nsWorkspaceRunningApplications+coreGraphicsWindowList"
        );
        assert_eq!(value["scope"], "runningAppBundleId");
        assert_eq!(value["status"], "listed");
        assert_eq!(value["bundleId"], "com.apple.Terminal");
        assert_eq!(value["appCount"], 2);
        assert_eq!(value["windowCount"], 3);
        assert_eq!(value["apps"][0]["app"]["pid"], 101);
        assert_eq!(value["apps"][0]["status"], "listed");
        assert_eq!(value["apps"][0]["windows"][0]["nativeWindowId"], 98765);
        assert_eq!(
            value["apps"][0]["warnings"],
            serde_json::json!(["ignored offscreen windows"])
        );
        assert_eq!(value["apps"][1]["app"]["pid"], 303);
        assert_eq!(value["apps"][1]["status"], "listed");
        assert_eq!(value["apps"][1]["windows"][0]["nativeWindowId"], 98767);
        assert!(value["warnings"].as_array().unwrap().is_empty());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"screenshot\"",
            "\"capture\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/list_app_windows_by_bundle_id result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_returns_not_found() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Missing" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_app_windows_by_bundle_id json");
        assert_eq!(value["status"], "notFound");
        assert_eq!(value["bundleId"], "com.apple.Missing");
        assert_eq!(value["appCount"], 0);
        assert_eq!(value["windowCount"], 0);
        assert!(value["apps"].as_array().unwrap().is_empty());
        assert!(value["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_handles_per_app_window_error_as_partial() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: Some(303),
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_app_windows_by_bundle_id json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["appCount"], 2);
        assert_eq!(value["windowCount"], 2);
        assert_eq!(value["apps"][1]["app"]["pid"], 303);
        assert_eq!(value["apps"][1]["status"], "windowListFailed");
        assert!(value["apps"][1]["windows"].as_array().unwrap().is_empty());
        assert!(value["warnings"][0]
            .as_str()
            .unwrap()
            .contains("failed to list windows for pid 303"));
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_marks_disappearing_app_as_partial() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: Some(303),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_app_windows_by_bundle_id json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["appCount"], 2);
        assert_eq!(value["windowCount"], 2);
        assert_eq!(value["apps"][1]["app"]["pid"], 303);
        assert_eq!(value["apps"][1]["status"], "appNotFound");
        assert!(value["apps"][1]["windows"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_rejects_stale_pid_bundle_mismatch() {
        struct StaleBundleRuntime;

        impl ComputerUseRuntimeBridge for StaleBundleRuntime {
            fn inspect_automation_window(
                &self,
                _request: ComputerUseInspectRequest,
            ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
                panic!("computer/list_app_windows_by_bundle_id must not inspect automation windows")
            }

            fn list_running_apps(
                &self,
                request: ComputerUseListAppsRequest,
            ) -> Result<
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
                ComputerUseRuntimeError,
            > {
                assert!(request.include_hidden);
                assert!(request.include_background);

                Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                        apps: vec![ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal".to_string(),
                            is_active: true,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        }],
                        frontmost_pid: Some(101),
                    },
                )
            }

            fn list_app_windows(
                &self,
                request: ComputerUseListAppWindowsRequest,
            ) -> Result<
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
                ComputerUseRuntimeError,
            > {
                assert_eq!(request.pid, 101);

                Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: Some(ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.TextEdit".to_string()),
                            name: "TextEdit".to_string(),
                            is_active: false,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        }),
                        windows: vec![test_native_window(98765, 0, "TextEdit")],
                        warnings: Vec::new(),
                    },
                )
            }
        }

        let runtime = StaleBundleRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_app_windows_by_bundle_id json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["appCount"], 1);
        assert_eq!(value["windowCount"], 0);
        assert_eq!(value["apps"][0]["status"], "bundleIdChanged");
        assert!(value["apps"][0]["windows"].as_array().unwrap().is_empty());
        assert!(value["warnings"][0]
            .as_str()
            .unwrap()
            .contains("bundleIdChanged for pid 101"));
    }

    #[test]
    fn computer_list_app_windows_by_bundle_id_propagates_app_list_failure() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: true,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("inspection_failed"));
        assert!(result.content[0]
            .text
            .contains("failed to list running apps"));
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_returns_window() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98767 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_app_window_by_bundle_id json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_APP_WINDOWS_SCHEMA_VERSION)
        );
        assert_eq!(
            value["source"],
            "nsWorkspaceRunningApplications+coreGraphicsWindowList"
        );
        assert_eq!(value["scope"], "runningAppBundleIdNativeWindowId");
        assert_eq!(value["status"], "found");
        assert_eq!(value["bundleId"], "com.apple.Terminal");
        assert_eq!(value["nativeWindowId"], 98767);
        assert_eq!(value["appCount"], 2);
        assert_eq!(value["app"]["pid"], 303);
        assert_eq!(value["window"]["nativeWindowId"], 98767);
        assert_eq!(
            value["warnings"],
            serde_json::json!(["ignored offscreen windows"])
        );

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"screenshot\"",
            "\"capture\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_app_window_by_bundle_id result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_returns_app_not_found() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Missing", "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_app_window_by_bundle_id json");
        assert_eq!(value["status"], "appNotFound");
        assert_eq!(value["bundleId"], "com.apple.Missing");
        assert_eq!(value["nativeWindowId"], 98765);
        assert_eq!(value["appCount"], 0);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert!(value["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_returns_window_not_found() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 11111 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_app_window_by_bundle_id json");
        assert_eq!(value["status"], "windowNotFound");
        assert_eq!(value["appCount"], 2);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert_eq!(
            value["warnings"],
            serde_json::json!(["ignored offscreen windows"])
        );
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_handles_per_app_window_error_as_partial() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: Some(303),
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98767 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_app_window_by_bundle_id json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["appCount"], 2);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert!(value["warnings"][1]
            .as_str()
            .unwrap()
            .contains("failed to list windows for pid 303"));
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_marks_disappearing_app_as_partial() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: Some(303),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98767 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_app_window_by_bundle_id json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["appCount"], 2);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert!(value["warnings"][1]
            .as_str()
            .unwrap()
            .contains("appNotFound for pid 303"));
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_returns_found_with_prior_partial_warning() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: false,
            fail_pid: Some(101),
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98767 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_app_window_by_bundle_id json");
        assert_eq!(value["status"], "found");
        assert_eq!(value["app"]["pid"], 303);
        assert_eq!(value["window"]["nativeWindowId"], 98767);
        assert!(value["warnings"][0]
            .as_str()
            .unwrap()
            .contains("failed to list windows for pid 101"));
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_rejects_stale_pid_bundle_mismatch() {
        struct StaleBundleRuntime;

        impl ComputerUseRuntimeBridge for StaleBundleRuntime {
            fn inspect_automation_window(
                &self,
                _request: ComputerUseInspectRequest,
            ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError> {
                panic!("computer/get_app_window_by_bundle_id must not inspect automation windows")
            }

            fn list_running_apps(
                &self,
                request: ComputerUseListAppsRequest,
            ) -> Result<
                crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot,
                ComputerUseRuntimeError,
            > {
                assert!(request.include_hidden);
                assert!(request.include_background);

                Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppsSnapshot {
                        apps: vec![ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.Terminal".to_string()),
                            name: "Terminal".to_string(),
                            is_active: true,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        }],
                        frontmost_pid: Some(101),
                    },
                )
            }

            fn list_app_windows(
                &self,
                request: ComputerUseListAppWindowsRequest,
            ) -> Result<
                crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot,
                ComputerUseRuntimeError,
            > {
                assert_eq!(request.pid, 101);

                Ok(
                    crate::computer_use::runtime_bridge::ComputerUseListAppWindowsSnapshot {
                        app: Some(ComputerUseRunningAppInfo {
                            pid: 101,
                            bundle_id: Some("com.apple.TextEdit".to_string()),
                            name: "TextEdit".to_string(),
                            is_active: false,
                            is_hidden: false,
                            activation_policy: "regular".to_string(),
                        }),
                        windows: vec![test_native_window(98765, 0, "TextEdit")],
                        warnings: Vec::new(),
                    },
                )
            }
        }

        let runtime = StaleBundleRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_app_window_by_bundle_id json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["appCount"], 1);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert!(value["warnings"][0]
            .as_str()
            .unwrap()
            .contains("bundleIdChanged for pid 101"));
    }

    #[test]
    fn computer_get_app_window_by_bundle_id_propagates_app_list_failure() {
        let runtime = BundleIdAppWindowsRuntime {
            fail_apps: true,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL,
            &serde_json::json!({ "bundleId": "com.apple.Terminal", "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("inspection_failed"));
        assert!(result.content[0]
            .text
            .contains("failed to list running apps"));
    }

    #[test]
    fn computer_list_native_windows_with_runtime_returns_grouped_read_only_snapshot() {
        let runtime = GroupedNativeWindowsRuntime {
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_NATIVE_WINDOWS_TOOL,
            &serde_json::json!({ "includeHidden": true }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_native_windows json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_NATIVE_WINDOWS_SCHEMA_VERSION)
        );
        assert_eq!(
            value["source"],
            "nsWorkspaceRunningApplications+coreGraphicsWindowList"
        );
        assert_eq!(value["scope"], "runningGuiApps");
        assert_eq!(value["status"], "listed");
        assert_eq!(value["frontmostPid"], 101);
        assert_eq!(value["appCount"], 2);
        assert_eq!(value["windowCount"], 2);
        assert_eq!(value["apps"][0]["app"]["pid"], 101);
        assert_eq!(value["apps"][0]["status"], "listed");
        assert_eq!(value["apps"][0]["windows"][0]["nativeWindowId"], 98765);
        assert_eq!(value["apps"][0]["windows"][1]["zOrder"], 1);
        assert_eq!(value["apps"][1]["app"]["pid"], 202);
        assert_eq!(value["apps"][1]["status"], "listed");
        assert!(value["apps"][1]["windows"].as_array().unwrap().is_empty());
        assert!(value["warnings"].as_array().unwrap().is_empty());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"screenshot\"",
            "\"capture\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/list_native_windows result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_list_native_windows_handles_per_app_window_error_as_partial_observation() {
        let runtime = GroupedNativeWindowsRuntime {
            fail_pid: Some(202),
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_NATIVE_WINDOWS_TOOL,
            &serde_json::json!({ "includeHidden": true }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_native_windows json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["windowCount"], 2);
        assert_eq!(value["apps"][0]["status"], "listed");
        assert_eq!(value["apps"][1]["status"], "windowListFailed");
        assert!(value["apps"][1]["windows"].as_array().unwrap().is_empty());
        assert!(value["apps"][1]["warnings"][0]
            .as_str()
            .unwrap()
            .contains("failed to list windows for pid 202"));
        assert!(value["warnings"][0]
            .as_str()
            .unwrap()
            .contains("windowListFailed for pid 202"));
    }

    #[test]
    fn computer_list_native_windows_marks_disappearing_app_as_partial_app_not_found() {
        let runtime = GroupedNativeWindowsRuntime {
            fail_pid: None,
            missing_pid: Some(202),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_NATIVE_WINDOWS_TOOL,
            &serde_json::json!({ "includeHidden": true }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_native_windows json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["windowCount"], 2);
        assert_eq!(value["apps"][0]["status"], "listed");
        assert_eq!(value["apps"][1]["app"]["pid"], 202);
        assert_eq!(value["apps"][1]["status"], "appNotFound");
        assert!(value["apps"][1]["windows"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_get_native_window_returns_window_by_native_window_id() {
        let runtime = NativeWindowLookupRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_NATIVE_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_native_window json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_NATIVE_WINDOWS_SCHEMA_VERSION)
        );
        assert_eq!(
            value["source"],
            "nsWorkspaceRunningApplications+coreGraphicsWindowList"
        );
        assert_eq!(value["scope"], "nativeWindowId");
        assert_eq!(value["status"], "found");
        assert_eq!(value["nativeWindowId"], 98765);
        assert_eq!(value["app"]["pid"], 101);
        assert_eq!(value["window"]["nativeWindowId"], 98765);
        assert_eq!(value["window"]["title"], "Terminal");
        assert!(value["warnings"].as_array().unwrap().is_empty());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"screenshot\"",
            "\"capture\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_native_window result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_native_window_returns_not_found_for_unknown_native_window_id() {
        let runtime = NativeWindowLookupRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_NATIVE_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 11111 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_native_window json");
        assert_eq!(
            value["source"],
            "nsWorkspaceRunningApplications+coreGraphicsWindowList"
        );
        assert_eq!(value["scope"], "nativeWindowId");
        assert_eq!(value["status"], "notFound");
        assert_eq!(value["nativeWindowId"], 11111);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert!(value["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_get_native_window_returns_partial_when_lookup_has_per_app_failures() {
        let runtime = NativeWindowLookupRuntime {
            fail_apps: false,
            fail_pid: Some(202),
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_NATIVE_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 11111 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_native_window json");
        assert_eq!(value["status"], "partial");
        assert_eq!(value["nativeWindowId"], 11111);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert!(value["warnings"][0]
            .as_str()
            .unwrap()
            .contains("windowListFailed for pid 202"));
    }

    #[test]
    fn computer_get_native_window_returns_partial_when_app_disappears_during_lookup() {
        let runtime = NativeWindowLookupRuntime {
            fail_apps: false,
            fail_pid: None,
            missing_pid: Some(202),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_NATIVE_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 11111 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_native_window json");
        assert_eq!(value["status"], "partial");
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert!(value["warnings"][0]
            .as_str()
            .unwrap()
            .contains("appNotFound for pid 202"));
    }

    #[test]
    fn computer_get_native_window_propagates_top_level_app_list_failure() {
        let runtime = NativeWindowLookupRuntime {
            fail_apps: true,
            fail_pid: None,
            missing_pid: None,
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_NATIVE_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("inspection_failed"));
        assert!(result.content[0]
            .text
            .contains("failed to list running apps"));
    }

    #[test]
    fn choose_frontmost_native_window_prefers_lowest_z_order_then_window_id() {
        let window = choose_frontmost_native_window(vec![
            test_native_window(300, 0, "Later id"),
            test_native_window(200, 0, "Earlier id"),
            test_native_window(100, 1, "Behind"),
        ])
        .expect("frontmost native window");

        assert_eq!(window.native_window_id, 200);
    }

    #[test]
    fn computer_get_frontmost_native_window_returns_lowest_z_order_window() {
        let runtime = FrontmostNativeWindowRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            windows: vec![
                test_native_window(98766, 1, "Terminal Settings"),
                test_native_window(98765, 0, "Terminal"),
            ],
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_native_window json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_FRONTMOST_NATIVE_WINDOW_SCHEMA_VERSION)
        );
        assert_eq!(
            value["source"],
            "nsWorkspaceRunningApplications+coreGraphicsWindowList"
        );
        assert_eq!(value["scope"], "frontmostNativeWindow");
        assert_eq!(value["status"], "found");
        assert_eq!(value["frontmostPid"], 101);
        assert_eq!(value["app"]["pid"], 101);
        assert_eq!(value["window"]["nativeWindowId"], 98765);
        assert_eq!(value["window"]["title"], "Terminal");
        assert_eq!(value["windowCount"], 2);
        assert!(value["warnings"].as_array().unwrap().is_empty());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"screenshot\"",
            "\"capture\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_frontmost_native_window result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_frontmost_native_window_returns_no_frontmost_app() {
        let runtime = FrontmostNativeWindowRuntime {
            frontmost_pid: None,
            missing_app_window_pid: None,
            windows: vec![test_native_window(98765, 0, "Terminal")],
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_native_window json");
        assert_eq!(value["status"], "noFrontmostApp");
        assert!(value["frontmostPid"].is_null());
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert_eq!(value["windowCount"], 0);
    }

    #[test]
    fn computer_get_frontmost_native_window_returns_app_not_found() {
        let runtime = FrontmostNativeWindowRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: Some(101),
            windows: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_native_window json");
        assert_eq!(value["status"], "appNotFound");
        assert_eq!(value["frontmostPid"], 101);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert_eq!(value["windowCount"], 0);
    }

    #[test]
    fn computer_get_frontmost_native_window_returns_no_windows() {
        let runtime = FrontmostNativeWindowRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            windows: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_native_window json");
        assert_eq!(value["status"], "noWindows");
        assert_eq!(value["frontmostPid"], 101);
        assert_eq!(value["app"]["pid"], 101);
        assert!(value["window"].is_null());
        assert_eq!(value["windowCount"], 0);
    }

    #[test]
    fn computer_list_frontmost_app_windows_returns_all_frontmost_app_windows() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: false,
            windows: vec![
                test_native_window(98766, 1, "Terminal Settings"),
                test_native_window(98765, 0, "Terminal"),
            ],
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_frontmost_app_windows json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_FRONTMOST_APP_WINDOWS_SCHEMA_VERSION)
        );
        assert_eq!(
            value["source"],
            "nsWorkspaceRunningApplications+coreGraphicsWindowList"
        );
        assert_eq!(value["scope"], "frontmostAppWindows");
        assert_eq!(value["status"], "listed");
        assert_eq!(value["frontmostPid"], 101);
        assert_eq!(value["app"]["pid"], 101);
        assert_eq!(value["windowCount"], 2);
        assert_eq!(value["windows"][0]["nativeWindowId"], 98766);
        assert_eq!(value["windows"][1]["nativeWindowId"], 98765);
        assert!(value["warnings"].as_array().unwrap().is_empty());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"screenshot\"",
            "\"capture\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/list_frontmost_app_windows result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_list_frontmost_app_windows_preserves_window_warnings() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: false,
            windows: vec![test_native_window(98765, 0, "Terminal")],
            warnings: vec!["ignored offscreen windows".to_string()],
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_frontmost_app_windows json");
        assert_eq!(value["status"], "listed");
        assert_eq!(
            value["warnings"],
            serde_json::json!(["ignored offscreen windows"])
        );
    }

    #[test]
    fn computer_list_frontmost_app_windows_returns_no_frontmost_app() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: None,
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: false,
            windows: vec![test_native_window(98765, 0, "Terminal")],
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_frontmost_app_windows json");
        assert_eq!(value["status"], "noFrontmostApp");
        assert!(value["frontmostPid"].is_null());
        assert!(value["app"].is_null());
        assert_eq!(value["windowCount"], 0);
        assert!(value["windows"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_list_frontmost_app_windows_returns_app_not_found() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: Some(101),
            fail_apps: false,
            fail_windows: false,
            windows: Vec::new(),
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_frontmost_app_windows json");
        assert_eq!(value["status"], "appNotFound");
        assert_eq!(value["frontmostPid"], 101);
        assert!(value["app"].is_null());
        assert_eq!(value["windowCount"], 0);
        assert!(value["windows"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_list_frontmost_app_windows_returns_no_windows() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: false,
            windows: Vec::new(),
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid list_frontmost_app_windows json");
        assert_eq!(value["status"], "noWindows");
        assert_eq!(value["frontmostPid"], 101);
        assert_eq!(value["app"]["pid"], 101);
        assert_eq!(value["windowCount"], 0);
        assert!(value["windows"].as_array().unwrap().is_empty());
    }

    #[test]
    fn computer_list_frontmost_app_windows_propagates_app_list_failure() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: true,
            fail_windows: false,
            windows: Vec::new(),
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("inspection_failed"));
        assert!(result.content[0]
            .text
            .contains("failed to list running apps"));
    }

    #[test]
    fn computer_list_frontmost_app_windows_propagates_window_list_failure() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: true,
            windows: Vec::new(),
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("inspection_failed"));
        assert!(result.content[0]
            .text
            .contains("failed to list windows for pid 101"));
    }

    #[test]
    fn computer_get_frontmost_app_window_returns_window_by_native_window_id() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: false,
            windows: vec![
                test_native_window(98766, 1, "Terminal Settings"),
                test_native_window(98765, 0, "Terminal"),
            ],
            warnings: vec!["ignored offscreen windows".to_string()],
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_app_window json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_FRONTMOST_APP_WINDOW_SCHEMA_VERSION)
        );
        assert_eq!(
            value["source"],
            "nsWorkspaceRunningApplications+coreGraphicsWindowList"
        );
        assert_eq!(value["scope"], "frontmostAppNativeWindowId");
        assert_eq!(value["status"], "found");
        assert_eq!(value["nativeWindowId"], 98765);
        assert_eq!(value["frontmostPid"], 101);
        assert_eq!(value["app"]["pid"], 101);
        assert_eq!(value["window"]["nativeWindowId"], 98765);
        assert_eq!(value["windowCount"], 2);
        assert_eq!(
            value["warnings"],
            serde_json::json!(["ignored offscreen windows"])
        );

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"focus\"",
            "\"activate\"",
            "\"launch\"",
            "\"quit\"",
            "\"hide\"",
            "\"move\"",
            "\"resize\"",
            "\"setBounds\"",
            "\"screenshot\"",
            "\"capture\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_frontmost_app_window result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_frontmost_app_window_returns_window_not_found() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: false,
            windows: vec![test_native_window(98765, 0, "Terminal")],
            warnings: vec!["ignored offscreen windows".to_string()],
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98766 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_app_window json");
        assert_eq!(value["status"], "windowNotFound");
        assert_eq!(value["nativeWindowId"], 98766);
        assert_eq!(value["frontmostPid"], 101);
        assert_eq!(value["app"]["pid"], 101);
        assert!(value["window"].is_null());
        assert_eq!(value["windowCount"], 1);
        assert_eq!(
            value["warnings"],
            serde_json::json!(["ignored offscreen windows"])
        );
    }

    #[test]
    fn computer_get_frontmost_app_window_returns_no_frontmost_app() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: None,
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: false,
            windows: vec![test_native_window(98765, 0, "Terminal")],
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_app_window json");
        assert_eq!(value["status"], "noFrontmostApp");
        assert_eq!(value["nativeWindowId"], 98765);
        assert!(value["frontmostPid"].is_null());
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert_eq!(value["windowCount"], 0);
    }

    #[test]
    fn computer_get_frontmost_app_window_returns_app_not_found() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: Some(101),
            fail_apps: false,
            fail_windows: false,
            windows: Vec::new(),
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_app_window json");
        assert_eq!(value["status"], "appNotFound");
        assert_eq!(value["nativeWindowId"], 98765);
        assert_eq!(value["frontmostPid"], 101);
        assert!(value["app"].is_null());
        assert!(value["window"].is_null());
        assert_eq!(value["windowCount"], 0);
    }

    #[test]
    fn computer_get_frontmost_app_window_returns_no_windows() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: false,
            windows: Vec::new(),
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_frontmost_app_window json");
        assert_eq!(value["status"], "noWindows");
        assert_eq!(value["nativeWindowId"], 98765);
        assert_eq!(value["frontmostPid"], 101);
        assert_eq!(value["app"]["pid"], 101);
        assert!(value["window"].is_null());
        assert_eq!(value["windowCount"], 0);
    }

    #[test]
    fn computer_get_frontmost_app_window_propagates_app_list_failure() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: true,
            fail_windows: false,
            windows: Vec::new(),
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("inspection_failed"));
        assert!(result.content[0]
            .text
            .contains("failed to list running apps"));
    }

    #[test]
    fn computer_get_frontmost_app_window_propagates_window_list_failure() {
        let runtime = ListFrontmostAppWindowsRuntime {
            frontmost_pid: Some(101),
            missing_app_window_pid: None,
            fail_apps: false,
            fail_windows: true,
            windows: Vec::new(),
            warnings: Vec::new(),
        };
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL,
            &serde_json::json!({ "nativeWindowId": 98765 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("inspection_failed"));
        assert!(result.content[0]
            .text
            .contains("failed to list windows for pid 101"));
    }

    #[test]
    fn computer_list_app_windows_requires_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_APP_WINDOWS_TOOL,
            &serde_json::json!({ "pid": 101 }),
            None,
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("runtime_unavailable"));
    }

    #[test]
    fn computer_list_menus_returns_cached_snapshot_without_runtime() {
        let result =
            handle_computer_use_tool_call(COMPUTER_LIST_MENUS_TOOL, &serde_json::json!({}), None);

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_menus json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_MENUS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "frontmostAppTrackerCache");
        assert!(value["cache"]["status"].is_string());
        assert!(value["cache"]["isFetching"].is_boolean());
        assert!(value["menus"].is_array());
        assert!(value["warnings"].is_array());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/list_menus result must not expose menu action handles; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_list_menu_item_paths_returns_cached_snapshot_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_MENU_ITEM_PATHS_TOOL,
            &serde_json::json!({}),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_menu_item_paths json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_MENUS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "frontmostAppTrackerCache");
        assert_eq!(value["scope"], "cachedMenuItemPaths");
        assert!(
            value["status"] == "listed"
                || value["status"] == "noTrackedApp"
                || value["status"] == "noCachedMenus"
        );
        assert!(value["cache"]["status"].is_string());
        assert!(value["cache"]["isFetching"].is_boolean());
        assert!(value["items"].is_array());
        assert!(value["warnings"].is_array());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/list_menu_item_paths result must not expose menu action handles; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_list_menu_item_paths_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_MENU_ITEM_PATHS_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_menu_item_paths json");
        assert_eq!(value["source"], "frontmostAppTrackerCache");
        assert_eq!(value["scope"], "cachedMenuItemPaths");
    }

    #[test]
    fn computer_get_menu_item_returns_cache_snapshot_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_MENU_ITEM_TOOL,
            &serde_json::json!({ "path": ["__missing_menu_for_contract_test__"] }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_menu_item json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_MENUS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "frontmostAppTrackerCache");
        assert_eq!(value["scope"], "cachedMenuPath");
        assert!(
            value["status"] == "notFound"
                || value["status"] == "noTrackedApp"
                || value["status"] == "noCachedMenus"
        );
        assert_eq!(
            value["path"],
            serde_json::json!(["__missing_menu_for_contract_test__"])
        );
        assert!(value["cache"]["status"].is_string());
        assert!(value["cache"]["isFetching"].is_boolean());
        assert!(value["warnings"].is_array());
        assert!(value["item"].is_null());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_menu_item result must not expose menu action handles; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_menu_item_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_MENU_ITEM_TOOL,
            &serde_json::json!({ "path": ["__missing_menu_for_runtime_test__"] }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_menu_item json");
        assert_eq!(value["source"], "frontmostAppTrackerCache");
        assert_eq!(value["scope"], "cachedMenuPath");
    }

    #[test]
    fn computer_get_menu_item_by_index_path_returns_cache_snapshot_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_MENU_ITEM_BY_INDEX_PATH_TOOL,
            &serde_json::json!({ "indexPath": [9999] }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_menu_item_by_index_path json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_MENUS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "frontmostAppTrackerCache");
        assert_eq!(value["scope"], "cachedMenuIndexPath");
        assert!(
            value["status"] == "notFound"
                || value["status"] == "noTrackedApp"
                || value["status"] == "noCachedMenus"
        );
        assert_eq!(value["indexPath"], serde_json::json!([9999]));
        assert!(value["resolvedPath"].is_null());
        assert!(value["cache"]["status"].is_string());
        assert!(value["cache"]["isFetching"].is_boolean());
        assert!(value["warnings"].is_array());
        assert!(value["item"].is_null());

        for forbidden in [
            "\"action\"",
            "\"click\"",
            "\"press\"",
            "\"execute\"",
            "\"axElementPath\"",
            "\"AXPress\"",
        ] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_menu_item_by_index_path result must not expose menu action handles; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_menu_item_by_index_path_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_MENU_ITEM_BY_INDEX_PATH_TOOL,
            &serde_json::json!({ "indexPath": [9999] }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_menu_item_by_index_path json");
        assert_eq!(value["source"], "frontmostAppTrackerCache");
        assert_eq!(value["scope"], "cachedMenuIndexPath");
    }

    #[test]
    fn find_cached_menu_item_by_path_finds_top_level_item() {
        let items = vec![
            test_menu_item("File", vec![]),
            test_menu_item("Edit", vec![]),
        ];

        let found = find_cached_menu_item_by_path(&items, &[String::from("File")])
            .expect("top-level File item");

        assert_eq!(found.title, "File");
    }

    #[test]
    fn find_cached_menu_item_by_path_finds_nested_item() {
        let items = vec![test_menu_item(
            "File",
            vec![test_menu_item(
                "New",
                vec![test_menu_item("Project", vec![])],
            )],
        )];

        let found = find_cached_menu_item_by_path(
            &items,
            &[
                String::from("File"),
                String::from("New"),
                String::from("Project"),
            ],
        )
        .expect("nested Project item");

        assert_eq!(found.title, "Project");
    }

    #[test]
    fn find_cached_menu_item_by_path_returns_none_for_missing_segment() {
        let items = vec![test_menu_item("File", vec![test_menu_item("Open", vec![])])];

        let found =
            find_cached_menu_item_by_path(&items, &[String::from("File"), String::from("New")]);

        assert!(found.is_none());
    }

    #[test]
    fn find_cached_menu_item_by_path_returns_none_for_empty_path() {
        let items = vec![test_menu_item("File", vec![])];

        let found = find_cached_menu_item_by_path(&items, &[]);

        assert!(found.is_none());
    }

    #[test]
    fn find_cached_menu_item_by_index_path_finds_top_level_item() {
        let items = vec![
            test_menu_item("File", vec![]),
            test_menu_item("Edit", vec![]),
        ];

        let (found, path) =
            find_cached_menu_item_by_index_path(&items, &[1]).expect("top-level Edit item");

        assert_eq!(found.title, "Edit");
        assert_eq!(path, vec!["Edit"]);
    }

    #[test]
    fn find_cached_menu_item_by_index_path_finds_nested_item() {
        let items = vec![test_menu_item(
            "File",
            vec![test_menu_item(
                "New",
                vec![test_menu_item("Project", vec![])],
            )],
        )];

        let (found, path) =
            find_cached_menu_item_by_index_path(&items, &[0, 0, 0]).expect("nested Project item");

        assert_eq!(found.title, "Project");
        assert_eq!(path, vec!["File", "New", "Project"]);
    }

    #[test]
    fn find_cached_menu_item_by_index_path_returns_none_for_missing_index() {
        let items = vec![test_menu_item("File", vec![test_menu_item("Open", vec![])])];

        let found = find_cached_menu_item_by_index_path(&items, &[0, 1]);

        assert!(found.is_none());
    }

    #[test]
    fn find_cached_menu_item_by_index_path_returns_none_for_empty_path() {
        let items = vec![test_menu_item("File", vec![])];

        let found = find_cached_menu_item_by_index_path(&items, &[]);

        assert!(found.is_none());
    }

    #[test]
    fn flatten_cached_menu_item_paths_preserves_preorder_and_index_paths() {
        let items = vec![
            test_menu_item(
                "File",
                vec![
                    test_menu_item("New", vec![test_menu_item("Project", vec![])]),
                    test_menu_item("Open", vec![]),
                ],
            ),
            test_menu_item("Edit", vec![]),
        ];
        let mut flattened = Vec::new();

        flatten_cached_menu_item_paths(&items, &mut Vec::new(), &mut Vec::new(), &mut flattened);

        assert_eq!(flattened.len(), 5);
        assert_eq!(flattened[0].title, "File");
        assert_eq!(flattened[0].path, vec!["File"]);
        assert_eq!(flattened[0].index_path, vec![0]);
        assert_eq!(flattened[0].child_count, 2);
        assert_eq!(flattened[1].title, "New");
        assert_eq!(flattened[1].path, vec!["File", "New"]);
        assert_eq!(flattened[1].index_path, vec![0, 0]);
        assert_eq!(flattened[2].title, "Project");
        assert_eq!(flattened[2].path, vec!["File", "New", "Project"]);
        assert_eq!(flattened[2].index_path, vec![0, 0, 0]);
        assert_eq!(flattened[3].title, "Open");
        assert_eq!(flattened[3].path, vec!["File", "Open"]);
        assert_eq!(flattened[3].index_path, vec![0, 1]);
        assert_eq!(flattened[4].title, "Edit");
        assert_eq!(flattened[4].path, vec!["Edit"]);
        assert_eq!(flattened[4].index_path, vec![1]);
    }

    #[test]
    fn flatten_cached_menu_item_paths_round_trips_through_index_lookup() {
        let items = vec![
            test_menu_item(
                "File",
                vec![
                    test_menu_item("New", vec![test_menu_item("Project", vec![])]),
                    test_menu_item("Open", vec![]),
                ],
            ),
            test_menu_item("Edit", vec![test_menu_item("Undo", vec![])]),
        ];
        let mut flattened = Vec::new();

        flatten_cached_menu_item_paths(&items, &mut Vec::new(), &mut Vec::new(), &mut flattened);

        for flattened_item in flattened {
            let (found, resolved_path) =
                find_cached_menu_item_by_index_path(&items, &flattened_item.index_path)
                    .expect("flattened index path resolves");

            assert_eq!(found.title, flattened_item.title);
            assert_eq!(resolved_path, flattened_item.path);
        }
    }

    fn test_menu_item(title: &str, children: Vec<MenuBarItem>) -> MenuBarItem {
        MenuBarItem {
            title: title.to_string(),
            enabled: true,
            shortcut: None,
            children,
            ax_element_path: Vec::new(),
        }
    }

    fn test_native_window(
        native_window_id: u32,
        z_order: u32,
        title: &str,
    ) -> ComputerUseAppWindowInfo {
        ComputerUseAppWindowInfo {
            native_window_id,
            title: Some(title.to_string()),
            bounds: TargetWindowBounds {
                x: 10,
                y: 20,
                width: 300,
                height: 200,
            },
            is_on_screen: true,
            layer: 0,
            z_order,
            observation: None,
        }
    }

    #[test]
    fn computer_list_tray_menu_with_runtime_returns_snapshot() {
        let runtime = FakeComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_TRAY_MENU_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_tray_menu json");
        assert_eq!(value["schemaVersion"], serde_json::json!(1));
        assert_eq!(value["source"], "scriptKitTrayMenuModel");
        assert_eq!(value["owner"]["scope"], "ownTrayMenuOnly");
        assert!(value["sections"].is_array());
        assert!(!result.content[0].text.contains("\"click\""));
        assert!(!result.content[0].text.contains("\"execute\""));
    }

    #[test]
    fn computer_list_tray_menu_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_TRAY_MENU_TOOL,
            &serde_json::json!({}),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
    }

    #[test]
    fn computer_get_tray_menu_item_returns_item_by_section_and_item_index() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_TRAY_MENU_ITEM_TOOL,
            &serde_json::json!({ "sectionIndex": 0, "itemIndex": 0 }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_tray_menu_item json");
        assert_eq!(value["schemaVersion"], serde_json::json!(1));
        assert_eq!(value["source"], "scriptKitTrayMenuModel");
        assert_eq!(value["scope"], "ownTrayMenuSectionItemIndex");
        assert_eq!(value["status"], "found");
        assert_eq!(value["owner"]["scope"], "ownTrayMenuOnly");
        assert_eq!(value["sectionIndex"], 0);
        assert_eq!(value["itemIndex"], 0);
        assert_eq!(value["section"]["id"], "open");
        assert_eq!(value["section"]["label"], "Open");
        assert!(value["section"]["itemCount"]
            .as_u64()
            .is_some_and(|count| count > 0));
        assert_eq!(value["item"]["id"], "tray.open_script_kit");
        assert_eq!(value["item"]["title"], "Open Script Kit");
        assert!(value["warnings"].is_array());

        for forbidden in ["\"click\"", "\"press\"", "\"execute\"", "\"action\""] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_tray_menu_item result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_tray_menu_item_returns_section_not_found() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_TRAY_MENU_ITEM_TOOL,
            &serde_json::json!({ "sectionIndex": 9999, "itemIndex": 0 }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_tray_menu_item json");
        assert_eq!(value["status"], "sectionNotFound");
        assert!(value["section"].is_null());
        assert!(value["item"].is_null());
        assert!(value["warnings"].is_array());
    }

    #[test]
    fn computer_get_tray_menu_item_returns_item_not_found() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_TRAY_MENU_ITEM_TOOL,
            &serde_json::json!({ "sectionIndex": 0, "itemIndex": 9999 }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_tray_menu_item json");
        assert_eq!(value["status"], "itemNotFound");
        assert_eq!(value["section"]["id"], "open");
        assert!(value["item"].is_null());
        assert!(value["warnings"].is_array());
    }

    #[test]
    fn computer_get_tray_menu_item_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_TRAY_MENU_ITEM_TOOL,
            &serde_json::json!({ "sectionIndex": 0, "itemIndex": 0 }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_tray_menu_item json");
        assert_eq!(value["source"], "scriptKitTrayMenuModel");
        assert_eq!(value["scope"], "ownTrayMenuSectionItemIndex");
    }

    #[test]
    fn computer_get_tray_menu_item_by_id_returns_item_by_id() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL,
            &serde_json::json!({ "id": "tray.open_script_kit" }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_tray_menu_item_by_id json");
        assert_eq!(value["schemaVersion"], serde_json::json!(1));
        assert_eq!(value["source"], "scriptKitTrayMenuModel");
        assert_eq!(value["scope"], "ownTrayMenuItemId");
        assert_eq!(value["status"], "found");
        assert_eq!(value["owner"]["scope"], "ownTrayMenuOnly");
        assert_eq!(value["id"], "tray.open_script_kit");
        assert_eq!(value["sectionIndex"], 0);
        assert_eq!(value["itemIndex"], 0);
        assert_eq!(value["section"]["id"], "open");
        assert_eq!(value["section"]["label"], "Open");
        assert!(value["section"]["itemCount"]
            .as_u64()
            .is_some_and(|count| count > 0));
        assert_eq!(value["item"]["id"], "tray.open_script_kit");
        assert_eq!(value["item"]["title"], "Open Script Kit");
        assert!(value["warnings"].is_array());

        for forbidden in ["\"click\"", "\"press\"", "\"execute\"", "\"action\""] {
            assert!(
                !result.content[0].text.contains(forbidden),
                "computer/get_tray_menu_item_by_id result must not expose executable fields; found {forbidden}"
            );
        }
    }

    #[test]
    fn computer_get_tray_menu_item_by_id_returns_not_found() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL,
            &serde_json::json!({ "id": "__missing_tray_item_id__" }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_tray_menu_item_by_id json");
        assert_eq!(value["source"], "scriptKitTrayMenuModel");
        assert_eq!(value["scope"], "ownTrayMenuItemId");
        assert_eq!(value["status"], "notFound");
        assert_eq!(value["id"], "__missing_tray_item_id__");
        assert!(value["sectionIndex"].is_null());
        assert!(value["itemIndex"].is_null());
        assert!(value["section"].is_null());
        assert!(value["item"].is_null());
        assert!(value["warnings"].is_array());
    }

    #[test]
    fn computer_get_tray_menu_item_by_id_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL,
            &serde_json::json!({ "id": "tray.open_script_kit" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value = serde_json::from_str(&result.content[0].text)
            .expect("valid get_tray_menu_item_by_id json");
        assert_eq!(value["source"], "scriptKitTrayMenuModel");
        assert_eq!(value["scope"], "ownTrayMenuItemId");
    }

    #[test]
    fn computer_list_permissions_returns_status_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_LIST_PERMISSIONS_TOOL,
            &serde_json::json!({}),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid permissions json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_PERMISSIONS_SCHEMA_VERSION)
        );

        let permissions = value["permissions"].as_array().expect("permissions array");
        let accessibility = permissions
            .iter()
            .find(|permission| permission["id"] == "accessibility")
            .expect("accessibility status");
        assert_eq!(accessibility["name"], "Accessibility");
        assert!(accessibility["granted"].is_boolean());
        assert!(accessibility["status"] == "granted" || accessibility["status"] == "notGranted");

        let screen_recording = permissions
            .iter()
            .find(|permission| permission["id"] == "screenRecording")
            .expect("screen recording status");
        assert_eq!(screen_recording["name"], "Screen Recording");
        assert!(
            screen_recording["status"] == "granted"
                || screen_recording["status"] == "notGranted"
                || screen_recording["status"] == "unknown"
        );

        let event_synthesizing = permissions
            .iter()
            .find(|permission| permission["id"] == "eventSynthesizing")
            .expect("event synthesizing status");
        assert_eq!(event_synthesizing["name"], "Event Synthesizing");
        assert!(
            event_synthesizing["status"] == "granted"
                || event_synthesizing["status"] == "notGranted"
                || event_synthesizing["status"] == "unknown"
        );
        assert!(!result.content[0].text.contains("requestAccessibility"));
        assert!(!result.content[0].text.contains("requestEventSynthesizing"));
        assert!(!result.content[0].text.contains("grantInstructions"));
        assert!(!result.content[0].text.contains("openSettings"));
        assert!(!result.content[0].text.contains("\"execute\""));
        assert!(!result.content[0].text.contains("\"click\""));
        assert!(!result.content[0].text.contains("\"press\""));
    }

    #[test]
    fn computer_get_permission_returns_permission_by_id_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_PERMISSION_TOOL,
            &serde_json::json!({ "id": "screenRecording" }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_permission json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_PERMISSIONS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "macosPermissionPreflight");
        assert_eq!(value["scope"], "permissionId");
        assert_eq!(value["status"], "found");
        assert_eq!(value["permission"]["id"], "screenRecording");
        assert_eq!(value["permission"]["name"], "Screen Recording");
        assert!(
            value["permission"]["status"] == "granted"
                || value["permission"]["status"] == "notGranted"
                || value["permission"]["status"] == "unknown"
        );
        assert!(value["warnings"]
            .as_array()
            .expect("warnings array")
            .is_empty());
        assert!(!result.content[0].text.contains("requestAccessibility"));
        assert!(!result.content[0].text.contains("requestEventSynthesizing"));
        assert!(!result.content[0].text.contains("grantInstructions"));
        assert!(!result.content[0].text.contains("openSettings"));
        assert!(!result.content[0].text.contains("\"execute\""));
        assert!(!result.content[0].text.contains("\"click\""));
        assert!(!result.content[0].text.contains("\"press\""));
    }

    #[test]
    fn computer_get_permission_returns_not_found_for_unknown_id_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_PERMISSION_TOOL,
            &serde_json::json!({ "id": "unknownPermission" }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_permission json");
        assert_eq!(value["source"], "macosPermissionPreflight");
        assert_eq!(value["scope"], "permissionId");
        assert_eq!(value["status"], "notFound");
        assert!(value["permission"].is_null());
        assert!(value["warnings"]
            .as_array()
            .expect("warnings array")
            .is_empty());
    }

    #[test]
    fn computer_get_permission_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_PERMISSION_TOOL,
            &serde_json::json!({ "id": "unknownPermission" }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_permission json");
        assert_eq!(value["status"], "notFound");
    }

    #[test]
    fn computer_list_screens_returns_screen_snapshot_without_runtime() {
        let result =
            handle_computer_use_tool_call(COMPUTER_LIST_SCREENS_TOOL, &serde_json::json!({}), None);

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid list_screens json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_SCREENS_SCHEMA_VERSION)
        );

        let screens = value["screens"].as_array().expect("screens array");
        for screen in screens {
            assert!(screen["displayId"].is_number());
            assert!(screen["name"].is_string());
            assert!(screen["isPrimary"].is_boolean());
            assert!(screen["bounds"]["width"].is_number());
            assert!(screen["bounds"]["height"].is_number());
            assert!(screen["visibleBounds"]["width"].is_number());
            assert!(screen["visibleBounds"]["height"].is_number());
        }
    }

    #[test]
    fn computer_get_screen_returns_screen_by_display_id_without_runtime() {
        let list_result =
            handle_computer_use_tool_call(COMPUTER_LIST_SCREENS_TOOL, &serde_json::json!({}), None);
        assert_eq!(list_result.is_error, None);
        let list_value: serde_json::Value =
            serde_json::from_str(&list_result.content[0].text).expect("valid list_screens json");
        let Some(display_id) = list_value["screens"]
            .as_array()
            .and_then(|screens| screens.first())
            .and_then(|screen| screen["displayId"].as_u64())
        else {
            return;
        };

        let result = handle_computer_use_tool_call(
            COMPUTER_GET_SCREEN_TOOL,
            &serde_json::json!({ "displayId": display_id }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_screen json");
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(COMPUTER_SCREENS_SCHEMA_VERSION)
        );
        assert_eq!(value["source"], "coreGraphicsActiveDisplays");
        assert_eq!(value["scope"], "displayId");
        assert_eq!(value["status"], "found");
        assert_eq!(value["screen"]["displayId"], serde_json::json!(display_id));
        assert!(value["warnings"]
            .as_array()
            .expect("warnings array")
            .is_empty());
        assert!(!result.content[0].text.contains("\"move\""));
        assert!(!result.content[0].text.contains("\"resize\""));
        assert!(!result.content[0].text.contains("\"screenshot\""));
        assert!(!result.content[0].text.contains("\"click\""));
        assert!(!result.content[0].text.contains("\"press\""));
        assert!(!result.content[0].text.contains("\"execute\""));
    }

    #[test]
    fn computer_get_screen_returns_not_found_for_unknown_display_id_without_runtime() {
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_SCREEN_TOOL,
            &serde_json::json!({ "displayId": u32::MAX }),
            None,
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_screen json");
        assert_eq!(value["source"], "coreGraphicsActiveDisplays");
        assert_eq!(value["scope"], "displayId");
        assert_eq!(value["status"], "notFound");
        assert!(value["screen"].is_null());
        assert!(value["warnings"]
            .as_array()
            .expect("warnings array")
            .is_empty());
    }

    #[test]
    fn computer_get_screen_ignores_supplied_runtime() {
        let runtime = PanickingComputerUseRuntime;
        let result = handle_computer_use_tool_call(
            COMPUTER_GET_SCREEN_TOOL,
            &serde_json::json!({ "displayId": u32::MAX }),
            Some(&runtime),
        );

        assert_eq!(result.is_error, None);
        let value: serde_json::Value =
            serde_json::from_str(&result.content[0].text).expect("valid get_screen json");
        assert_eq!(value["status"], "notFound");
    }
}
