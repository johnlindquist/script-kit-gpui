# Development Guide for script-kit-gpui

This guide explains how to set up and use the development environment with hot-reload capabilities.

## Prerequisites

### Required

- **Rust** (1.70+) – Install from https://rustup.rs/
- **cargo-watch** – Auto-recompiler tool for Rust projects

  ```bash
  cargo install cargo-watch
  ```

### Optional but Recommended

- A terminal with good color support for clear output
- Text editor with Rust support (VS Code, Neovim, etc.)

## Running the Dev Server

Start the development runner with automatic rebuilds:

```bash
./dev.sh
```

Or if you prefer bash explicitly:

```bash
bash dev.sh
```

The script will:
1. Check if `cargo-watch` is installed (offering installation instructions if not)
2. Start the Rust compiler with `cargo watch -c -x run`
3. Clear the screen between rebuilds for clean output
4. Automatically rebuild and restart the app whenever you save a `.rs` file

Press **Ctrl+C** to stop the development runner.

## Hot Reload Workflow

This project supports multiple hot-reload mechanisms for a smooth development experience:

### 1. **Rust Code Changes** (via cargo-watch)
- Editing any `.rs` file triggers `cargo watch` to rebuild and restart the application
- The app instantly reflects your code changes
- No manual restart needed

### 2. **Theme Changes** (via ~/.kit/theme.json)
The app automatically watches `~/.kit/theme.json` for changes:
- Modify colors, fonts, or any theme settings in this file
- The UI refreshes in real-time without restarting the app
- See the "Theme Configuration" section below for details

### 3. **Script List Changes** (via ~/.kenv/scripts)
The app automatically detects new or modified scripts:
- Add a new script to `~/.kenv/scripts/`
- Remove or rename an existing script
- The app refreshes the script list without restarting
- Changes appear in the UI immediately

## Theme Configuration

To set up hot-reload for UI themes:

### First Time Setup

1. Create the Kit home directory:
   ```bash
   mkdir -p ~/.kit
   ```

2. Create or edit `~/.kit/theme.json`:
   ```json
   {
     "background": "#1e1e1e",
     "foreground": "#e0e0e0",
     "accent": "#007acc",
     "border": "#464647"
   }
   ```

3. Run the dev server - it will automatically watch this file for changes

### Editing Your Theme

Edit `~/.kit/theme.json` in your text editor while the dev server runs. Changes appear instantly in the UI without restarting.

## Best Practices for Development

### Terminal Setup

- Use a terminal with **256-color support** for the best visual experience
- **Full-screen terminal** recommended for viewing logs and output
- **Clear scrollback** between dev sessions for cleaner logs

### Workflow Tips

1. **Keep the log panel open** – Use `Cmd+L` in the app to toggle the logs panel
   - Shows real-time events: hotkey presses, script executions, filter changes
   - Helpful for debugging configuration issues

2. **Test scripts incrementally**
   - Create test scripts in `~/.kenv/scripts/`
   - Run them through the UI to verify behavior
   - Check logs for execution details

3. **Hotkey testing**
   - Configure your hotkey in `~/.kit/config.json`
   - Press the configured hotkey to toggle the app visibility
   - Logs will show when the hotkey is detected and processed

4. **Use filtering** – Type to search scripts
   - Helps verify the filtering logic is working correctly
   - Type to add characters, Backspace to remove, Escape to clear

### Common Development Tasks

#### Test a Single File Change
```bash
# Dev server is already running with cargo-watch
# Just save your file and wait ~2-5 seconds for recompile
```

#### Check the Build Log
```bash
# Look at the cargo-watch output in your terminal
# It shows compilation errors, warnings, and execution output
```

#### Revert a Change
```bash
# Stop dev server: Ctrl+C
# Run: git checkout path/to/file.rs
# Start dev server again: ./dev.sh
```

#### Clean Build
```bash
# Stop dev server: Ctrl+C
# Run: cargo clean
# Start dev server again: ./dev.sh
# (This will recompile everything from scratch)
```

## Troubleshooting

### Script crashes immediately after startup
- Check the terminal output for panic messages
- Look at the logs panel (Cmd+L) for detailed events
- Verify Rust dependencies are correct: `cargo build`

### cargo-watch not detecting changes
- Ensure files are being saved to disk (check modification timestamps)
- Stop and restart the dev server: Ctrl+C, then `./dev.sh`
- Try `cargo clean && ./dev.sh` for a full rebuild

### Hotkey not registering
- Check the logs panel (Cmd+L) for hotkey registration messages
- Verify your hotkey config in `~/.kit/config.json` is valid
- Some system shortcuts may conflict - try a different key combination

### Theme changes not appearing
- Verify `~/.kit/theme.json` exists and is valid JSON
- Check the logs for file watcher errors
- Restart the dev server if hot-reload doesn't trigger

## Architecture Overview

The dev experience is built on several components:

- **cargo-watch** – Detects Rust source changes → triggers rebuild/restart
- **notify crate** – File system watcher for config and script changes
- **GPUI** – The UI framework with reactive rendering
- **Global hotkey listener** – Background thread detecting system hotkey press

These work together to provide instant feedback on:
1. Code changes (cargo-watch)
2. Configuration/theme changes (notify)
3. New/modified scripts (notify + file watcher)
4. Hotkey presses (global-hotkey thread)

## Next Steps

1. ✅ Install `cargo-watch`: `cargo install cargo-watch`
2. ✅ Start dev server: `./dev.sh`
3. ✅ Create a test script in `~/.kenv/scripts/`
4. ✅ Configure hotkey in `~/.kit/config.json`
5. ✅ Use `Cmd+L` to view logs while developing

Happy hacking! 🚀
