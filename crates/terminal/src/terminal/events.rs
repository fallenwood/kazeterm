use alacritty_terminal::event::{Event as AlacTermEvent, EventListener};
use futures::channel::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct TerminalEventListener(pub UnboundedSender<AlacTermEvent>);

impl EventListener for TerminalEventListener {
  fn send_event(&self, event: AlacTermEvent) {
    self.0.unbounded_send(event).ok();
  }
}
