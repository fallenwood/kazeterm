use futures::channel::mpsc::UnboundedSender;
use terminal_kernel::event::{Event as AlacTermEvent, EventListener};

#[derive(Clone)]
pub struct TerminalEventListener(pub UnboundedSender<AlacTermEvent>);

impl EventListener for TerminalEventListener {
  fn send_event(&self, event: AlacTermEvent) {
    self.0.unbounded_send(event).ok();
  }
}
