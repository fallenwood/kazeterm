use std::{collections::HashMap, time::Instant};

use terminal::program_status::{ProgramState, ProgramStatusRecord};

use super::NotificationReason;

/// One terminal's event history. It follows the entity, not its window or tab index.
#[derive(Default)]
pub(super) struct NotificationState {
  input: Option<Instant>,
  revisions: HashMap<String, u64>,
  structured_cycle: bool,
  fallback_claimed: bool,
  fallback_delivered: bool,
  pending: Option<NotificationReason>,
}

impl NotificationState {
  pub(super) fn observe(
    &mut self,
    input: Instant,
    records: &[ProgramStatusRecord],
  ) -> Vec<ProgramStatusRecord> {
    if self.input != Some(input) {
      self.input = Some(input);
      self.structured_cycle = false;
      self.fallback_claimed = false;
      self.fallback_delivered = false;
      self.pending = None;
    }
    self
      .revisions
      .retain(|id, _| records.iter().any(|record| &record.id == id));
    let mut changed = Vec::new();
    for record in records {
      // A still-active structured task remains authoritative after user input.
      if matches!(record.state, ProgramState::Working | ProgramState::Blocked) {
        self.structured_cycle = true;
        self.pending = None;
      }
      if self
        .revisions
        .insert(record.id.clone(), record.state_revision)
        == Some(record.state_revision)
      {
        continue;
      }
      if matches!(record.state, ProgramState::Working | ProgramState::Idle) {
        self.fallback_delivered = false;
      }
      if record.state != ProgramState::Idle {
        self.structured_cycle = true;
        self.pending = None;
      }
      if matches!(
        record.state,
        ProgramState::Done | ProgramState::Blocked | ProgramState::Error
      ) {
        changed.push(record.clone());
      }
    }
    changed.reverse();
    changed.sort_by_key(|record| std::cmp::Reverse(record.state.priority()));
    // Coalesce one late structured event with a fallback already delivered for this cycle.
    if !changed.is_empty() && self.fallback_delivered {
      self.fallback_delivered = false;
      changed.remove(0);
    }
    changed
  }

  pub(super) fn queue(&mut self, reason: NotificationReason) -> bool {
    if self.structured_cycle
      || self.fallback_claimed
      || matches!(reason, NotificationReason::ProgramStatus(_))
    {
      return false;
    }
    // Prompt return is more specific than the supplementary Bell signal.
    if self.pending.is_none()
      || matches!(
        (&self.pending, &reason),
        (
          Some(NotificationReason::Bell),
          NotificationReason::CommandFinished
        )
      )
    {
      self.pending = Some(reason);
      true
    } else {
      false
    }
  }

  pub(super) fn mark_fallback_delivered(&mut self) {
    self.fallback_delivered = true;
  }

  pub(super) fn pending(&self) -> Option<&NotificationReason> {
    self.pending.as_ref()
  }

  pub(super) fn take_fallback(&mut self, input: Instant) -> Option<NotificationReason> {
    if self.input != Some(input) {
      self.pending = None;
      return None;
    }
    let reason = self.pending.take()?;
    self.fallback_claimed = true;
    Some(reason)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn record(id: &str, state: ProgramState, revision: u64) -> ProgramStatusRecord {
    ProgramStatusRecord {
      id: id.into(),
      state,
      state_revision: revision,
      kind: None,
      progress: None,
      app: None,
      title: None,
      msg: None,
    }
  }

  #[test]
  fn osc_notifications_are_once_per_state_transition_not_metadata_refresh() {
    let mut state = NotificationState::default();
    let input = Instant::now();
    let mut done = record("build", ProgramState::Done, 1);
    assert_eq!(state.observe(input, &[done.clone()]).len(), 1);
    done.msg = Some("new details".into());
    done.progress = Some(100);
    assert!(state.observe(input, &[done.clone()]).is_empty());
    assert!(!state.queue(NotificationReason::Bell));
    assert!(!state.queue(NotificationReason::CommandFinished));
    done.state_revision = 3; // working -> done may occur between UI snapshots.
    assert_eq!(state.observe(input, &[done.clone()]).len(), 1);
    state.observe(input, &[]);
    done.state_revision = 5;
    assert_eq!(state.observe(input, &[done]).len(), 1);
  }

  #[test]
  fn fallback_signals_coalesce_with_each_other_and_with_osc_in_both_orders() {
    let input = Instant::now();
    let done = record("", ProgramState::Done, 1);
    let mut state = NotificationState::default();
    state.observe(input, &[]);
    assert!(state.queue(NotificationReason::Bell));
    assert!(state.queue(NotificationReason::CommandFinished));
    assert!(!state.queue(NotificationReason::Bell));
    assert_eq!(state.observe(input, &[done.clone()]).len(), 1);
    assert!(state.take_fallback(input).is_none());
    assert!(!state.queue(NotificationReason::Bell));

    let mut fallback_first = NotificationState::default();
    fallback_first.observe(input, &[]);
    fallback_first.queue(NotificationReason::Bell);
    assert!(fallback_first.take_fallback(input).is_some());
    fallback_first.mark_fallback_delivered();
    assert!(!fallback_first.queue(NotificationReason::CommandFinished));
    assert!(fallback_first.observe(input, &[done.clone()]).is_empty());
    assert!(fallback_first.observe(input, &[done]).is_empty());
    // A new input cycle allows a plain command, even if an old done record survives.
    let next = input + std::time::Duration::from_secs(1);
    fallback_first.observe(next, &[record("", ProgramState::Done, 1)]);
    assert!(fallback_first.queue(NotificationReason::CommandFinished));
    assert!(fallback_first.take_fallback(next).is_some());
  }

  #[test]
  fn independent_tasks_and_attention_states_remain_distinct_and_new_input_cancels_pending() {
    let input = Instant::now();
    let mut state = NotificationState::default();
    assert!(
      state
        .observe(input, &[record("a", ProgramState::Working, 1)])
        .is_empty()
    );
    let input = input + std::time::Duration::from_secs(1);
    assert!(
      state
        .observe(input, &[record("a", ProgramState::Working, 1)])
        .is_empty()
    );
    assert!(!state.queue(NotificationReason::Bell));
    assert_eq!(
      state
        .observe(
          input,
          &[
            record("a", ProgramState::Blocked, 2),
            record("b", ProgramState::Error, 3)
          ]
        )
        .len(),
      2
    );
    assert_eq!(
      state
        .observe(
          input,
          &[
            record("a", ProgramState::Done, 4),
            record("b", ProgramState::Error, 3)
          ]
        )
        .len(),
      1
    );
    let next = input + std::time::Duration::from_secs(1);
    state.observe(next, &[]);
    state.queue(NotificationReason::Bell);
    assert!(
      state
        .take_fallback(next + std::time::Duration::from_secs(1))
        .is_none()
    );
  }
}
