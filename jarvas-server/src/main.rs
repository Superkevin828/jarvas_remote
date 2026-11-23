use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::Deserialize;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

mod platform;
mod safety;

fn ok(message: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(message)])
}

fn fail(message: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(format!("JARVAS error: {message}"))])
}

macro_rules! run {
    ($expression:expr) => {
        match $expression {
            Ok(message) => Ok(ok(message)),
            Err(message) => Ok(fail(message)),
        }
    };
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MoveMouseParams {
    /// Absolute screen X coordinate in pixels, measured from the left edge of the primary display.
    x: i32,
    /// Absolute screen Y coordinate in pixels, measured from the top edge of the primary display.
    y: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ClickParams {
    /// Mouse button: left, right, or middle. Defaults to left.
    #[serde(default = "default_button")]
    button: String,
    /// Set true for a double-click. Defaults to false.
    #[serde(default)]
    double: bool,
    /// Optional X coordinate to move the cursor to before clicking.
    x: Option<i32>,
    /// Optional Y coordinate to move the cursor to before clicking.
    y: Option<i32>,
}

fn default_button() -> String {
    "left".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DragParams {
    /// Starting X coordinate.
    from_x: i32,
    /// Starting Y coordinate.
    from_y: i32,
    /// Ending X coordinate.
    to_x: i32,
    /// Ending Y coordinate.
    to_y: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ScrollParams {
    /// Number of wheel notches. Positive scrolls up, negative scrolls down.
    amount: i32,
    /// Optional X coordinate to move the cursor to before scrolling.
    x: Option<i32>,
    /// Optional Y coordinate to move the cursor to before scrolling.
    y: Option<i32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TextParams {
    /// Exact text to type using Unicode input (supports any characters).
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct KeyParams {
    /// Key name, e.g. enter, esc, tab, space, backspace, delete, up, down, f1, a, 7.
    key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HotkeyParams {
    /// Combination of modifiers and one key joined by '+', e.g. Ctrl+C, Alt+Tab, Ctrl+Shift+S, Win+R.
    combo: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WindowParams {
    /// Numeric window handle from list_windows, e.g. 123456. Takes priority over title_contains.
    window_handle: Option<u64>,
    /// Case-insensitive fragment of the window title, e.g. Notepad.
    title_contains: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CloseWindowParams {
    /// Numeric window handle from list_windows.
    window_handle: Option<u64>,
    /// Case-insensitive fragment of the window title.
    title_contains: Option<String>,
    /// Must be true because closing a window can lose unsaved work.
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MoveWindowParams {
    /// Numeric window handle from list_windows.
    window_handle: Option<u64>,
    /// Case-insensitive fragment of the window title.
    title_contains: Option<String>,
    /// New left edge in pixels.
    x: i32,
    /// New top edge in pixels.
    y: i32,
    /// New width in pixels; must be positive.
    width: i32,
    /// New height in pixels; must be positive.
    height: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LaunchAppParams {
    /// App name (notepad, calc, code) or a full path such as C:\Windows\System32\notepad.exe.
    name_or_path: String,
    /// Optional command-line arguments, e.g. --new-window.
    args: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CloseAppParams {
    /// Executable name or process name, e.g. notepad, notepad.exe, chrome.
    process_name: String,
    /// Must be true because terminating a process can lose unsaved work.
    confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ScreenshotParams {
    /// Optional window handle from list_windows to capture only that window; omit for full screen.
    window_handle: Option<u64>,
    /// Optional absolute path to also save the PNG, e.g. C:\Users\Me\Desktop\shot.png.
    path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FindElementParams {
    /// Exact UI element name (label text), e.g. Save. Case-sensitive in Windows UI Automation.
    name: Option<String>,
    /// Role such as button, edit, text, window, checkbox, combobox, menuitem, tabitem, hyperlink.
    role: Option<String>,
    /// Optional window handle to limit the search to one app.
    window_handle: Option<u64>,
    /// Maximum number of matches to return (1-100, default 20).
    max_results: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ClickElementParams {
    /// Exact UI element name to click, e.g. OK.
    name: Option<String>,
    /// Element role such as button, menuitem, or hyperlink.
    role: Option<String>,
    /// Which match to click when several elements match (default 0).
    #[serde(default)]
    index: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LogParams {
    /// Number of recent action-log lines to return (default 20).
    limit: Option<usize>,
}

#[derive(Clone)]
struct Jarvas;

#[tool_router]
impl Jarvas {
    #[tool(
        description = "Move the mouse cursor to an absolute screen coordinate.",
        annotations(
            title = "Move mouse",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn move_mouse(
        &self,
        Parameters(params): Parameters<MoveMouseParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::input::move_mouse(params.x, params.y))
    }

    #[tool(
        description = "Click the left, right, or middle mouse button, optionally at a coordinate. Use double=true for a double-click.",
        annotations(
            title = "Click",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn click(
        &self,
        Parameters(params): Parameters<ClickParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::input::click(
            &params.button,
            params.double,
            params.x,
            params.y
        ))
    }

    #[tool(
        description = "Drag the mouse from one screen coordinate to another using the left button.",
        annotations(
            title = "Drag",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn drag(
        &self,
        Parameters(params): Parameters<DragParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::input::drag(
            params.from_x,
            params.from_y,
            params.to_x,
            params.to_y
        ))
    }

    #[tool(
        description = "Scroll the mouse wheel at the current cursor position or at a given coordinate. Positive amount scrolls up, negative scrolls down.",
        annotations(
            title = "Scroll",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn scroll(
        &self,
        Parameters(params): Parameters<ScrollParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::input::scroll(params.amount, params.x, params.y))
    }

    #[tool(
        description = "Return the current cursor position as JSON with x and y fields.",
        annotations(
            title = "Get cursor position",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn get_cursor_position(&self) -> Result<CallToolResult, McpError> {
        run!(platform::input::get_cursor_position())
    }

    #[tool(
        description = "Type arbitrary text into the focused app using Unicode key events. Use this to fill text fields.",
        annotations(
            title = "Type text",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn type_text(
        &self,
        Parameters(params): Parameters<TextParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::input::type_text(&params.text))
    }

    #[tool(
        description = "Press and release a single key by name, e.g. enter, esc, tab, space, backspace, delete, f1, a.",
        annotations(
            title = "Press key",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn press_key(
        &self,
        Parameters(params): Parameters<KeyParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::input::press_key(&params.key))
    }

    #[tool(
        description = "Press a keyboard shortcut such as Ctrl+C, Alt+Tab, Ctrl+Shift+S, or Win+R.",
        annotations(
            title = "Hotkey",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn hotkey(
        &self,
        Parameters(params): Parameters<HotkeyParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::input::hotkey(&params.combo))
    }

    #[tool(
        description = "List all visible top-level windows with their handles, titles, classes, and screen positions. Call this first when you need to target a window.",
        annotations(
            title = "List windows",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn list_windows(&self) -> Result<CallToolResult, McpError> {
        run!(platform::window::list_windows())
    }

    #[tool(
        description = "Bring a window to the foreground. Identify it by window_handle or title_contains from list_windows.",
        annotations(
            title = "Focus window",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn focus_window(
        &self,
        Parameters(params): Parameters<WindowParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::window::focus_window(
            params.window_handle,
            params.title_contains.as_deref()
        ))
    }

    #[tool(
        description = "Ask a window to close by sending WM_CLOSE. DESTRUCTIVE: requires confirm=true because unsaved work may be lost.",
        annotations(
            title = "Close window",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn close_window(
        &self,
        Parameters(params): Parameters<CloseWindowParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::window::close_window(
            params.window_handle,
            params.title_contains.as_deref(),
            params.confirm
        ))
    }

    #[tool(
        description = "Minimize a window identified by handle or title fragment.",
        annotations(
            title = "Minimize",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn minimize(
        &self,
        Parameters(params): Parameters<WindowParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::window::minimize(
            params.window_handle,
            params.title_contains.as_deref()
        ))
    }

    #[tool(
        description = "Maximize a window identified by handle or title fragment.",
        annotations(
            title = "Maximize",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn maximize(
        &self,
        Parameters(params): Parameters<WindowParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::window::maximize(
            params.window_handle,
            params.title_contains.as_deref()
        ))
    }

    #[tool(
        description = "Restore a minimized or maximized window to its normal size.",
        annotations(
            title = "Restore window",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn restore_window(
        &self,
        Parameters(params): Parameters<WindowParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::window::restore_window(
            params.window_handle,
            params.title_contains.as_deref()
        ))
    }

    #[tool(
        description = "Move and/or resize a window to an absolute screen position. Identify it by handle or title fragment.",
        annotations(
            title = "Move/resize window",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn move_window(
        &self,
        Parameters(params): Parameters<MoveWindowParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::window::move_window(
            params.window_handle,
            params.title_contains.as_deref(),
            params.x,
            params.y,
            params.width,
            params.height
        ))
    }

    #[tool(
        description = "Launch an app by name (searches PATH and Windows App Paths) or by full path, with optional arguments.",
        annotations(
            title = "Launch app",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn launch_app(
        &self,
        Parameters(params): Parameters<LaunchAppParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::window::launch_app(
            &params.name_or_path,
            params.args.as_deref()
        ))
    }

    #[tool(
        description = "Terminate a running app by process name. DESTRUCTIVE: requires confirm=true because unsaved work will be lost.",
        annotations(
            title = "Close app",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn close_app(
        &self,
        Parameters(params): Parameters<CloseAppParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::window::close_app(
            &params.process_name,
            params.confirm
        ))
    }

    #[tool(
        description = "Take a screenshot of the whole screen or one window and return it as a PNG image. Optionally save it to a file path.",
        annotations(
            title = "Screenshot",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn screenshot(
        &self,
        Parameters(params): Parameters<ScreenshotParams>,
    ) -> Result<CallToolResult, McpError> {
        match platform::vision::capture(params.window_handle, params.path.as_deref()) {
            Ok(shot) => {
                let encoded = STANDARD.encode(&shot.png);
                let mut blocks = vec![ContentBlock::image(encoded, "image/png".to_string())];
                let saved = shot
                    .saved_to
                    .map(|path| format!(" Saved to {path}."))
                    .unwrap_or_default();
                blocks.push(ContentBlock::text(format!(
                    "Screenshot captured ({}x{} PNG, {} bytes).{saved}",
                    shot.width,
                    shot.height,
                    shot.png.len()
                )));
                Ok(CallToolResult::success(blocks))
            }
            Err(message) => Ok(fail(message)),
        }
    }

    #[tool(
        description = "Return the screen and virtual-desktop size in pixels as JSON.",
        annotations(
            title = "Get screen size",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn get_screen_size(&self) -> Result<CallToolResult, McpError> {
        run!(platform::vision::get_screen_size())
    }

    #[tool(
        description = "Find UI elements by name and/or role using Windows UI Automation. Returns names, roles, automation IDs, and screen bounds so you can click without guessing pixels.",
        annotations(
            title = "Find element",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn find_element(
        &self,
        Parameters(params): Parameters<FindElementParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::uia::find_element(
            params.name.as_deref(),
            params.role.as_deref(),
            params.window_handle,
            params.max_results.unwrap_or(20)
        ))
    }

    #[tool(
        description = "Click a UI element by its name and/or role using UI Automation (InvokePattern when available, otherwise a click at its center).",
        annotations(
            title = "Click element",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn click_element(
        &self,
        Parameters(params): Parameters<ClickElementParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::uia::click_element(
            params.name.as_deref(),
            params.role.as_deref(),
            params.index
        ))
    }

    #[tool(
        description = "Read plain text from the Windows clipboard.",
        annotations(
            title = "Get clipboard",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn get_clipboard(&self) -> Result<CallToolResult, McpError> {
        run!(platform::clipboard::get_clipboard())
    }

    #[tool(
        description = "Replace the Windows clipboard contents with the given text.",
        annotations(
            title = "Set clipboard",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn set_clipboard(
        &self,
        Parameters(params): Parameters<TextParams>,
    ) -> Result<CallToolResult, McpError> {
        run!(platform::clipboard::set_clipboard(&params.text))
    }

    #[tool(
        description = "Return the most recent JARVAS action-log lines so you can verify what was done. The panic hotkey is Ctrl+Shift+J.",
        annotations(
            title = "Get action log",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_action_log(
        &self,
        Parameters(params): Parameters<LogParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(ok(safety::action_log(
            params.limit.unwrap_or(20).clamp(1, 200),
        )))
    }
}

#[tool_handler(
    name = "jarvas",
    version = "0.1.0",
    instructions = "JARVAS controls this Windows PC: mouse, keyboard, windows, apps, screenshots, UI Automation, clipboard. Press Ctrl+Shift+J at any time to trigger an emergency stop. Destructive tools require confirm=true."
)]
impl ServerHandler for Jarvas {}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    safety::init_panic_hotkey();
    eprintln!("JARVAS MCP server v0.1.0 ready (stdio). Panic hotkey: Ctrl+Shift+J");
    let service = Jarvas.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
