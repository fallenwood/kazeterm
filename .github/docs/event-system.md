# Event System

Kazeterm includes an optional event system that allows triggering UI actions from external sources. The event system can be configured via command-line arguments to read events from:

- **stdio**: Read JSON events from stdin (useful for piping commands)
- **socket**: Read JSON events from a Unix domain socket (all platforms)

By default, the event system is disabled but events can still be sent programmatically from within the application.

## Command-Line Usage

```bash
# Normal startup (event system disabled, no external event source)
kazeterm

# Enable event system reading from stdin
kazeterm --event-source stdio

# Enable event system reading from a Unix domain socket (all platforms)
kazeterm --event-source socket --event-socket /tmp/kazeterm.sock

# On Windows, use a file path for Unix domain socket
kazeterm --event-source socket --event-socket C:\Users\user\kazeterm.sock
```

## Event Format (JSON)

Events are sent as JSON objects, one per line. The `event` field specifies the event type using a tagged enum format:

```json
{"event": "NewTerminalWithDefaultProfile"}
{"event": "NewTerminalWithProfile", "profile_name": "bash", "working_directory": "/home"}
{"event": "SendTextToTerminal", "text": "echo hello\r"}
{"event": "SwitchToTab", "position": 0}
{"event": "NextTab"}
{"event": "PreviousTab"}
{"event": "CloseActiveTab"}
{"event": "SplitHorizontal"}
{"event": "SplitVertical"}
{"event": "ToggleSearch"}
{"event": "ReloadConfig"}
{"event": "Custom", "name": "my.event", "data": "some data"}
```

## Available Events

| Event | Description |
|-------|-------------|
| `NewTerminalWithDefaultProfile` | Create a new terminal tab with the default profile |
| `NewTerminalWithProfile { profile_name, working_directory }` | Create a terminal with a specific profile |
| `CloseActiveTab` | Close the currently active tab |
| `CloseTab { tab_index }` | Close a specific tab by index |
| `NextTab` | Switch to the next tab |
| `PreviousTab` | Switch to the previous tab |
| `SwitchToTab { position }` | Switch to a tab by position (0-indexed) |
| `SplitHorizontal` | Split the active pane horizontally |
| `SplitVertical` | Split the active pane vertically |
| `CloseActivePane` | Close the active pane within a split |
| `ToggleSearch` | Toggle the search bar visibility |
| `ShowAboutDialog` | Show the about dialog |
| `ReloadConfig` | Reload configuration and themes |
| `FocusActiveTerminal` | Focus the active terminal |
| `SendTextToTerminal { text }` | Send text to the active terminal |
| `Custom { name, data }` | Custom event for extensions |

## Usage Examples

### From within the application

```rust
use crate::event_system::{AppEvent, send_event};

// Create a new terminal with default profile
send_event(AppEvent::NewTerminalWithDefaultProfile);

// Create a terminal with a specific profile
send_event(AppEvent::NewTerminalWithProfile {
    profile_name: "PowerShell".to_string(),
    working_directory: Some("C:\\Users\\me\\projects".to_string()),
});

// Send text to the active terminal
send_event(AppEvent::SendTextToTerminal {
    text: "echo Hello, World!\r".to_string(),
});
```

### From a background thread

```rust
use std::thread;
use crate::event_system::{AppEvent, send_event};

// Spawn a background thread
thread::spawn(|| {
    // Wait for some condition...
    std::thread::sleep(std::time::Duration::from_secs(5));

    // Send an event from the background thread
    send_event(AppEvent::NewTerminalWithDefaultProfile);
});
```

### Non-blocking send

```rust
use crate::event_system::{AppEvent, try_send_event};

// Try to send without blocking (useful in async contexts)
if try_send_event(AppEvent::NextTab) {
    println!("Event sent successfully");
} else {
    println!("Event system not initialized or channel full");
}
```

## Extending with Custom Events

The `Custom` event type allows for extensibility:

```rust
send_event(AppEvent::Custom {
    name: "my_extension.action".to_string(),
    data: r#"{"key": "value"}"#.to_string(),
});
```

Custom events are logged and can be handled by extending the `handle_event` function in `event_system.rs`.

## External Event Sources

### Sending Events via Stdin

Start Kazeterm with stdin event source:

```bash
kazeterm --event-source stdio
```

Then pipe JSON events to it:

```bash
echo '{"event": "NewTerminalWithDefaultProfile"}' | kazeterm --event-source stdio
```

Or use a FIFO/named pipe for continuous event streaming:

```bash
# Create a FIFO (Linux/macOS)
mkfifo /tmp/kazeterm-events

# Start Kazeterm reading from the FIFO
cat /tmp/kazeterm-events | kazeterm --event-source stdio &

# Send events
echo '{"event": "NewTerminalWithDefaultProfile"}' > /tmp/kazeterm-events
echo '{"event": "SendTextToTerminal", "text": "ls -la\r"}' > /tmp/kazeterm-events
```

### Sending Events via Unix Socket (Linux/macOS)

Start Kazeterm with socket event source:

```bash
kazeterm --event-source socket --event-socket /tmp/kazeterm.sock
```

Then connect and send events:

```bash
# Using netcat
echo '{"event": "NewTerminalWithDefaultProfile"}' | nc -U /tmp/kazeterm.sock

# Using socat
echo '{"event": "NextTab"}' | socat - UNIX-CONNECT:/tmp/kazeterm.sock
```

### Sending Events via Unix Socket (Windows)

Windows 10 version 1803 and later support Unix domain sockets natively.

Start Kazeterm with socket event source:

```powershell
kazeterm --event-source socket --event-socket C:\Users\user\kazeterm.sock
```

Then connect using a Unix socket client (e.g., via Python, Node.js, or other languages with Unix socket support on Windows).

## Architecture

The event system uses:
- A global `OnceLock<Sender<AppEvent>>` for thread-safe event sending
- An async event loop running on GPUI's executor
- `smol::channel` for efficient async communication
- Optional external readers for stdin or socket/pipe input

Events are processed sequentially to maintain UI consistency.
