# crates/kazeterm-event-system/

## Responsibility

Provides typed application-event transport from arbitrary threads and newline-delimited external JSON sources into handlers that mutate a target GPUI entity on the main thread.

## Design

- `AppEvent` is the internal command vocabulary; `JsonEvent` is its deserializable external representation.
- `EventBus<T>` implements Publisher/Subscriber dispatch using event discriminants and ordered handler lists.
- A process-wide `OnceLock<Sender<AppEvent>>` is the single global event ingress used by blocking and non-blocking send helpers.
- Source adapters run stdin or Unix-domain-socket readers on background threads and normalize JSON lines into `AppEvent`s.
- The async dispatch loop upgrades a weak GPUI entity and enters its window/context before invoking subscribers, preserving the UI-thread boundary.

## Flow

1. The application builds an `EventBus<T>` and calls `start_event_system` with the target entity, window, and `EventSourceConfig`.
2. Programmatic callers use `send_event`/`try_send_event`; optional stdin/socket readers deserialize one JSON event per line into the same channel.
3. A detached GPUI task receives events in order.
4. `dispatch_event` updates the target window and entity, then `EventBus::dispatch` invokes all handlers registered for the event discriminant.
5. Dropped targets, closed channels, malformed external input, and source failures are logged rather than crossing the UI boundary unchecked.

## Integration

- Depends on: GPUI for entity/window dispatch, `smol` for channels, and Serde for external JSON.
- Consumed by: `kazeterm::event_system`, which registers application-specific handlers and selects CLI-configured event sources.
- Entry points: `src/lib.rs` for lifecycle/global send APIs, `event_bus.rs` for subscriptions, and `event_sources.rs`/`json_event.rs` for external ingress.
