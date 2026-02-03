# Event System

Kazeterm includes a flexible event system that allows triggering UI actions from any thread, including background threads. This is useful for:

- Automation and scripting
- Inter-process communication (IPC)
- Plugin systems
- Remote control via sockets

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
    text: "echo Hello, World!\n".to_string(),
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

## Architecture

The event system uses:
- A global `OnceLock<Sender<AppEvent>>` for thread-safe event sending
- An async event loop running on GPUI's executor
- `smol::channel` for efficient async communication

Events are processed sequentially to maintain UI consistency.
