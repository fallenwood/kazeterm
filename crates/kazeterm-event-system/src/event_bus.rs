use std::collections::HashMap;

use gpui::{Context, Window};

use crate::AppEvent;

type EventHandler<T> = Box<dyn Fn(&mut T, AppEvent, &mut Window, &mut Context<T>) + Send + 'static>;

/// Centralized event bus that dispatches [`AppEvent`]s to registered subscribers.
///
/// Subscribers register handlers keyed by event discriminant. When an event is
/// dispatched, all handlers registered for that discriminant are invoked in
/// registration order.
pub struct EventBus<T> {
  handlers: HashMap<&'static str, Vec<EventHandler<T>>>,
}

impl<T> Default for EventBus<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> EventBus<T> {
  pub fn new() -> Self {
    Self {
      handlers: HashMap::new(),
    }
  }

  /// Register a handler for a specific event discriminant.
  ///
  /// Multiple handlers can be registered for the same discriminant; they will
  /// all be called in registration order when a matching event is dispatched.
  pub fn subscribe<F>(&mut self, discriminant: &'static str, handler: F)
  where
    F: Fn(&mut T, AppEvent, &mut Window, &mut Context<T>) + Send + 'static,
  {
    self
      .handlers
      .entry(discriminant)
      .or_default()
      .push(Box::new(handler));
  }

  /// Dispatch an event to all registered handlers for that event's discriminant.
  ///
  /// Returns the number of handlers that were invoked.
  pub fn dispatch(
    &self,
    target: &mut T,
    event: AppEvent,
    window: &mut Window,
    cx: &mut Context<T>,
  ) -> usize {
    let discriminant = event.discriminant();
    if let Some(handlers) = self.handlers.get(discriminant) {
      for handler in handlers {
        handler(target, event.clone(), window, cx);
      }
      handlers.len()
    } else {
      tracing::debug!("No handlers registered for event: {}", discriminant);
      0
    }
  }

  /// Returns the number of handlers registered for a discriminant.
  pub fn handler_count(&self, discriminant: &str) -> usize {
    self.handlers.get(discriminant).map_or(0, Vec::len)
  }
}

#[cfg(test)]
mod tests {
  use super::EventBus;

  struct TestState;

  #[test]
  fn event_bus_subscribe_count() {
    let mut bus = EventBus::<TestState>::new();
    assert_eq!(bus.handler_count("NextTab"), 0);

    bus.subscribe("NextTab", |_state, _event, _window, _cx| {});
    assert_eq!(bus.handler_count("NextTab"), 1);

    bus.subscribe("NextTab", |_state, _event, _window, _cx| {});
    assert_eq!(bus.handler_count("NextTab"), 2);
  }
}
