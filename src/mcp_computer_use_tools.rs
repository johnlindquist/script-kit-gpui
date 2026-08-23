//! MCP computer-use tools.
//!
//! Iteration 1 exposes `computer/see` as the agent-facing name for Script Kit's
//! existing `inspectAutomationWindow` snapshot contract. Native input actions
//! remain deferred until they can cite stable inspection receipts.

use crate::computer_use::runtime_bridge::{
    ComputerUseAppWindowInfo, ComputerUseCaptureNativeWindowError,
    ComputerUseCaptureNativeWindowRequest, ComputerUseCaptureRenderWindowRequest,
    ComputerUseCaptureRenderWindowSnapshot, ComputerUseCaptureRenderWindowStatus,
    ComputerUseInspectRequest, ComputerUseListAppWindowsRequest, ComputerUseListAppsRequest,
    ComputerUseRunningAppInfo, ComputerUseRuntimeBridge, ComputerUseRuntimeError,
};
use crate::computer_use::types::{
    ComputerUseCaptureNativeWindowArgs, ComputerUseCaptureRenderWindowArgs, ComputerUseSeeArgs,
};
use crate::frontmost_app_tracker::{get_cached_menu_snapshot, get_last_real_app};
use crate::mcp_kit_tools::{ToolContent, ToolDefinition, ToolResult};
use crate::menu_bar::MenuBarItem;
use crate::protocol::{
    AutomationWindowInfo, DisplayInfo, TargetWindowBounds, AUTOMATION_WINDOW_SCHEMA_VERSION,
};
use serde_json::Value;

pub const COMPUTER_USE_NAMESPACE: &str = "computer/";
pub const COMPUTER_SEE_TOOL: &str = "computer/see";
pub const COMPUTER_LIST_WINDOWS_TOOL: &str = "computer/list_windows";
pub const COMPUTER_GET_WINDOW_TOOL: &str = "computer/get_window";
pub const COMPUTER_GET_FOCUSED_WINDOW_TOOL: &str = "computer/get_focused_window";
pub const COMPUTER_LIST_APPS_TOOL: &str = "computer/list_apps";
pub const COMPUTER_GET_APP_TOOL: &str = "computer/get_app";
pub const COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL: &str = "computer/list_apps_by_bundle_id";
pub const COMPUTER_LIST_APP_WINDOWS_TOOL: &str = "computer/list_app_windows";
pub const COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL: &str =
    "computer/list_app_windows_by_bundle_id";
pub const COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL: &str = "computer/get_app_window_by_bundle_id";
pub const COMPUTER_CAPTURE_NATIVE_WINDOW_TOOL: &str = "computer/capture_native_window";
pub const COMPUTER_CAPTURE_RENDER_WINDOW_TOOL: &str = "computer/capture_render_window";
pub const COMPUTER_LIST_NATIVE_WINDOWS_TOOL: &str = "computer/list_native_windows";
pub const COMPUTER_GET_NATIVE_WINDOW_TOOL: &str = "computer/get_native_window";
pub const COMPUTER_GET_APP_WINDOW_TOOL: &str = "computer/get_app_window";
pub const COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL: &str = "computer/get_frontmost_native_window";
pub const COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL: &str = "computer/list_frontmost_app_windows";
pub const COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL: &str = "computer/get_frontmost_app_window";
pub const COMPUTER_GET_FRONTMOST_APP_TOOL: &str = "computer/get_frontmost_app";
pub const COMPUTER_LIST_MENUS_TOOL: &str = "computer/list_menus";
pub const COMPUTER_LIST_MENU_ITEM_PATHS_TOOL: &str = "computer/list_menu_item_paths";
pub const COMPUTER_GET_MENU_ITEM_TOOL: &str = "computer/get_menu_item";
pub const COMPUTER_GET_MENU_ITEM_BY_INDEX_PATH_TOOL: &str = "computer/get_menu_item_by_index_path";
pub const COMPUTER_LIST_TRAY_MENU_TOOL: &str = "computer/list_tray_menu";
pub const COMPUTER_GET_TRAY_MENU_ITEM_TOOL: &str = "computer/get_tray_menu_item";
pub const COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL: &str = "computer/get_tray_menu_item_by_id";
pub const COMPUTER_LIST_SCREENS_TOOL: &str = "computer/list_screens";
pub const COMPUTER_GET_SCREEN_TOOL: &str = "computer/get_screen";
pub const COMPUTER_LIST_PERMISSIONS_TOOL: &str = "computer/list_permissions";
pub const COMPUTER_GET_PERMISSION_TOOL: &str = "computer/get_permission";
const COMPUTER_APPS_SCHEMA_VERSION: u32 = 1;
const COMPUTER_APP_WINDOWS_SCHEMA_VERSION: u32 = 1;
const COMPUTER_NATIVE_WINDOWS_SCHEMA_VERSION: u32 = 1;
const COMPUTER_FRONTMOST_NATIVE_WINDOW_SCHEMA_VERSION: u32 = 1;
const COMPUTER_FRONTMOST_APP_WINDOWS_SCHEMA_VERSION: u32 = 1;
const COMPUTER_FRONTMOST_APP_WINDOW_SCHEMA_VERSION: u32 = 1;
const COMPUTER_FRONTMOST_APP_SCHEMA_VERSION: u32 = 1;
const COMPUTER_MENUS_SCHEMA_VERSION: u32 = 1;
const COMPUTER_SCREENS_SCHEMA_VERSION: u32 = 1;
const COMPUTER_PERMISSIONS_SCHEMA_VERSION: u32 = 1;

pub fn get_computer_use_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: COMPUTER_SEE_TOOL.to_string(),
            description:
                "Inspect a Script Kit automation window and return a state-first computer-use observation."
                    .to_string(),
            input_schema: computer_see_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_WINDOWS_TOOL.to_string(),
            description: "List registered Script Kit automation windows without interacting with them."
                .to_string(),
            input_schema: computer_list_windows_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_WINDOW_TOOL.to_string(),
            description: "Return one registered Script Kit automation window by stable automation window id without screenshots, native focus changes, or runtime inspection."
                .to_string(),
            input_schema: computer_get_window_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_FOCUSED_WINDOW_TOOL.to_string(),
            description: "Return the focused Script Kit automation window from the automation registry without screenshots, native focus changes, or runtime inspection."
                .to_string(),
            input_schema: computer_get_focused_window_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_APPS_TOOL.to_string(),
            description: "List running GUI applications without launching, quitting, focusing, hiding, or sending input."
                .to_string(),
            input_schema: computer_list_apps_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_APP_TOOL.to_string(),
            description: "Return one running GUI application by PID without launching, quitting, focusing, hiding, or sending input."
                .to_string(),
            input_schema: computer_get_app_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL.to_string(),
            description: "List currently running GUI applications matching an exact bundle id without launching, quitting, focusing, hiding, or sending input."
                .to_string(),
            input_schema: computer_list_apps_by_bundle_id_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_APP_WINDOWS_TOOL.to_string(),
            description: "List native windows for one running GUI application by PID without focusing, moving, resizing, or capturing screenshots."
                .to_string(),
            input_schema: computer_list_app_windows_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL.to_string(),
            description: "List native windows for every running GUI application matching an exact bundle id without focusing, activating, launching, quitting, hiding, moving, resizing, capturing screenshots, or sending input."
                .to_string(),
            input_schema: computer_list_app_windows_by_bundle_id_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL.to_string(),
            description: "Return one native window owned by a currently running GUI application matching an exact bundle id and CoreGraphics window id without focusing, activating, launching, quitting, hiding, moving, resizing, capturing screenshots, or sending input."
                .to_string(),
            input_schema: computer_get_app_window_by_bundle_id_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_CAPTURE_NATIVE_WINDOW_TOOL.to_string(),
            description: "Capture a PNG screenshot of one exact native macOS window after PID, nativeWindowId, optional bundle-id ownership, and capture-candidate revalidation. Does not focus, activate, move, resize, click, type, request permissions, or fall back to another window."
                .to_string(),
            input_schema: computer_capture_native_window_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_CAPTURE_RENDER_WINDOW_TOOL.to_string(),
            description: "Capture app-rendered GPUI pixels for one resolved Script Kit automation window from inside the live runtime. Does not focus, activate, move, resize, click, type, request permissions, or use macOS WindowServer screenshot capture. This does not prove macOS WindowServer compositor/native blur output."
                .to_string(),
            input_schema: computer_capture_render_window_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_NATIVE_WINDOWS_TOOL.to_string(),
            description: "List native windows grouped by running GUI application without focusing, activating, moving, resizing, capturing screenshots, or sending input."
                .to_string(),
            input_schema: computer_list_native_windows_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_NATIVE_WINDOW_TOOL.to_string(),
            description: "Return one native window by CoreGraphics window id across running GUI applications without focusing, activating, moving, resizing, capturing screenshots, or sending input."
                .to_string(),
            input_schema: computer_get_native_window_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_APP_WINDOW_TOOL.to_string(),
            description: "Return one native window for one running GUI application by PID and CoreGraphics window id without focusing, moving, resizing, capturing screenshots, or sending input."
                .to_string(),
            input_schema: computer_get_app_window_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL.to_string(),
            description: "Return the current frontmost app's top native window without focusing, activating, launching, quitting, hiding, moving, resizing, capturing screenshots, inspecting AX elements, requesting permissions, enumerating menu extras, exposing action handles, or sending input."
                .to_string(),
            input_schema: computer_get_frontmost_native_window_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL.to_string(),
            description: "List the current frontmost app's native windows without focusing, activating, launching, quitting, hiding, moving, resizing, capturing screenshots, inspecting AX elements, requesting permissions, enumerating menu extras, exposing action handles, or sending input."
                .to_string(),
            input_schema: computer_list_frontmost_app_windows_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL.to_string(),
            description: "Return one native window from the current frontmost app by CoreGraphics window id without focusing, activating, launching, quitting, hiding, moving, resizing, capturing screenshots, inspecting AX elements, requesting permissions, enumerating menu extras, exposing action handles, or sending input."
                .to_string(),
            input_schema: computer_get_frontmost_app_window_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_FRONTMOST_APP_TOOL.to_string(),
            description: "Return the last tracked non-Script-Kit frontmost app from the frontmost app tracker cache without refreshing, focusing, activating, or requesting permissions."
                .to_string(),
            input_schema: computer_get_frontmost_app_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_MENUS_TOOL.to_string(),
            description: "List cached menu items for the last tracked real application without refreshing, focusing, clicking, or requesting permissions."
                .to_string(),
            input_schema: computer_list_menus_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_MENU_ITEM_PATHS_TOOL.to_string(),
            description: "List flattened cached menu item paths and zero-based index paths without refreshing menus, focusing apps, clicking, or requesting permissions."
                .to_string(),
            input_schema: computer_list_menu_item_paths_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_MENU_ITEM_TOOL.to_string(),
            description: "Return one cached menu item by exact title path without refreshing menus, focusing apps, clicking, or requesting permissions."
                .to_string(),
            input_schema: computer_get_menu_item_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_MENU_ITEM_BY_INDEX_PATH_TOOL.to_string(),
            description: "Return one cached menu item by zero-based recursive index path without refreshing menus, focusing apps, clicking, or requesting permissions."
                .to_string(),
            input_schema: computer_get_menu_item_by_index_path_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_TRAY_MENU_TOOL.to_string(),
            description: "List Script Kit's own tray menu model without opening the menu, clicking status items, invoking actions, or requesting permissions."
                .to_string(),
            input_schema: computer_list_tray_menu_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_TRAY_MENU_ITEM_TOOL.to_string(),
            description: "Return one Script Kit tray menu item by section and item index without opening the menu, clicking status items, invoking actions, or requesting permissions."
                .to_string(),
            input_schema: computer_get_tray_menu_item_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL.to_string(),
            description: "Return one Script Kit tray menu item by stable tray item id without opening the menu, clicking status items, invoking actions, or requesting permissions."
                .to_string(),
            input_schema: computer_get_tray_menu_item_by_id_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_SCREENS_TOOL.to_string(),
            description: "List attached screens/displays without moving windows, changing screen placement, or requesting permissions."
                .to_string(),
            input_schema: computer_list_screens_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_SCREEN_TOOL.to_string(),
            description: "Return one attached screen/display by CoreGraphics display id without moving windows, changing screen placement, capturing screenshots, or requesting permissions."
                .to_string(),
            input_schema: computer_get_screen_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_LIST_PERMISSIONS_TOOL.to_string(),
            description: "List read-only macOS permission status for Script Kit computer-use features without requesting permissions."
                .to_string(),
            input_schema: computer_list_permissions_input_schema(),
        },
        ToolDefinition {
            name: COMPUTER_GET_PERMISSION_TOOL.to_string(),
            description: "Return one read-only macOS permission status by permission id without requesting permissions, opening settings, synthesizing events, or mutating app/window state."
                .to_string(),
            input_schema: computer_get_permission_input_schema(),
        },
    ]
}

pub fn is_computer_use_tool(name: &str) -> bool {
    name.starts_with(COMPUTER_USE_NAMESPACE)
}

pub fn handle_computer_use_tool_call(
    name: &str,
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    match name {
        COMPUTER_SEE_TOOL => handle_see(arguments, runtime),
        COMPUTER_LIST_WINDOWS_TOOL => handle_list_windows(arguments),
        COMPUTER_GET_WINDOW_TOOL => handle_get_window(arguments),
        COMPUTER_GET_FOCUSED_WINDOW_TOOL => handle_get_focused_window(arguments),
        COMPUTER_LIST_APPS_TOOL => handle_list_apps(arguments, runtime),
        COMPUTER_GET_APP_TOOL => handle_get_app(arguments, runtime),
        COMPUTER_LIST_APPS_BY_BUNDLE_ID_TOOL => handle_list_apps_by_bundle_id(arguments, runtime),
        COMPUTER_LIST_APP_WINDOWS_TOOL => handle_list_app_windows(arguments, runtime),
        COMPUTER_LIST_APP_WINDOWS_BY_BUNDLE_ID_TOOL => {
            handle_list_app_windows_by_bundle_id(arguments, runtime)
        }
        COMPUTER_GET_APP_WINDOW_BY_BUNDLE_ID_TOOL => {
            handle_get_app_window_by_bundle_id(arguments, runtime)
        }
        COMPUTER_CAPTURE_NATIVE_WINDOW_TOOL => handle_capture_native_window(arguments, runtime),
        COMPUTER_CAPTURE_RENDER_WINDOW_TOOL => handle_capture_render_window(arguments, runtime),
        COMPUTER_LIST_NATIVE_WINDOWS_TOOL => handle_list_native_windows(arguments, runtime),
        COMPUTER_GET_NATIVE_WINDOW_TOOL => handle_get_native_window(arguments, runtime),
        COMPUTER_GET_APP_WINDOW_TOOL => handle_get_app_window(arguments, runtime),
        COMPUTER_GET_FRONTMOST_NATIVE_WINDOW_TOOL => {
            handle_get_frontmost_native_window(arguments, runtime)
        }
        COMPUTER_LIST_FRONTMOST_APP_WINDOWS_TOOL => {
            handle_list_frontmost_app_windows(arguments, runtime)
        }
        COMPUTER_GET_FRONTMOST_APP_WINDOW_TOOL => {
            handle_get_frontmost_app_window(arguments, runtime)
        }
        COMPUTER_GET_FRONTMOST_APP_TOOL => handle_get_frontmost_app(arguments),
        COMPUTER_LIST_MENUS_TOOL => handle_list_menus(arguments),
        COMPUTER_LIST_MENU_ITEM_PATHS_TOOL => handle_list_menu_item_paths(arguments),
        COMPUTER_GET_MENU_ITEM_TOOL => handle_get_menu_item(arguments),
        COMPUTER_GET_MENU_ITEM_BY_INDEX_PATH_TOOL => handle_get_menu_item_by_index_path(arguments),
        COMPUTER_LIST_TRAY_MENU_TOOL => handle_list_tray_menu(arguments),
        COMPUTER_GET_TRAY_MENU_ITEM_TOOL => handle_get_tray_menu_item(arguments),
        COMPUTER_GET_TRAY_MENU_ITEM_BY_ID_TOOL => handle_get_tray_menu_item_by_id(arguments),
        COMPUTER_LIST_SCREENS_TOOL => handle_list_screens(arguments),
        COMPUTER_GET_SCREEN_TOOL => handle_get_screen(arguments),
        COMPUTER_LIST_PERMISSIONS_TOOL => handle_list_permissions(arguments),
        COMPUTER_GET_PERMISSION_TOOL => handle_get_permission(arguments),
        _ => error_result(
            "unknown_tool",
            &format!("Unknown computer-use tool: {name}"),
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseListWindowsArgs {}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListWindowsResult {
    schema_version: u32,
    windows: Vec<AutomationWindowInfo>,
    focused_window_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseGetWindowArgs {
    id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetWindowResult {
    schema_version: u32,
    source: &'static str,
    status: &'static str,
    window: Option<AutomationWindowInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseGetFocusedWindowArgs {}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetFocusedWindowResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    focused_window_id: Option<String>,
    window: Option<AutomationWindowInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseListAppsArgs {
    #[serde(default)]
    include_hidden: bool,
    #[serde(default)]
    include_background: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListAppsResult {
    schema_version: u32,
    apps: Vec<ComputerUseRunningAppInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frontmost_pid: Option<i32>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseGetAppArgs {
    pid: i32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetAppResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    app: Option<ComputerUseRunningAppInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseListAppsByBundleIdArgs {
    bundle_id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListAppsByBundleIdResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    bundle_id: String,
    app_count: usize,
    apps: Vec<ComputerUseRunningAppInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseListAppWindowsArgs {
    pid: i32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListAppWindowsResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    app: Option<ComputerUseRunningAppInfo>,
    windows: Vec<ComputerUseAppWindowInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseListAppWindowsByBundleIdArgs {
    bundle_id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListAppWindowsByBundleIdResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    bundle_id: String,
    app_count: usize,
    window_count: usize,
    apps: Vec<ComputerUseNativeWindowsForApp>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseGetAppWindowByBundleIdArgs {
    bundle_id: String,
    native_window_id: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetAppWindowByBundleIdResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    bundle_id: String,
    native_window_id: u32,
    app_count: usize,
    app: Option<ComputerUseRunningAppInfo>,
    window: Option<ComputerUseAppWindowInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseListNativeWindowsArgs {
    #[serde(default)]
    include_hidden: bool,
    #[serde(default)]
    include_background: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListNativeWindowsResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    frontmost_pid: Option<i32>,
    app_count: usize,
    window_count: usize,
    apps: Vec<ComputerUseNativeWindowsForApp>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseNativeWindowsForApp {
    app: ComputerUseRunningAppInfo,
    status: &'static str,
    windows: Vec<ComputerUseAppWindowInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseGetNativeWindowArgs {
    native_window_id: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetNativeWindowResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    native_window_id: u32,
    app: Option<ComputerUseRunningAppInfo>,
    window: Option<ComputerUseAppWindowInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseGetAppWindowArgs {
    pid: i32,
    native_window_id: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetAppWindowResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    app: Option<ComputerUseRunningAppInfo>,
    window: Option<ComputerUseAppWindowInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseGetFrontmostNativeWindowArgs {}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetFrontmostNativeWindowResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    frontmost_pid: Option<i32>,
    app: Option<ComputerUseRunningAppInfo>,
    window: Option<ComputerUseAppWindowInfo>,
    window_count: usize,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseListFrontmostAppWindowsArgs {}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListFrontmostAppWindowsResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    frontmost_pid: Option<i32>,
    app: Option<ComputerUseRunningAppInfo>,
    window_count: usize,
    windows: Vec<ComputerUseAppWindowInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseGetFrontmostAppWindowArgs {
    native_window_id: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetFrontmostAppWindowResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    native_window_id: u32,
    frontmost_pid: Option<i32>,
    app: Option<ComputerUseRunningAppInfo>,
    window: Option<ComputerUseAppWindowInfo>,
    window_count: usize,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseGetFrontmostAppArgs {}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetFrontmostAppResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    app: Option<ComputerUseFrontmostApp>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseFrontmostApp {
    pid: i32,
    bundle_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_title: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseListMenusArgs {}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseListMenuItemPathsArgs {}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseGetMenuItemArgs {
    path: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseGetMenuItemByIndexPathArgs {
    index_path: Vec<usize>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseListTrayMenuArgs {}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseGetTrayMenuItemArgs {
    section_index: usize,
    item_index: usize,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseGetTrayMenuItemByIdArgs {
    id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListMenusResult {
    schema_version: u32,
    source: &'static str,
    app: Option<ComputerUseMenuApp>,
    cache: ComputerUseMenuCache,
    menus: Vec<ComputerUseMenuItem>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListMenuItemPathsResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    app: Option<ComputerUseMenuApp>,
    cache: ComputerUseMenuCache,
    items: Vec<ComputerUseMenuItemPath>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseMenuItemPath {
    index_path: Vec<usize>,
    path: Vec<String>,
    title: String,
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    shortcut: Option<String>,
    child_count: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetMenuItemResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    app: Option<ComputerUseMenuApp>,
    cache: ComputerUseMenuCache,
    path: Vec<String>,
    item: Option<ComputerUseMenuItem>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetMenuItemByIndexPathResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    app: Option<ComputerUseMenuApp>,
    cache: ComputerUseMenuCache,
    index_path: Vec<usize>,
    resolved_path: Option<Vec<String>>,
    item: Option<ComputerUseMenuItem>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseMenuApp {
    pid: i32,
    bundle_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_title: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseMenuCache {
    status: &'static str,
    is_fetching: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseMenuItem {
    title: String,
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    shortcut: Option<String>,
    children: Vec<ComputerUseMenuItem>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetTrayMenuItemResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    owner: crate::tray::TrayMenuOwnerObservation,
    section_index: usize,
    item_index: usize,
    section: Option<ComputerUseTrayMenuSectionSummary>,
    item: Option<crate::tray::TrayMenuItemObservation>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetTrayMenuItemByIdResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    owner: crate::tray::TrayMenuOwnerObservation,
    id: String,
    section_index: Option<usize>,
    item_index: Option<usize>,
    section: Option<ComputerUseTrayMenuSectionSummary>,
    item: Option<crate::tray::TrayMenuItemObservation>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseTrayMenuSectionSummary {
    id: &'static str,
    label: &'static str,
    item_count: usize,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseListScreensArgs {}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListScreensResult {
    schema_version: u32,
    screens: Vec<DisplayInfo>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComputerUseGetScreenArgs {
    display_id: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetScreenResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    screen: Option<DisplayInfo>,
    warnings: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseListPermissionsArgs {}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseListPermissionsResult {
    schema_version: u32,
    permissions: Vec<ComputerUsePermissionStatus>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ComputerUseGetPermissionArgs {
    id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUseGetPermissionResult {
    schema_version: u32,
    source: &'static str,
    scope: &'static str,
    status: &'static str,
    permission: Option<ComputerUsePermissionStatus>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputerUsePermissionStatus {
    id: &'static str,
    name: &'static str,
    granted: Option<bool>,
    status: &'static str,
}

fn handle_see(arguments: &Value, runtime: Option<&dyn ComputerUseRuntimeBridge>) -> ToolResult {
    let args: ComputerUseSeeArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return runtime_error_result(&args, ComputerUseRuntimeError::Unavailable);
    };

    let request = ComputerUseInspectRequest {
        target: args.target.clone(),
        hi_dpi: args.hi_dpi,
        probes: args.probes.clone(),
    };

    match runtime.inspect_automation_window(request) {
        Ok(snapshot) => json_tool_result(&snapshot),
        Err(error) => runtime_error_result(&args, error),
    }
}

fn handle_list_windows(arguments: &Value) -> ToolResult {
    let _args: ComputerUseListWindowsArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    json_tool_result(&ComputerUseListWindowsResult {
        schema_version: AUTOMATION_WINDOW_SCHEMA_VERSION,
        windows: crate::windows::list_automation_windows(),
        focused_window_id: crate::windows::focused_automation_window_id(),
    })
}

fn handle_get_window(arguments: &Value) -> ToolResult {
    let args: ComputerUseGetWindowArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let window = crate::windows::automation_window_by_id(&args.id);

    json_tool_result(&ComputerUseGetWindowResult {
        schema_version: AUTOMATION_WINDOW_SCHEMA_VERSION,
        source: "automationWindowRegistry",
        status: if window.is_some() {
            "found"
        } else {
            "notFound"
        },
        window,
        warnings: Vec::new(),
    })
}

fn handle_get_focused_window(arguments: &Value) -> ToolResult {
    let _args: ComputerUseGetFocusedWindowArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let window = crate::windows::focused_automation_window();
    let focused_window_id = window.as_ref().map(|window| window.id.clone());

    json_tool_result(&ComputerUseGetFocusedWindowResult {
        schema_version: AUTOMATION_WINDOW_SCHEMA_VERSION,
        source: "automationWindowRegistry",
        scope: "focusedAutomationWindow",
        status: if window.is_some() {
            "focused"
        } else {
            "noFocusedWindow"
        },
        focused_window_id,
        window,
        warnings: Vec::new(),
    })
}

fn handle_list_apps(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseListAppsArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/list_apps requires the live GPUI runtime bridge to enumerate running applications safely",
        );
    };

    let request = ComputerUseListAppsRequest {
        include_hidden: args.include_hidden,
        include_background: args.include_background,
    };

    match runtime.list_running_apps(request) {
        Ok(snapshot) => json_tool_result(&ComputerUseListAppsResult {
            schema_version: COMPUTER_APPS_SCHEMA_VERSION,
            apps: snapshot.apps,
            frontmost_pid: snapshot.frontmost_pid,
        }),
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn handle_get_app(arguments: &Value, runtime: Option<&dyn ComputerUseRuntimeBridge>) -> ToolResult {
    let args: ComputerUseGetAppArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/get_app requires the live GPUI runtime bridge to enumerate running applications safely",
        );
    };

    let request = ComputerUseListAppsRequest {
        include_hidden: true,
        include_background: true,
    };

    match runtime.list_running_apps(request) {
        Ok(snapshot) => {
            let app = snapshot.apps.into_iter().find(|app| app.pid == args.pid);
            json_tool_result(&ComputerUseGetAppResult {
                schema_version: COMPUTER_APPS_SCHEMA_VERSION,
                source: "nsWorkspaceRunningApplications",
                scope: "runningAppPid",
                status: if app.is_some() { "found" } else { "notFound" },
                app,
                warnings: Vec::new(),
            })
        }
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn handle_list_apps_by_bundle_id(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseListAppsByBundleIdArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    if args.bundle_id.is_empty() {
        return error_result("invalid_arguments", "bundleId must not be empty");
    }

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/list_apps_by_bundle_id requires the live GPUI runtime bridge to enumerate running applications safely",
        );
    };

    let snapshot = match runtime.list_running_apps(ComputerUseListAppsRequest {
        include_hidden: true,
        include_background: true,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => return error_result(error.error_code(), &error.message()),
    };

    let apps: Vec<ComputerUseRunningAppInfo> = snapshot
        .apps
        .into_iter()
        .filter(|app| app.bundle_id.as_deref() == Some(args.bundle_id.as_str()))
        .collect();

    json_tool_result(&ComputerUseListAppsByBundleIdResult {
        schema_version: COMPUTER_APPS_SCHEMA_VERSION,
        source: "nsWorkspaceRunningApplications",
        scope: "runningAppBundleId",
        status: if apps.is_empty() {
            "notFound"
        } else {
            "listed"
        },
        bundle_id: args.bundle_id,
        app_count: apps.len(),
        apps,
        warnings: Vec::new(),
    })
}

fn handle_list_app_windows(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseListAppWindowsArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/list_app_windows requires the live GPUI runtime bridge to enumerate app windows safely",
        );
    };

    let request = ComputerUseListAppWindowsRequest { pid: args.pid };

    match runtime.list_app_windows(request) {
        Ok(snapshot) => json_tool_result(&ComputerUseListAppWindowsResult {
            schema_version: COMPUTER_APP_WINDOWS_SCHEMA_VERSION,
            source: "coreGraphicsWindowList",
            scope: "runningAppPid",
            status: if snapshot.app.is_some() {
                "found"
            } else {
                "notFound"
            },
            app: snapshot.app,
            windows: snapshot.windows,
            warnings: snapshot.warnings,
        }),
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn handle_list_app_windows_by_bundle_id(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseListAppWindowsByBundleIdArgs =
        match serde_json::from_value(arguments.clone()) {
            Ok(args) => args,
            Err(error) => return error_result("invalid_arguments", &error.to_string()),
        };

    if args.bundle_id.is_empty() {
        return error_result("invalid_arguments", "bundleId must not be empty");
    }

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/list_app_windows_by_bundle_id requires the live GPUI runtime bridge to enumerate app windows safely",
        );
    };

    let apps_snapshot = match runtime.list_running_apps(ComputerUseListAppsRequest {
        include_hidden: true,
        include_background: true,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => return error_result(error.error_code(), &error.message()),
    };

    let matching_apps: Vec<ComputerUseRunningAppInfo> = apps_snapshot
        .apps
        .into_iter()
        .filter(|app| app.bundle_id.as_deref() == Some(args.bundle_id.as_str()))
        .collect();

    if matching_apps.is_empty() {
        return json_tool_result(&ComputerUseListAppWindowsByBundleIdResult {
            schema_version: COMPUTER_APP_WINDOWS_SCHEMA_VERSION,
            source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
            scope: "runningAppBundleId",
            status: "notFound",
            bundle_id: args.bundle_id,
            app_count: 0,
            window_count: 0,
            apps: Vec::new(),
            warnings: Vec::new(),
        });
    }

    let mut app_groups = Vec::new();
    let mut warnings = Vec::new();
    let mut partial = false;
    let mut window_count = 0usize;

    for app in matching_apps {
        match runtime.list_app_windows(ComputerUseListAppWindowsRequest { pid: app.pid }) {
            Ok(snapshot) => {
                let Some(snapshot_app) = snapshot.app else {
                    partial = true;
                    app_groups.push(ComputerUseNativeWindowsForApp {
                        app,
                        status: "appNotFound",
                        windows: Vec::new(),
                        warnings: snapshot.warnings,
                    });
                    continue;
                };

                if snapshot_app.bundle_id.as_deref() != Some(args.bundle_id.as_str()) {
                    partial = true;
                    let warning = format!(
                        "bundleIdChanged for pid {} while listing bundleId {}",
                        app.pid, args.bundle_id
                    );
                    warnings.push(warning.clone());
                    let mut app_warnings = snapshot.warnings;
                    app_warnings.push(warning);
                    app_groups.push(ComputerUseNativeWindowsForApp {
                        app,
                        status: "bundleIdChanged",
                        windows: Vec::new(),
                        warnings: app_warnings,
                    });
                    continue;
                }

                window_count += snapshot.windows.len();

                app_groups.push(ComputerUseNativeWindowsForApp {
                    app: snapshot_app,
                    status: "listed",
                    windows: snapshot.windows,
                    warnings: snapshot.warnings,
                });
            }
            Err(error) => {
                partial = true;
                let warning = format!("windowListFailed for pid {}: {}", app.pid, error.message());
                warnings.push(warning.clone());
                app_groups.push(ComputerUseNativeWindowsForApp {
                    app,
                    status: "windowListFailed",
                    windows: Vec::new(),
                    warnings: vec![warning],
                });
            }
        }
    }

    json_tool_result(&ComputerUseListAppWindowsByBundleIdResult {
        schema_version: COMPUTER_APP_WINDOWS_SCHEMA_VERSION,
        source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
        scope: "runningAppBundleId",
        status: if partial { "partial" } else { "listed" },
        bundle_id: args.bundle_id,
        app_count: app_groups.len(),
        window_count,
        apps: app_groups,
        warnings,
    })
}

fn handle_get_app_window_by_bundle_id(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseGetAppWindowByBundleIdArgs =
        match serde_json::from_value(arguments.clone()) {
            Ok(args) => args,
            Err(error) => return error_result("invalid_arguments", &error.to_string()),
        };

    if args.bundle_id.is_empty() {
        return error_result("invalid_arguments", "bundleId must not be empty");
    }

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/get_app_window_by_bundle_id requires the live GPUI runtime bridge to enumerate app windows safely",
        );
    };

    let apps_snapshot = match runtime.list_running_apps(ComputerUseListAppsRequest {
        include_hidden: true,
        include_background: true,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => return error_result(error.error_code(), &error.message()),
    };

    let matching_apps: Vec<ComputerUseRunningAppInfo> = apps_snapshot
        .apps
        .into_iter()
        .filter(|app| app.bundle_id.as_deref() == Some(args.bundle_id.as_str()))
        .collect();
    let app_count = matching_apps.len();

    if matching_apps.is_empty() {
        return json_tool_result(&ComputerUseGetAppWindowByBundleIdResult {
            schema_version: COMPUTER_APP_WINDOWS_SCHEMA_VERSION,
            source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
            scope: "runningAppBundleIdNativeWindowId",
            status: "appNotFound",
            bundle_id: args.bundle_id,
            native_window_id: args.native_window_id,
            app_count,
            app: None,
            window: None,
            warnings: Vec::new(),
        });
    }

    let mut warnings = Vec::new();
    let mut partial = false;

    for app in matching_apps {
        match runtime.list_app_windows(ComputerUseListAppWindowsRequest { pid: app.pid }) {
            Ok(snapshot) => {
                let Some(snapshot_app) = snapshot.app else {
                    partial = true;
                    warnings.push(format!(
                        "appNotFound for pid {} while searching bundleId {} nativeWindowId {}",
                        app.pid, args.bundle_id, args.native_window_id
                    ));
                    warnings.extend(snapshot.warnings);
                    continue;
                };

                if snapshot_app.bundle_id.as_deref() != Some(args.bundle_id.as_str()) {
                    partial = true;
                    warnings.push(format!(
                        "bundleIdChanged for pid {} while searching bundleId {} nativeWindowId {}",
                        app.pid, args.bundle_id, args.native_window_id
                    ));
                    warnings.extend(snapshot.warnings);
                    continue;
                }

                warnings.extend(snapshot.warnings);

                if let Some(window) = snapshot
                    .windows
                    .into_iter()
                    .find(|window| window.native_window_id == args.native_window_id)
                {
                    return json_tool_result(&ComputerUseGetAppWindowByBundleIdResult {
                        schema_version: COMPUTER_APP_WINDOWS_SCHEMA_VERSION,
                        source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
                        scope: "runningAppBundleIdNativeWindowId",
                        status: "found",
                        bundle_id: args.bundle_id,
                        native_window_id: args.native_window_id,
                        app_count,
                        app: Some(snapshot_app),
                        window: Some(window),
                        warnings,
                    });
                }
            }
            Err(error) => {
                partial = true;
                warnings.push(format!(
                    "windowListFailed for pid {} while searching bundleId {} nativeWindowId {}: {}",
                    app.pid,
                    args.bundle_id,
                    args.native_window_id,
                    error.message()
                ));
            }
        }
    }

    json_tool_result(&ComputerUseGetAppWindowByBundleIdResult {
        schema_version: COMPUTER_APP_WINDOWS_SCHEMA_VERSION,
        source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
        scope: "runningAppBundleIdNativeWindowId",
        status: if partial { "partial" } else { "windowNotFound" },
        bundle_id: args.bundle_id,
        native_window_id: args.native_window_id,
        app_count,
        app: None,
        window: None,
        warnings,
    })
}

fn handle_list_native_windows(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseListNativeWindowsArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/list_native_windows requires the live GPUI runtime bridge to enumerate native windows safely",
        );
    };

    let apps_snapshot = match runtime.list_running_apps(ComputerUseListAppsRequest {
        include_hidden: args.include_hidden,
        include_background: args.include_background,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => return error_result(error.error_code(), &error.message()),
    };

    let mut app_groups = Vec::new();
    let mut warnings = Vec::new();
    let mut partial = false;
    let mut window_count = 0usize;

    for app in apps_snapshot.apps {
        match runtime.list_app_windows(ComputerUseListAppWindowsRequest { pid: app.pid }) {
            Ok(snapshot) => {
                let status = if snapshot.app.is_some() {
                    "listed"
                } else {
                    partial = true;
                    "appNotFound"
                };
                window_count += snapshot.windows.len();

                app_groups.push(ComputerUseNativeWindowsForApp {
                    app,
                    status,
                    windows: snapshot.windows,
                    warnings: snapshot.warnings,
                });
            }
            Err(error) => {
                partial = true;
                let warning = format!("windowListFailed for pid {}: {}", app.pid, error.message());
                warnings.push(warning.clone());
                app_groups.push(ComputerUseNativeWindowsForApp {
                    app,
                    status: "windowListFailed",
                    windows: Vec::new(),
                    warnings: vec![warning],
                });
            }
        }
    }

    json_tool_result(&ComputerUseListNativeWindowsResult {
        schema_version: COMPUTER_NATIVE_WINDOWS_SCHEMA_VERSION,
        source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
        scope: "runningGuiApps",
        status: if partial { "partial" } else { "listed" },
        frontmost_pid: apps_snapshot.frontmost_pid,
        app_count: app_groups.len(),
        window_count,
        apps: app_groups,
        warnings,
    })
}

fn handle_get_native_window(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseGetNativeWindowArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/get_native_window requires the live GPUI runtime bridge to enumerate native windows safely",
        );
    };

    let apps_snapshot = match runtime.list_running_apps(ComputerUseListAppsRequest {
        include_hidden: true,
        include_background: true,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => return error_result(error.error_code(), &error.message()),
    };

    let mut warnings = Vec::new();
    let mut partial = false;

    for app in apps_snapshot.apps {
        match runtime.list_app_windows(ComputerUseListAppWindowsRequest { pid: app.pid }) {
            Ok(snapshot) => {
                if snapshot.app.is_none() {
                    partial = true;
                    warnings.push(format!(
                        "appNotFound for pid {} while searching nativeWindowId {}",
                        app.pid, args.native_window_id
                    ));
                }

                warnings.extend(snapshot.warnings);

                if let Some(window) = snapshot
                    .windows
                    .into_iter()
                    .find(|window| window.native_window_id == args.native_window_id)
                {
                    return json_tool_result(&ComputerUseGetNativeWindowResult {
                        schema_version: COMPUTER_NATIVE_WINDOWS_SCHEMA_VERSION,
                        source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
                        scope: "nativeWindowId",
                        status: "found",
                        native_window_id: args.native_window_id,
                        app: snapshot.app.or(Some(app)),
                        window: Some(window),
                        warnings,
                    });
                }
            }
            Err(error) => {
                partial = true;
                warnings.push(format!(
                    "windowListFailed for pid {} while searching nativeWindowId {}: {}",
                    app.pid,
                    args.native_window_id,
                    error.message()
                ));
            }
        }
    }

    json_tool_result(&ComputerUseGetNativeWindowResult {
        schema_version: COMPUTER_NATIVE_WINDOWS_SCHEMA_VERSION,
        source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
        scope: "nativeWindowId",
        status: if partial { "partial" } else { "notFound" },
        native_window_id: args.native_window_id,
        app: None,
        window: None,
        warnings,
    })
}

fn handle_get_app_window(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseGetAppWindowArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/get_app_window requires the live GPUI runtime bridge to enumerate app windows safely",
        );
    };

    let request = ComputerUseListAppWindowsRequest { pid: args.pid };

    match runtime.list_app_windows(request) {
        Ok(snapshot) => {
            let app = snapshot.app;
            let window = if app.is_some() {
                snapshot
                    .windows
                    .into_iter()
                    .find(|window| window.native_window_id == args.native_window_id)
            } else {
                None
            };
            let status = match (&app, &window) {
                (Some(_), Some(_)) => "found",
                (Some(_), None) => "windowNotFound",
                (None, _) => "appNotFound",
            };

            json_tool_result(&ComputerUseGetAppWindowResult {
                schema_version: COMPUTER_APP_WINDOWS_SCHEMA_VERSION,
                source: "coreGraphicsWindowList",
                scope: "runningAppPidNativeWindowId",
                status,
                app,
                window,
                warnings: snapshot.warnings,
            })
        }
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn handle_capture_native_window(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseCaptureNativeWindowArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/capture_native_window requires the live GPUI runtime bridge to revalidate and capture native windows safely",
        );
    };

    let correlation_id = format!(
        "mcp-computer-capture-native-window:{}",
        uuid::Uuid::new_v4()
    );
    tracing::info!(
        target: "script_kit::automation",
        correlation_id = %correlation_id,
        pid = args.pid,
        native_window_id = args.native_window_id,
        expected_bundle_id = ?args.expected_bundle_id,
        hi_dpi = args.hi_dpi,
        include_image = args.include_image,
        "computer.capture_native_window.request"
    );

    let request = ComputerUseCaptureNativeWindowRequest {
        pid: args.pid,
        native_window_id: args.native_window_id,
        hi_dpi: args.hi_dpi,
        include_image: args.include_image,
        expected_bundle_id: args.expected_bundle_id,
        correlation_id: correlation_id.clone(),
    };

    match runtime.capture_native_window(request) {
        Ok(snapshot) => {
            tracing::info!(
                target: "script_kit::automation",
                correlation_id = %snapshot.correlation_id,
                status = ?snapshot.status,
                error_code = ?snapshot.error.as_ref().map(|error| error.code),
                pid = args.pid,
                native_window_id = args.native_window_id,
                byte_length = ?snapshot.capture.as_ref().map(|capture| capture.byte_length),
                sha256 = ?snapshot.capture.as_ref().map(|capture| capture.sha256.as_str()),
                width = ?snapshot.capture.as_ref().map(|capture| capture.width),
                height = ?snapshot.capture.as_ref().map(|capture| capture.height),
                returned_image = snapshot
                    .capture
                    .as_ref()
                    .and_then(|capture| capture.png_base64.as_ref())
                    .is_some(),
                "computer.capture_native_window.result"
            );
            json_tool_result(&snapshot)
        }
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn handle_capture_render_window(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseCaptureRenderWindowArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let correlation_id = format!(
        "mcp-computer-capture-render-window:{}",
        uuid::Uuid::new_v4()
    );
    let target = args.target.clone();
    let request = ComputerUseCaptureRenderWindowRequest {
        target: target.clone(),
        hi_dpi: args.hi_dpi,
        include_image: args.include_image,
        correlation_id: correlation_id.clone(),
    };

    let Some(runtime) = runtime else {
        let snapshot = ComputerUseCaptureRenderWindowSnapshot {
            schema_version: 1,
            source: "gpuiRenderReadback",
            scope: "liveAutomationWindowRenderReadback",
            status: ComputerUseCaptureRenderWindowStatus::Unsupported,
            correlation_id,
            target,
            capture: None,
            error: Some(ComputerUseCaptureNativeWindowError {
                code: "runtime_unavailable",
                message: "computer/capture_render_window requires the live GPUI runtime bridge"
                    .to_string(),
                reason: Some("runtime_unavailable".to_string()),
                pixel_audit: None,
            }),
            warnings: vec![
                "No pixels were captured; do not count this as app-render visual proof."
                    .to_string(),
            ],
            limitation: "App-rendered GPUI pixels only; does not prove macOS WindowServer compositor/native blur output.",
        };
        return json_tool_result(&snapshot);
    };

    match runtime.capture_render_window(request) {
        Ok(snapshot) => json_tool_result(&snapshot),
        Err(ComputerUseRuntimeError::Unavailable) => {
            let snapshot = ComputerUseCaptureRenderWindowSnapshot {
                schema_version: 1,
                source: "gpuiRenderReadback",
                scope: "liveAutomationWindowRenderReadback",
                status: ComputerUseCaptureRenderWindowStatus::Unsupported,
                correlation_id,
                target,
                capture: None,
                error: Some(ComputerUseCaptureNativeWindowError {
                    code: "gpui_readback_unavailable",
                    message: "GPUI render readback is not implemented in this runtime".to_string(),
                    reason: Some("unsupported".to_string()),
                    pixel_audit: None,
                }),
                warnings: vec![
                    "No pixels were captured; do not count this as app-render visual proof."
                        .to_string(),
                ],
                limitation: "App-rendered GPUI pixels only; does not prove macOS WindowServer compositor/native blur output.",
            };
            json_tool_result(&snapshot)
        }
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn handle_get_frontmost_native_window(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let _args: ComputerUseGetFrontmostNativeWindowArgs =
        match serde_json::from_value(arguments.clone()) {
            Ok(args) => args,
            Err(error) => return error_result("invalid_arguments", &error.to_string()),
        };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/get_frontmost_native_window requires the live GPUI runtime bridge to enumerate the frontmost native window safely",
        );
    };

    let apps_snapshot = match runtime.list_running_apps(ComputerUseListAppsRequest {
        include_hidden: true,
        include_background: true,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => return error_result(error.error_code(), &error.message()),
    };

    let Some(frontmost_pid) = apps_snapshot.frontmost_pid else {
        return json_tool_result(&ComputerUseGetFrontmostNativeWindowResult {
            schema_version: COMPUTER_FRONTMOST_NATIVE_WINDOW_SCHEMA_VERSION,
            source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
            scope: "frontmostNativeWindow",
            status: "noFrontmostApp",
            frontmost_pid: None,
            app: None,
            window: None,
            window_count: 0,
            warnings: Vec::new(),
        });
    };

    match runtime.list_app_windows(ComputerUseListAppWindowsRequest { pid: frontmost_pid }) {
        Ok(snapshot) => {
            let app = snapshot.app;
            let window_count = snapshot.windows.len();
            let window = if app.is_some() {
                choose_frontmost_native_window(snapshot.windows)
            } else {
                None
            };
            let status = match (&app, &window) {
                (None, _) => "appNotFound",
                (Some(_), Some(_)) => "found",
                (Some(_), None) => "noWindows",
            };

            json_tool_result(&ComputerUseGetFrontmostNativeWindowResult {
                schema_version: COMPUTER_FRONTMOST_NATIVE_WINDOW_SCHEMA_VERSION,
                source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
                scope: "frontmostNativeWindow",
                status,
                frontmost_pid: Some(frontmost_pid),
                app,
                window,
                window_count,
                warnings: snapshot.warnings,
            })
        }
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn choose_frontmost_native_window(
    windows: Vec<ComputerUseAppWindowInfo>,
) -> Option<ComputerUseAppWindowInfo> {
    windows
        .into_iter()
        .min_by_key(|window| (window.z_order, window.native_window_id))
}

fn handle_list_frontmost_app_windows(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let _args: ComputerUseListFrontmostAppWindowsArgs =
        match serde_json::from_value(arguments.clone()) {
            Ok(args) => args,
            Err(error) => return error_result("invalid_arguments", &error.to_string()),
        };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/list_frontmost_app_windows requires the live GPUI runtime bridge to enumerate the frontmost app windows safely",
        );
    };

    let apps_snapshot = match runtime.list_running_apps(ComputerUseListAppsRequest {
        include_hidden: true,
        include_background: true,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => return error_result(error.error_code(), &error.message()),
    };

    let Some(frontmost_pid) = apps_snapshot.frontmost_pid else {
        return json_tool_result(&ComputerUseListFrontmostAppWindowsResult {
            schema_version: COMPUTER_FRONTMOST_APP_WINDOWS_SCHEMA_VERSION,
            source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
            scope: "frontmostAppWindows",
            status: "noFrontmostApp",
            frontmost_pid: None,
            app: None,
            window_count: 0,
            windows: Vec::new(),
            warnings: Vec::new(),
        });
    };

    match runtime.list_app_windows(ComputerUseListAppWindowsRequest { pid: frontmost_pid }) {
        Ok(snapshot) => {
            let app = snapshot.app;
            let window_count = snapshot.windows.len();
            let status = if app.is_none() {
                "appNotFound"
            } else if snapshot.windows.is_empty() {
                "noWindows"
            } else {
                "listed"
            };

            json_tool_result(&ComputerUseListFrontmostAppWindowsResult {
                schema_version: COMPUTER_FRONTMOST_APP_WINDOWS_SCHEMA_VERSION,
                source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
                scope: "frontmostAppWindows",
                status,
                frontmost_pid: Some(frontmost_pid),
                app,
                window_count,
                windows: snapshot.windows,
                warnings: snapshot.warnings,
            })
        }
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn handle_get_frontmost_app_window(
    arguments: &Value,
    runtime: Option<&dyn ComputerUseRuntimeBridge>,
) -> ToolResult {
    let args: ComputerUseGetFrontmostAppWindowArgs = match serde_json::from_value(arguments.clone())
    {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let Some(runtime) = runtime else {
        return error_result(
            "runtime_unavailable",
            "computer/get_frontmost_app_window requires the live GPUI runtime bridge to enumerate the frontmost app window safely",
        );
    };

    let apps_snapshot = match runtime.list_running_apps(ComputerUseListAppsRequest {
        include_hidden: true,
        include_background: true,
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => return error_result(error.error_code(), &error.message()),
    };

    let Some(frontmost_pid) = apps_snapshot.frontmost_pid else {
        return json_tool_result(&ComputerUseGetFrontmostAppWindowResult {
            schema_version: COMPUTER_FRONTMOST_APP_WINDOW_SCHEMA_VERSION,
            source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
            scope: "frontmostAppNativeWindowId",
            status: "noFrontmostApp",
            native_window_id: args.native_window_id,
            frontmost_pid: None,
            app: None,
            window: None,
            window_count: 0,
            warnings: Vec::new(),
        });
    };

    match runtime.list_app_windows(ComputerUseListAppWindowsRequest { pid: frontmost_pid }) {
        Ok(snapshot) => {
            let app = snapshot.app;
            let window_count = snapshot.windows.len();
            let window = if app.is_some() {
                snapshot
                    .windows
                    .into_iter()
                    .find(|window| window.native_window_id == args.native_window_id)
            } else {
                None
            };
            let status = match (&app, window_count, &window) {
                (None, _, _) => "appNotFound",
                (Some(_), 0, _) => "noWindows",
                (Some(_), _, Some(_)) => "found",
                (Some(_), _, None) => "windowNotFound",
            };

            json_tool_result(&ComputerUseGetFrontmostAppWindowResult {
                schema_version: COMPUTER_FRONTMOST_APP_WINDOW_SCHEMA_VERSION,
                source: "nsWorkspaceRunningApplications+coreGraphicsWindowList",
                scope: "frontmostAppNativeWindowId",
                status,
                native_window_id: args.native_window_id,
                frontmost_pid: Some(frontmost_pid),
                app,
                window,
                window_count,
                warnings: snapshot.warnings,
            })
        }
        Err(error) => error_result(error.error_code(), &error.message()),
    }
}

fn handle_get_frontmost_app(arguments: &Value) -> ToolResult {
    let _args: ComputerUseGetFrontmostAppArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let app = get_last_real_app().map(|app| ComputerUseFrontmostApp {
        pid: app.pid,
        bundle_id: app.bundle_id,
        name: app.name,
        window_title: app.window_title,
    });

    json_tool_result(&ComputerUseGetFrontmostAppResult {
        schema_version: COMPUTER_FRONTMOST_APP_SCHEMA_VERSION,
        source: "frontmostAppTrackerCache",
        scope: "lastNonScriptKitApp",
        status: if app.is_some() {
            "tracked"
        } else {
            "noTrackedApp"
        },
        app,
        warnings: Vec::new(),
    })
}

fn handle_list_menus(arguments: &Value) -> ToolResult {
    let _args: ComputerUseListMenusArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let snapshot = get_cached_menu_snapshot();
    let app = snapshot.app.map(|app| ComputerUseMenuApp {
        pid: app.pid,
        bundle_id: app.bundle_id,
        name: app.name,
        window_title: app.window_title,
    });

    json_tool_result(&ComputerUseListMenusResult {
        schema_version: COMPUTER_MENUS_SCHEMA_VERSION,
        source: "frontmostAppTrackerCache",
        app,
        cache: ComputerUseMenuCache {
            status: snapshot.status.as_str(),
            is_fetching: snapshot.is_fetching,
        },
        menus: snapshot.items.iter().map(computer_use_menu_item).collect(),
        warnings: Vec::new(),
    })
}

fn handle_list_menu_item_paths(arguments: &Value) -> ToolResult {
    let _args: ComputerUseListMenuItemPathsArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let snapshot = get_cached_menu_snapshot();
    let app = snapshot.app.map(|app| ComputerUseMenuApp {
        pid: app.pid,
        bundle_id: app.bundle_id,
        name: app.name,
        window_title: app.window_title,
    });
    let status = if app.is_none() {
        "noTrackedApp"
    } else if snapshot.items.is_empty() {
        "noCachedMenus"
    } else {
        "listed"
    };
    let mut items = Vec::new();
    if status == "listed" {
        flatten_cached_menu_item_paths(
            &snapshot.items,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut items,
        );
    }

    json_tool_result(&ComputerUseListMenuItemPathsResult {
        schema_version: COMPUTER_MENUS_SCHEMA_VERSION,
        source: "frontmostAppTrackerCache",
        scope: "cachedMenuItemPaths",
        status,
        app,
        cache: ComputerUseMenuCache {
            status: snapshot.status.as_str(),
            is_fetching: snapshot.is_fetching,
        },
        items,
        warnings: Vec::new(),
    })
}

fn handle_get_menu_item(arguments: &Value) -> ToolResult {
    let args: ComputerUseGetMenuItemArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    if args.path.is_empty() || args.path.iter().any(|segment| segment.is_empty()) {
        return error_result(
            "invalid_arguments",
            "path must contain at least one non-empty menu title segment",
        );
    }

    let snapshot = get_cached_menu_snapshot();
    let app = snapshot.app.map(|app| ComputerUseMenuApp {
        pid: app.pid,
        bundle_id: app.bundle_id,
        name: app.name,
        window_title: app.window_title,
    });
    let item =
        find_cached_menu_item_by_path(&snapshot.items, &args.path).map(computer_use_menu_item);
    let status = if app.is_none() {
        "noTrackedApp"
    } else if snapshot.items.is_empty() {
        "noCachedMenus"
    } else if item.is_some() {
        "found"
    } else {
        "notFound"
    };

    json_tool_result(&ComputerUseGetMenuItemResult {
        schema_version: COMPUTER_MENUS_SCHEMA_VERSION,
        source: "frontmostAppTrackerCache",
        scope: "cachedMenuPath",
        status,
        app,
        cache: ComputerUseMenuCache {
            status: snapshot.status.as_str(),
            is_fetching: snapshot.is_fetching,
        },
        path: args.path,
        item,
        warnings: Vec::new(),
    })
}

fn handle_get_menu_item_by_index_path(arguments: &Value) -> ToolResult {
    let args: ComputerUseGetMenuItemByIndexPathArgs =
        match serde_json::from_value(arguments.clone()) {
            Ok(args) => args,
            Err(error) => return error_result("invalid_arguments", &error.to_string()),
        };

    if args.index_path.is_empty() {
        return error_result(
            "invalid_arguments",
            "indexPath must contain at least one index",
        );
    }

    let snapshot = get_cached_menu_snapshot();
    let app = snapshot.app.map(|app| ComputerUseMenuApp {
        pid: app.pid,
        bundle_id: app.bundle_id,
        name: app.name,
        window_title: app.window_title,
    });
    let found = if app.is_some() && !snapshot.items.is_empty() {
        find_cached_menu_item_by_index_path(&snapshot.items, &args.index_path)
    } else {
        None
    };
    let status = if app.is_none() {
        "noTrackedApp"
    } else if snapshot.items.is_empty() {
        "noCachedMenus"
    } else if found.is_some() {
        "found"
    } else {
        "notFound"
    };
    let (item, resolved_path) = match found {
        Some((item, resolved_path)) => (Some(computer_use_menu_item(item)), Some(resolved_path)),
        None => (None, None),
    };

    json_tool_result(&ComputerUseGetMenuItemByIndexPathResult {
        schema_version: COMPUTER_MENUS_SCHEMA_VERSION,
        source: "frontmostAppTrackerCache",
        scope: "cachedMenuIndexPath",
        status,
        app,
        cache: ComputerUseMenuCache {
            status: snapshot.status.as_str(),
            is_fetching: snapshot.is_fetching,
        },
        index_path: args.index_path,
        resolved_path,
        item,
        warnings: Vec::new(),
    })
}

fn handle_list_tray_menu(arguments: &Value) -> ToolResult {
    let _args: ComputerUseListTrayMenuArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    json_tool_result(&crate::tray::current_tray_menu_observation_snapshot())
}

fn handle_get_tray_menu_item(arguments: &Value) -> ToolResult {
    let args: ComputerUseGetTrayMenuItemArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let snapshot = crate::tray::current_tray_menu_observation_snapshot();
    let section = snapshot.sections.get(args.section_index);
    let section_summary = section.map(computer_use_tray_menu_section_summary);
    let item = section.and_then(|section| section.items.get(args.item_index).cloned());
    let status = if section.is_none() {
        "sectionNotFound"
    } else if item.is_none() {
        "itemNotFound"
    } else {
        "found"
    };
    let mut warnings = snapshot.warnings;
    if status == "sectionNotFound" {
        warnings.push(format!(
            "tray menu section index {} was not found",
            args.section_index
        ));
    } else if status == "itemNotFound" {
        warnings.push(format!(
            "tray menu item index {} was not found in section {}",
            args.item_index, args.section_index
        ));
    }

    json_tool_result(&ComputerUseGetTrayMenuItemResult {
        schema_version: 1,
        source: "scriptKitTrayMenuModel",
        scope: "ownTrayMenuSectionItemIndex",
        status,
        owner: snapshot.owner,
        section_index: args.section_index,
        item_index: args.item_index,
        section: section_summary,
        item,
        warnings,
    })
}

fn handle_get_tray_menu_item_by_id(arguments: &Value) -> ToolResult {
    let args: ComputerUseGetTrayMenuItemByIdArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    if args.id.is_empty() {
        return error_result(
            "invalid_arguments",
            "id must be a non-empty tray menu item id",
        );
    }

    let snapshot = crate::tray::current_tray_menu_observation_snapshot();
    let mut found = None;
    'sections: for (section_index, section) in snapshot.sections.iter().enumerate() {
        for (item_index, item) in section.items.iter().enumerate() {
            if item.id == args.id {
                found = Some((
                    section_index,
                    item_index,
                    computer_use_tray_menu_section_summary(section),
                    item.clone(),
                ));
                break 'sections;
            }
        }
    }

    let mut warnings = snapshot.warnings;
    let (status, section_index, item_index, section, item) = match found {
        Some((section_index, item_index, section, item)) => (
            "found",
            Some(section_index),
            Some(item_index),
            Some(section),
            Some(item),
        ),
        None => {
            warnings.push(format!("tray menu item id {} was not found", args.id));
            ("notFound", None, None, None, None)
        }
    };

    json_tool_result(&ComputerUseGetTrayMenuItemByIdResult {
        schema_version: 1,
        source: "scriptKitTrayMenuModel",
        scope: "ownTrayMenuItemId",
        status,
        owner: snapshot.owner,
        id: args.id,
        section_index,
        item_index,
        section,
        item,
        warnings,
    })
}

fn computer_use_tray_menu_section_summary(
    section: &crate::tray::TrayMenuSectionObservation,
) -> ComputerUseTrayMenuSectionSummary {
    ComputerUseTrayMenuSectionSummary {
        id: section.id,
        label: section.label,
        item_count: section.items.len(),
    }
}

fn find_cached_menu_item_by_path<'a>(
    items: &'a [MenuBarItem],
    path: &[String],
) -> Option<&'a MenuBarItem> {
    let (head, tail) = path.split_first()?;
    let item = items.iter().find(|item| item.title == *head)?;
    if tail.is_empty() {
        Some(item)
    } else {
        find_cached_menu_item_by_path(&item.children, tail)
    }
}

fn find_cached_menu_item_by_index_path<'a>(
    items: &'a [MenuBarItem],
    index_path: &[usize],
) -> Option<(&'a MenuBarItem, Vec<String>)> {
    let (head, tail) = index_path.split_first()?;
    let item = items.get(*head)?;
    if tail.is_empty() {
        Some((item, vec![item.title.clone()]))
    } else {
        let (found, mut path) = find_cached_menu_item_by_index_path(&item.children, tail)?;
        path.insert(0, item.title.clone());
        Some((found, path))
    }
}

fn flatten_cached_menu_item_paths(
    items: &[MenuBarItem],
    title_prefix: &mut Vec<String>,
    index_prefix: &mut Vec<usize>,
    out: &mut Vec<ComputerUseMenuItemPath>,
) {
    for (index, item) in items.iter().enumerate() {
        title_prefix.push(item.title.clone());
        index_prefix.push(index);
        out.push(ComputerUseMenuItemPath {
            index_path: index_prefix.clone(),
            path: title_prefix.clone(),
            title: item.title.clone(),
            enabled: item.enabled,
            shortcut: item
                .shortcut
                .as_ref()
                .map(|shortcut| shortcut.to_display_string()),
            child_count: item.children.len(),
        });
        flatten_cached_menu_item_paths(&item.children, title_prefix, index_prefix, out);
        index_prefix.pop();
        title_prefix.pop();
    }
}

fn computer_use_menu_item(item: &MenuBarItem) -> ComputerUseMenuItem {
    ComputerUseMenuItem {
        title: item.title.clone(),
        enabled: item.enabled,
        shortcut: item
            .shortcut
            .as_ref()
            .map(|shortcut| shortcut.to_display_string()),
        children: item.children.iter().map(computer_use_menu_item).collect(),
    }
}

fn handle_list_screens(arguments: &Value) -> ToolResult {
    let _args: ComputerUseListScreensArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    match list_screens() {
        Ok(screens) => json_tool_result(&ComputerUseListScreensResult {
            schema_version: COMPUTER_SCREENS_SCHEMA_VERSION,
            screens,
        }),
        Err(error) => error_result("screen_list_failed", &error),
    }
}

fn handle_get_screen(arguments: &Value) -> ToolResult {
    let args: ComputerUseGetScreenArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    match list_screens() {
        Ok(screens) => {
            let screen = screens
                .into_iter()
                .find(|screen| screen.display_id == args.display_id);
            let status = if screen.is_some() {
                "found"
            } else {
                "notFound"
            };

            json_tool_result(&ComputerUseGetScreenResult {
                schema_version: COMPUTER_SCREENS_SCHEMA_VERSION,
                source: "coreGraphicsActiveDisplays",
                scope: "displayId",
                status,
                screen,
                warnings: Vec::new(),
            })
        }
        Err(error) => error_result("screen_list_failed", &error),
    }
}

fn handle_list_permissions(arguments: &Value) -> ToolResult {
    let _args: ComputerUseListPermissionsArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    json_tool_result(&ComputerUseListPermissionsResult {
        schema_version: COMPUTER_PERMISSIONS_SCHEMA_VERSION,
        permissions: computer_use_permission_statuses(),
    })
}

fn handle_get_permission(arguments: &Value) -> ToolResult {
    let args: ComputerUseGetPermissionArgs = match serde_json::from_value(arguments.clone()) {
        Ok(args) => args,
        Err(error) => return error_result("invalid_arguments", &error.to_string()),
    };

    let permission = computer_use_permission_statuses()
        .into_iter()
        .find(|permission| permission.id == args.id);
    let status = if permission.is_some() {
        "found"
    } else {
        "notFound"
    };

    json_tool_result(&ComputerUseGetPermissionResult {
        schema_version: COMPUTER_PERMISSIONS_SCHEMA_VERSION,
        source: "macosPermissionPreflight",
        scope: "permissionId",
        status,
        permission,
        warnings: Vec::new(),
    })
}

fn computer_use_permission_statuses() -> Vec<ComputerUsePermissionStatus> {
    vec![
        permission_status(
            "accessibility",
            "Accessibility",
            Some(crate::permissions_wizard::check_accessibility_permission()),
        ),
        permission_status(
            "screenRecording",
            "Screen Recording",
            crate::platform::screen_capture_access_preflight(),
        ),
        permission_status(
            "eventSynthesizing",
            "Event Synthesizing",
            crate::platform::event_synthesizing_access_preflight(),
        ),
    ]
}

fn permission_status(
    id: &'static str,
    name: &'static str,
    granted: Option<bool>,
) -> ComputerUsePermissionStatus {
    let status = match granted {
        Some(true) => "granted",
        Some(false) => "notGranted",
        None => "unknown",
    };

    ComputerUsePermissionStatus {
        id,
        name,
        granted,
        status,
    }
}

#[cfg(target_os = "macos")]
fn list_screens() -> Result<Vec<DisplayInfo>, String> {
    use core_graphics::display::CGDisplay;

    const MACOS_MENU_BAR_HEIGHT: i32 = 24;

    let display_ids =
        CGDisplay::active_displays().map_err(|_| "Failed to get active displays".to_string())?;
    let main_display_id = CGDisplay::main().id;
    let mut screens = Vec::with_capacity(display_ids.len());

    for (index, display_id) in display_ids.iter().copied().enumerate() {
        let display = CGDisplay::new(display_id);
        let bounds = display.bounds();
        let visible_y = bounds.origin.y as i32 + MACOS_MENU_BAR_HEIGHT;
        let visible_height =
            (bounds.size.height as u32).saturating_sub(MACOS_MENU_BAR_HEIGHT as u32);

        screens.push(DisplayInfo {
            display_id,
            name: format!("Display {}", index + 1),
            is_primary: display_id == main_display_id,
            bounds: TargetWindowBounds {
                x: bounds.origin.x as i32,
                y: bounds.origin.y as i32,
                width: bounds.size.width as u32,
                height: bounds.size.height as u32,
            },
            visible_bounds: TargetWindowBounds {
                x: bounds.origin.x as i32,
                y: visible_y,
                width: bounds.size.width as u32,
                height: visible_height,
            },
            scale_factor: None,
        });
    }

    Ok(screens)
}

#[cfg(not(target_os = "macos"))]
fn list_screens() -> Result<Vec<DisplayInfo>, String> {
    Ok(vec![DisplayInfo {
        display_id: 0,
        name: "Primary Display".to_string(),
        is_primary: true,
        bounds: TargetWindowBounds {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        visible_bounds: TargetWindowBounds {
            x: 0,
            y: 24,
            width: 1920,
            height: 1056,
        },
        scale_factor: Some(1.0),
    }])
}

fn runtime_error_result(args: &ComputerUseSeeArgs, error: ComputerUseRuntimeError) -> ToolResult {
    let target = args
        .target
        .as_ref()
        .map(|target| serde_json::to_value(target).unwrap_or(Value::Null));

    ToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: serde_json::json!({
                "schemaVersion": 1,
                "errorCode": error.error_code(),
                "message": error.message(),
                "target": target,
            })
            .to_string(),
        }],
        is_error: Some(true),
    }
}

fn json_tool_result<T: serde::Serialize>(value: &T) -> ToolResult {
    match serde_json::to_string(value) {
        Ok(text) => ToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text,
            }],
            is_error: None,
        },
        Err(error) => error_result("serialization_failed", &error.to_string()),
    }
}

fn error_result(code: &str, message: &str) -> ToolResult {
    ToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: serde_json::json!({
                "schemaVersion": 1,
                "errorCode": code,
                "message": message,
            })
            .to_string(),
        }],
        is_error: Some(true),
    }
}

fn automation_window_target_schema() -> Value {
    serde_json::json!({
        "description": "AutomationWindowTarget. Omit to use the focused automation window where the caller schema allows omission.",
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
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "type": { "const": "kind" },
                    "kind": {
                        "type": "string",
                        "enum": ["main", "notes", "agentChatDetached", "actionsDialog", "promptPopup"]
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
            "target": {
                "description": "AutomationWindowTarget. Omit to use the focused automation window.",
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
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "type": { "const": "kind" },
                            "kind": {
                                "type": "string",
                                "enum": ["main", "notes", "agentChatDetached", "actionsDialog", "promptPopup"]
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
            },
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

#[cfg(test)]
include!("mcp_computer_use_tools_tests.rs");
