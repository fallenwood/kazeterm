use std::{
  collections::HashMap,
  time::{Duration, Instant},
};

use gpui::{App, Context, Entity, EntityId, Global, Task, WeakEntity};
use terminal::TerminalView;

use super::state::NotificationState;
use super::{MainWindow, NotificationContent, NotificationReason, notification_is_due};

/// Give structured OSC reports a chance to replace supplementary legacy signals.
const COALESCE_DELAY: Duration = Duration::from_millis(200);

struct TerminalNotifications {
  view: WeakEntity<TerminalView>,
  owner: WeakEntity<MainWindow>,
  state: NotificationState,
  pending: Option<NotificationContent>,
  task: Task<()>,
}

struct NotificationCenter {
  terminals: HashMap<EntityId, TerminalNotifications>,
  last_sent: Option<Instant>,
  #[cfg(test)]
  captured: Option<Vec<NotificationContent>>,
}

impl Default for NotificationCenter {
  fn default() -> Self {
    Self {
      terminals: HashMap::new(),
      last_sent: None,
      #[cfg(test)]
      captured: Some(Vec::new()),
    }
  }
}
impl Global for NotificationCenter {}

impl MainWindow {
  pub(crate) fn rebind_notification_owner(view: &Entity<TerminalView>, cx: &mut Context<Self>) {
    if cx.has_global::<NotificationCenter>() {
      let owner = cx.entity().downgrade();
      if let Some(entry) = cx
        .default_global::<NotificationCenter>()
        .terminals
        .get_mut(&view.entity_id())
      {
        entry.owner = owner;
      }
    }
  }

  pub(crate) fn maybe_send_program_notifications(
    &mut self,
    view: &Entity<TerminalView>,
    cx: &mut Context<Self>,
  ) {
    self.update_notifications(view, None, cx);
  }

  pub(crate) fn maybe_send_notification(
    &mut self,
    view: &Entity<TerminalView>,
    reason: NotificationReason,
    cx: &mut Context<Self>,
  ) {
    self.update_notifications(view, Some(reason), cx);
  }

  fn update_notifications(
    &mut self,
    view: &Entity<TerminalView>,
    reason: Option<NotificationReason>,
    cx: &mut Context<Self>,
  ) {
    let (input, records, current, previous) = {
      let terminal = view.read(cx).terminal().read(cx);
      (
        terminal.last_input_time,
        terminal.program_status_records().to_vec(),
        terminal.title_text.clone(),
        terminal.previous_title_text.clone(),
      )
    };
    let tab = self
      .items
      .iter()
      .find(|item| {
        item
          .split_container
          .all_terminals()
          .iter()
          .any(|(_, terminal)| terminal.entity_id() == view.entity_id())
      })
      .map(|item| item.display_title().to_string());
    if tab.is_none() {
      return; // Ignore an event queued on a window that no longer owns this terminal.
    }
    let config = cx.global::<config::Config>().notification.clone();
    let id = view.entity_id();
    let owner = cx.entity().downgrade();
    let (ready, schedule) = {
      let center = cx.default_global::<NotificationCenter>();
      center
        .terminals
        .retain(|_, entry| entry.view.upgrade().is_some());
      let allowed = notification_is_due(
        &config,
        Some(input.elapsed()),
        center.last_sent.map(|last| last.elapsed()),
      );
      let entry = center
        .terminals
        .entry(id)
        .or_insert_with(|| TerminalNotifications {
          view: view.downgrade(),
          owner: owner.clone(),
          state: NotificationState::default(),
          pending: None,
          task: Task::ready(()),
        });
      entry.owner = owner;
      let ready = entry.state.observe(input, &records);
      if entry.state.pending().is_none() {
        entry.pending = None;
        entry.task = Task::ready(());
      }
      let mut schedule = false;
      if let Some(reason) = reason {
        if entry.state.queue(reason) {
          if allowed {
            schedule = entry.pending.is_none();
            entry.pending = Some(entry.state.pending().unwrap().clone().content(
              tab.as_deref(),
              &current,
              &previous,
            ));
          } else {
            entry.state.take_fallback(input);
            entry.pending = None;
            entry.task = Task::ready(());
          }
        }
      }
      (ready, schedule)
    };
    for record in ready {
      let content =
        NotificationReason::ProgramStatus(record).content(tab.as_deref(), &current, &previous);
      // Explicit structured status is not a heuristic long-running-command signal.
      Self::deliver_notification(content, None, cx);
    }
    if schedule {
      let task = App::spawn(cx, async move |cx| {
        cx.background_executor().timer(COALESCE_DELAY).await;
        cx.update(|cx| Self::flush_notification(id, cx));
      });
      cx.default_global::<NotificationCenter>()
        .terminals
        .get_mut(&id)
        .unwrap()
        .task = task;
    }
  }

  fn flush_notification(id: EntityId, cx: &mut App) {
    let (view, owner) = cx
      .default_global::<NotificationCenter>()
      .terminals
      .get(&id)
      .map(|entry| (entry.view.upgrade(), entry.owner.upgrade()))
      .unwrap_or_default();
    let owned = owner.is_some_and(|owner| {
      owner.read(cx).items.iter().any(|item| {
        item
          .split_container
          .all_terminals()
          .iter()
          .any(|(_, view)| view.entity_id() == id)
      })
    });
    let view = view.filter(|_| owned);
    let Some(view) = view else {
      cx.default_global::<NotificationCenter>()
        .terminals
        .remove(&id);
      return;
    };
    let input = view.read(cx).terminal().read(cx).last_input_time;
    let entry = cx
      .default_global::<NotificationCenter>()
      .terminals
      .get_mut(&id)
      .unwrap();
    let reason = entry.state.take_fallback(input);
    let content = entry.pending.take();
    if reason.is_some()
      && let Some(content) = content
    {
      if Self::deliver_notification(content, Some(input.elapsed()), cx) {
        cx.default_global::<NotificationCenter>()
          .terminals
          .get_mut(&id)
          .unwrap()
          .state
          .mark_fallback_delivered();
      }
    }
  }

  fn deliver_notification(
    content: NotificationContent,
    idle: Option<Duration>,
    cx: &mut App,
  ) -> bool {
    let config = cx.global::<config::Config>().notification.clone();
    let center = cx.default_global::<NotificationCenter>();
    if !notification_is_due(&config, idle, center.last_sent.map(|last| last.elapsed())) {
      return false;
    }
    center.last_sent = Some(Instant::now());
    #[cfg(test)]
    if let Some(captured) = &mut center.captured {
      captured.push(content);
      return true;
    }
    Self::send_notification(content);
    true
  }
}

#[cfg(test)]
mod tests;
