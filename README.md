# JARVAS

[Download Windows release](https://github.com/Superkevin828/jarvas_remote/releases/latest/download/jarvas-server.exe) · [Download page](docs/index.html) · [Last chat](docs/CHAT_HISTORY.md)

JARVAS is a local Windows MCP server that lets an MCP-compatible AI assistant control your PC: mouse, keyboard, windows, screenshots, UI Automation, and clipboard.

It does **not** open a normal app window. Your AI client starts `jarvas-server.exe` in the background and talks to it over stdio.

## Contents

- [What it can do](#what-it-can-do)
- [Quick start](#quick-start)
- [Connect to AI chat bots](#connect-to-ai-chat-bots)
- [Safety](#safety)
- [Build from source](#build-from-source)
- [Test the server](#test-the-server)
- [Example prompts](#example-prompts)
- [FAQ](#faq)

## What it can do

| Area | Tools |
| --- | --- |
| Mouse | `move_mouse`, `click`, `drag`, `scroll`, `get_cursor_position` |
| Keyboard | `type_text`, `press_key`, `hotkey` |
| Windows | `list_windows`, `focus_window`, `close_window`, `minimize`, `maximize`, `restore_window`, `move_window` |
| Apps | `launch_app`, `close_app` |
| Vision | `screenshot`, `get_screen_size` |
| UI Automation | `find_element`, `click_element` |
| Clipboard | `get_clipboard`, `set_clipboard` |
| Diagnostics | `get_action_log` |

## Quick start

1. Download [`jarvas-server.exe`](https://github.com/Superkevin828/jarvas_remote/releases/latest/download/jarvas-server.exe).
2. Save it anywhere, for example `C:/Users/YOU/Downloads/jarvas-server.exe`.
3. Paste the config for your AI client below and replace the path with your actual `.exe` location.
4. Restart the AI client, confirm the `jarvas` server is connected, then ask it to control the PC.

> You do not need Rust or Node.js to use the release `.exe`. Rust is only needed if you build from source; Node is only needed for MCP Inspector tests.

## Connect to AI chat bots

JARVAS works with AI apps that support **MCP**. Browser-only chatbots usually cannot control your PC directly.

### OpenCode

Add this to `opencode.json` in your project, or to your global OpenCode config:

```json
{
  "$schema": "opencode.ai/config.json",
  "mcp": {
    "servers": {
      "jarvas": {
        "type": "local",
        "command": ["C:/Users/YOU/Downloads/jarvas-server.exe"],
        "environment": {
          "JARVAS_ACTION_DELAY_MS": "150"
        }
      }
    }
  }
}
```

If your OpenCode version uses the older flat form, put the same `jarvas` object directly under `mcp` instead of `mcp.servers`.

Complete example: [`docs/opencode.json.example`](docs/opencode.json.example).

### Claude Desktop, Cursor, Cline, Windsurf, and similar clients

Use the standard `mcpServers` format:

```json
{
  "mcpServers": {
    "jarvas": {
      "command": "C:/Users/YOU/Downloads/jarvas-server.exe",
      "args": [],
      "env": {
        "JARVAS_ACTION_DELAY_MS": "150"
      }
    }
  }
}
```

Complete example: [`docs/mcp-servers.json.example`](docs/mcp-servers.json.example).

After adding the config, fully restart the AI app. In OpenCode you can check with `/mcps`; in other clients look for `jarvas` in the MCP/tools panel.

## Safety

- **Emergency stop:** press `Ctrl+Shift+J` anywhere to block all further JARVAS actions until the server is restarted.
- **Destructive actions:** `close_window` and `close_app` require `confirm=true`.
- **Action delay:** `JARVAS_ACTION_DELAY_MS` defaults to `150` ms between actions. Increase it if you want slower, more careful control.
- **Action log:** actions go to stderr, to `jarvas.log` beside the executable, and to the `get_action_log` tool.
- **Local only:** JARVAS does not listen on a network port. The AI client launches it as a local process.

## Build from source

Requires Windows 10/11 x64 and a Rust toolchain.

```powershell
cd jarvas-server
cargo build --release
```

Verified with:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --release
```

All three pass with zero warnings.

## Test the server

List tools:

```powershell
npx -y @modelcontextprotocol/inspector --cli .\target\release\jarvas-server.exe --method tools/list
```

Call a tool:

```powershell
npx -y @modelcontextprotocol/inspector --cli .\target\release\jarvas-server.exe --method tools/call --tool-name get_cursor_position
```

Pass each argument as its own `--tool-arg key=value` pair:

```powershell
npx -y @modelcontextprotocol/inspector --cli .\target\release\jarvas-server.exe --method tools/call --tool-name move_mouse --tool-arg x=600 --tool-arg y=300
```

## Example prompts

- "Use jarvas to take a screenshot."
- "List visible windows and focus Notepad."
- "Open Notepad and type `JARVAS is ready`."
- "Find the Save button and click it."
- "Get the screen size, then click the center of the screen."
- "Close Calculator with confirm=true."

## FAQ

### Where is the app?

The downloadable release is attached to [GitHub Releases](https://github.com/Superkevin828/jarvas_remote/releases/latest). The source lives in [`jarvas-server/`](jarvas-server/).

### Does it work on any Windows PC?

It should work on 64-bit Windows 10/11 with an interactive desktop session. Copy the `.exe` to the other PC and update the path in your AI client config. It has not been tested on Windows 7 or 32-bit Windows.

### Does it need internet?

No, not to run. The first source build needs internet to download Rust crates; after that, `cargo build --release --offline` works.

### Can ChatGPT or a browser chatbot use it?

Only if that client supports local MCP servers. Plain browser chat interfaces do not.

### What if the AI does something wrong?

Press `Ctrl+Shift+J` immediately, then restart the AI client or MCP server.

## Last chat

The recent setup/usage conversation is preserved in [`docs/CHAT_HISTORY.md`](docs/CHAT_HISTORY.md).
