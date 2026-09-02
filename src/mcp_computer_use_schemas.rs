fn automation_window_instance_target_schema() -> Value {
    serde_json::json!({
        "description": "One exact registered window lifetime. Qualified render readback requires this target and the expected completed-frame identity; discovery selectors do not confer capture authority.",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "type": { "const": "instance" },
            "id": { "type": "string", "minLength": 1 },
            "generation": { "type": "integer", "minimum": 1, "maximum": u64::MAX }
        },
        "required": ["type", "id", "generation"]
    })
}

fn automation_window_target_schema() -> Value {
    use strum::IntoEnumIterator;

    serde_json::json!({
        "description": "AutomationWindowTarget. main/focused/id/kind/titleContains are discovery selectors; instance names one exact lifetime. Omit to use the focused window only where the caller schema allows omission. Qualified render readback requires instance plus expected completed-frame identity.",
        "oneOf": [
            {
                "type": "object",
                "additionalProperties": false,
                "properties": { "type": { "const": "main" } },
                "required": ["type"]
            },
            {
                "type": "object",
                "additionalProperties": false,
                "properties": { "type": { "const": "focused" } },
                "required": ["type"]
            },
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "type": { "const": "id" },
                    "id": { "type": "string" }
                },
                "required": ["type", "id"]
            },
            automation_window_instance_target_schema(),
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "type": { "const": "kind" },
                    "kind": {
                        "type": "string",
                        "enum": crate::protocol::AutomationWindowKind::iter().collect::<Vec<_>>()
                    },
                    "index": { "type": "integer", "minimum": 0 }
                },
                "required": ["type", "kind"]
            },
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "type": { "const": "titleContains" },
                    "text": { "type": "string" }
                },
                "required": ["type", "text"]
            }
        ]
    })
}

fn computer_see_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "target": automation_window_target_schema(),
            "hiDpi": { "type": "boolean", "default": false },
            "probes": {
                "type": "array",
                "default": [],
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "x": { "type": "integer", "minimum": 0 },
                        "y": { "type": "integer", "minimum": 0 }
                    },
                    "required": ["x", "y"]
                }
            }
        }
    })
}

fn computer_list_windows_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_get_window_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string" }
        },
        "required": ["id"]
    })
}

fn computer_get_focused_window_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_list_apps_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "includeHidden": {
                "type": "boolean",
                "default": false,
                "description": "Include hidden running GUI applications."
            },
            "includeBackground": {
                "type": "boolean",
                "default": false,
                "description": "Include accessory, prohibited, and unknown background applications in addition to regular GUI apps."
            }
        }
    })
}

fn computer_get_app_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "pid": { "type": "integer" }
        },
        "required": ["pid"]
    })
}

fn computer_list_apps_by_bundle_id_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "bundleId": {
                "type": "string",
                "minLength": 1,
                "description": "Exact bundle identifier for currently running GUI applications, e.g. com.apple.Terminal."
            }
        },
        "required": ["bundleId"]
    })
}

fn computer_list_app_windows_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "pid": { "type": "integer" }
        },
        "required": ["pid"]
    })
}

fn computer_list_app_windows_by_bundle_id_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "bundleId": {
                "type": "string",
                "minLength": 1,
                "description": "Exact bundle identifier for a currently running GUI application, e.g. com.apple.Terminal."
            }
        },
        "required": ["bundleId"]
    })
}

fn computer_get_app_window_by_bundle_id_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "bundleId": {
                "type": "string",
                "minLength": 1,
                "description": "Exact bundle identifier for a currently running GUI application, e.g. com.apple.Terminal."
            },
            "nativeWindowId": {
                "type": "integer",
                "minimum": 0,
                "maximum": 4_294_967_295u64,
                "description": "CoreGraphics native window id to look up within currently running apps matching bundleId."
            }
        },
        "required": ["bundleId", "nativeWindowId"]
    })
}

fn computer_capture_native_window_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "pid": {
                "type": "integer",
                "description": "Owner process id from computer/list_app_windows or computer/list_native_windows."
            },
            "nativeWindowId": {
                "type": "integer",
                "minimum": 0,
                "maximum": 4_294_967_295u64,
                "description": "Moment-in-time CoreGraphics native window id to capture."
            },
            "hiDpi": {
                "type": "boolean",
                "default": false,
                "description": "Return native Retina pixels when true; otherwise downscale through the existing screenshot path."
            },
            "includeImage": {
                "type": "boolean",
                "default": false,
                "description": "Include pngBase64 in the JSON receipt. When false, return only dimensions, SHA-256, and pixel audit."
            },
            "expectedBundleId": {
                "type": "string",
                "description": "Optional exact bundle-id guard. Capture is refused if pid no longer belongs to this bundle."
            }
        },
        "required": ["pid", "nativeWindowId"]
    })
}

fn computer_capture_render_window_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "target": automation_window_target_schema(),
            "expected": {
                "type": "object",
                "description": "Exact AutomationTargetIdentitySnapshot for the completed frame. Required by qualified owned readback; optional for ordinary-runtime compatibility.",
                "properties": {
                    "windowId": { "type": "string" },
                    "windowGeneration": { "type": "integer", "minimum": 0 },
                    "appViewVariant": { "type": "string" },
                    "targetGeneration": { "type": "integer", "minimum": 0 },
                    "surfaceGeneration": { "type": "integer", "minimum": 0 },
                    "dataGeneration": { "type": "integer", "minimum": 0 },
                    "presentationRevision": { "type": "integer", "minimum": 0 },
                    "themeRevision": { "type": "integer", "minimum": 0 },
                    "frameGeneration": { "type": "integer", "minimum": 0 }
                },
                "required": ["windowId", "appViewVariant", "targetGeneration", "surfaceGeneration", "dataGeneration"]
            },
            "probes": {
                "type": "array", "maxItems": 64,
                "description": "Native-resolution coordinates in the exact retained completed frame; never draws or captures the desktop.",
                "items": { "type": "object", "additionalProperties": false,
                    "properties": { "x": { "type": "integer", "minimum": 0 }, "y": { "type": "integer", "minimum": 0 } },
                    "required": ["x", "y"] }
            },
            "hiDpi": {
                "type": "boolean",
                "default": false,
                "description": "Return high-DPI app-render pixels when the runtime readback path supports them."
            },
            "includeImage": {
                "type": "boolean",
                "default": false,
                "description": "Include pngBase64 in the JSON receipt. When false, return only dimensions, SHA-256, and pixel audit."
            }
        },
        "required": ["target"]
    })
}

fn computer_list_native_windows_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "includeHidden": {
                "type": "boolean",
                "default": false,
                "description": "Include hidden running GUI applications."
            },
            "includeBackground": {
                "type": "boolean",
                "default": false,
                "description": "Include accessory, prohibited, and unknown background applications in addition to regular GUI apps."
            }
        }
    })
}

fn computer_get_native_window_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "nativeWindowId": { "type": "integer", "minimum": 0, "maximum": 4_294_967_295u64 }
        },
        "required": ["nativeWindowId"]
    })
}

fn computer_get_app_window_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "pid": { "type": "integer" },
            "nativeWindowId": { "type": "integer", "minimum": 0, "maximum": 4294967295u64 }
        },
        "required": ["pid", "nativeWindowId"]
    })
}

fn computer_get_frontmost_native_window_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_list_frontmost_app_windows_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_get_frontmost_app_window_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "nativeWindowId": {
                "type": "integer",
                "minimum": 0,
                "maximum": u32::MAX as u64,
                "description": "CoreGraphics native window id from computer/list_frontmost_app_windows."
            }
        },
        "required": ["nativeWindowId"]
    })
}

fn computer_get_frontmost_app_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_list_menus_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_list_menu_item_paths_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_get_menu_item_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "path": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "string",
                    "minLength": 1
                },
                "description": "Exact cached menu title path, e.g. [\"File\", \"New Window\"]. Call computer/list_menus first."
            }
        },
        "required": ["path"]
    })
}

fn computer_get_menu_item_by_index_path_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "indexPath": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "integer",
                    "minimum": 0
                },
                "description": "Zero-based recursive index path. Use indexPath from computer/list_menu_item_paths, or derive the same position from computer/list_menus."
            }
        },
        "required": ["indexPath"]
    })
}

fn computer_list_tray_menu_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_get_tray_menu_item_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "sectionIndex": { "type": "integer", "minimum": 0 },
            "itemIndex": { "type": "integer", "minimum": 0 }
        },
        "required": ["sectionIndex", "itemIndex"]
    })
}

fn computer_get_tray_menu_item_by_id_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": {
                "type": "string",
                "minLength": 1,
                "description": "Stable tray menu item id from computer/list_tray_menu."
            }
        },
        "required": ["id"]
    })
}

fn computer_list_screens_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_get_screen_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "displayId": {
                "type": "integer",
                "minimum": 0,
                "maximum": 4_294_967_295u64,
            }
        },
        "required": ["displayId"]
    })
}

fn computer_list_permissions_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn computer_get_permission_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": {
                "type": "string",
                "enum": ["accessibility", "screenRecording", "eventSynthesizing"],
                "description": "Permission id from computer/list_permissions."
            }
        },
        "required": ["id"]
    })
}
