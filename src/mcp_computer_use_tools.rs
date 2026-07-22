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
