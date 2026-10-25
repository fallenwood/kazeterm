//! Applying validated static Kitty graphics commands to terminal-owned resources.

use super::Terminal;
use crate::kitty_graphics::{
  ImagePlacement, KittyAction, KittyCommand, KittyDelete, KittyResponseContext, PreparedImage,
  placement::placement_contains,
};

impl Terminal {
  pub(super) fn execute_graphics_command(
    &mut self,
    cmd: &KittyCommand,
    prepared_image: Option<PreparedImage>,
    cursor_line: i32,
    cursor_column: i32,
  ) {
    let context = KittyResponseContext::from(cmd);
    let result = match cmd.action {
      KittyAction::Transmit | KittyAction::TransmitAndDisplay => {
        let Some(image) = prepared_image else {
          return;
        };
        match self
          .image_storage
          .store_prepared(cmd.image_id, cmd.image_number, image)
        {
          Ok(id) => {
            // Retransmitting an ID invalidates all old placements. Evicted
            // images must not leave placements that can reappear on ID reuse.
            self.placement_manager.remove_by_image(id);
            self.placement_manager.gc(&self.image_storage);
            if cmd.action == KittyAction::TransmitAndDisplay {
              self.place_image(id, cmd, cursor_line, cursor_column);
            }
            Ok(id)
          }
          Err(error) => {
            tracing::warn!("Failed to store Kitty image: {error}");
            Err(format!("ENOSPC:{error}"))
          }
        }
      }
      KittyAction::Display => {
        let id = if cmd.image_number != 0 {
          self.image_storage.newest_for_number(cmd.image_number)
        } else {
          (cmd.image_id != 0).then_some(cmd.image_id)
        };
        if let Some(id) = id
          && self.image_storage.get(id).is_some()
        {
          self.place_image(id, cmd, cursor_line, cursor_column);
          Ok(id)
        } else {
          Err("ENOENT:Image not found".into())
        }
      }
      KittyAction::Delete => {
        self.handle_delete(cmd, cursor_line, cursor_column);
        return; // Kitty deletion commands do not produce acknowledgements.
      }
      KittyAction::Query => return, // Validated and replied to by the decoder worker.
    };
    if let Some(response) = context.respond(result) {
      self.write_to_pty(response.encode());
    }
  }

  fn place_image(&mut self, image_id: u32, cmd: &KittyCommand, line: i32, column: i32) {
    let Some(image) = self.image_storage.peek(image_id) else {
      return;
    };
    let bounds = &self.last_content.terminal_bounds;
    let mut placement = ImagePlacement {
      image_id,
      // Anonymous images cannot have a client-addressable placement ID.
      placement_id: if cmd.image_id == 0 && cmd.image_number == 0 {
        0
      } else {
        cmd.placement_id
      },
      line,
      column,
      display_columns: cmd.display_columns,
      display_rows: cmd.display_rows,
      width_cells: 0,
      height_cells: 0,
      crop: (cmd.crop_x, cmd.crop_y, cmd.crop_width, cmd.crop_height),
      z_index: cmd.z_index,
      x_offset: cmd.x_offset,
      y_offset: cmd.y_offset,
    };
    placement.update_cell_coverage(
      (image.width, image.height),
      (
        f32::from(bounds.cell_width()),
        f32::from(bounds.line_height()),
      ),
    );
    let height_cells = placement.height_cells;
    self.placement_manager.add(placement);

    if cmd.cursor_movement == 0
      && let Some(cnl) = &self.pending_cnl
    {
      cnl.store(height_cells, std::sync::atomic::Ordering::Release);
    }
  }

  fn handle_delete(&mut self, cmd: &KittyCommand, cursor_line: i32, cursor_column: i32) {
    let delete = cmd.delete.as_ref().cloned().unwrap_or(KittyDelete::All);
    let screen_top = self.term.history_size() as i64;
    let screen_bottom = screen_top + self.term.screen_lines() as i64;
    let numbered_id = match delete {
      KittyDelete::ByNumber { image_number, .. } => {
        self.image_storage.newest_for_number(image_number)
      }
      _ => None,
    };
    let mut affected = self.placement_manager.remove_matching(|p| match &delete {
      KittyDelete::All => {
        i64::from(p.line) < screen_bottom
          && i64::from(p.line) + i64::from(p.height_cells) > screen_top
      }
      KittyDelete::ById {
        image_id,
        placement_id,
      } => p.image_id == *image_id && placement_id.is_none_or(|id| p.placement_id == id),
      KittyDelete::ByNumber { placement_id, .. } => {
        Some(p.image_id) == numbered_id && placement_id.is_none_or(|id| p.placement_id == id)
      }
      KittyDelete::IdRange { first, last } => p.image_id >= *first && p.image_id <= *last,
      KittyDelete::AtCursor => {
        placement_contains(p, i64::from(cursor_line), i64::from(cursor_column))
      }
      KittyDelete::AtCell { x, y, z_index } => {
        *x > 0
          && *y > 0
          && placement_contains(p, screen_top + i64::from(*y) - 1, i64::from(*x) - 1)
          && z_index.is_none_or(|z| p.z_index == z)
      }
      KittyDelete::AtColumn(x) => {
        *x > 0
          && i64::from(p.column) < i64::from(*x)
          && i64::from(*x) - 1 < i64::from(p.column) + i64::from(p.width_cells)
      }
      KittyDelete::AtRow(y) => {
        *y > 0
          && i64::from(p.line) < screen_top + i64::from(*y)
          && screen_top + i64::from(*y) - 1 < i64::from(p.line) + i64::from(p.height_cells)
      }
      KittyDelete::ByZIndex(z) => p.z_index == *z,
      KittyDelete::AnimationFrames => false,
    });
    if cmd.delete_data {
      // ID/number/range deletion also frees matching unplaced images. Other
      // selectors only free images whose placements they actually removed.
      match delete {
        KittyDelete::ById {
          image_id,
          placement_id: None,
        } => affected.push(image_id),
        KittyDelete::ByNumber {
          placement_id: None, ..
        } => affected.extend(numbered_id),
        KittyDelete::IdRange { first, last } => affected.extend(
          self
            .image_storage
            .image_ids()
            .filter(|id| *id >= first && *id <= last),
        ),
        _ => {}
      }
      affected.sort_unstable();
      affected.dedup();
      for id in affected {
        if !self.placement_manager.has_image(id) {
          self.image_storage.remove(id);
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
  };

  use super::*;
  use crate::kitty_graphics::{KittyParser, prepare_image};
  use crate::test_support::fake_terminal_session;

  fn apply(terminal: &mut Terminal, raw: &[u8], line: i32, column: i32) {
    let mut command = KittyParser::new().parse(raw).unwrap().unwrap();
    let image = matches!(
      command.action,
      KittyAction::Transmit | KittyAction::TransmitAndDisplay
    )
    .then(|| prepare_image(&mut command).unwrap());
    terminal.execute_graphics_command(&command, image, line, column);
  }

  #[test]
  fn graphics_placement_uses_crop_single_axis_and_native_dimensions() {
    let (mut terminal, _, _, _) = fake_terminal_session(80, 24);
    let bounds = &mut terminal.last_content.terminal_bounds;
    bounds.cell_width = gpui::px(10.0);
    bounds.line_height = gpui::px(20.0);
    let mut upload = KittyCommand {
      image_id: 7,
      source_width: 1000,
      source_height: 80,
      payload: vec![0; 1000 * 80 * 4],
      ..KittyCommand::default()
    };
    let image = prepare_image(&mut upload).unwrap();
    terminal.execute_graphics_command(&upload, Some(image), 0, 0);

    for (raw, expected) in [
      ("a=p,i=7,p=1,c=3,C=1", (3, 1)),
      ("a=p,i=7,p=1,r=2,C=1", (50, 2)),
      ("a=p,i=7,p=1,x=20,y=10,w=40,h=30,C=1", (4, 2)),
      ("a=p,i=7,p=1,C=1", (100, 4)),
    ] {
      apply(&mut terminal, raw.as_bytes(), 0, 0);
      let mut extent = None;
      terminal.placement_manager.remove_matching(|p| {
        extent = Some((p.width_cells, p.height_cells));
        false
      });
      assert_eq!(extent, Some(expected), "{raw}");
    }
  }

  #[test]
  fn query_validates_and_replies_without_storing_an_image() {
    use crate::kitty_graphics::{RawGraphicsCommand, RawGraphicsEvent, spawn_graphics_worker};
    use futures::channel::mpsc;
    let (mut terminal, _, writes, _) = fake_terminal_session(80, 24);
    let (tx, rx) = mpsc::unbounded();
    let events = spawn_graphics_worker(rx);
    tx.unbounded_send(RawGraphicsEvent::Command(RawGraphicsCommand {
      data: b"a=q,i=31,f=24,s=1,v=1;AAAA".to_vec(),
      cursor_line: 0,
      cursor_column: 0,
    }))
    .unwrap();
    drop(tx);
    terminal.apply_graphics_event(events.recv_blocking().unwrap());
    assert_eq!(*writes.lock().unwrap(), [b"\x1b_Gi=31;OK\x1b\\".to_vec()]);
    assert_eq!(terminal.image_storage.image_count(), 0);
    assert_eq!(terminal.placement_manager.placement_count(), 0);
  }

  fn apply_stream(terminal: &mut Terminal, commands: &[&[u8]]) {
    use crate::kitty_graphics::{RawGraphicsCommand, RawGraphicsEvent, spawn_graphics_worker};
    use futures::channel::mpsc;
    let (tx, rx) = mpsc::unbounded();
    let events = spawn_graphics_worker(rx);
    for data in commands {
      tx.unbounded_send(RawGraphicsEvent::Command(RawGraphicsCommand {
        data: data.to_vec(),
        cursor_line: 0,
        cursor_column: 0,
      }))
      .unwrap();
    }
    // Model the DA reply queued by the PTY filter at exactly this stream position.
    tx.unbounded_send(RawGraphicsEvent::Reply(b"\x1b[?62;22c".to_vec()))
      .unwrap();
    drop(tx);
    while let Ok(event) = events.recv_blocking() {
      terminal.apply_graphics_event(event);
    }
  }

  #[test]
  fn query_does_not_replace_existing_images_or_placements() {
    let (mut terminal, _, writes, _) = fake_terminal_session(80, 24);
    apply(
      &mut terminal,
      b"a=T,i=7,p=3,s=1,v=1,c=1,r=1,C=1;/wAA/w==",
      2,
      3,
    );
    let before = terminal.image_storage.peek(7).unwrap().render_image.clone();
    writes.lock().unwrap().clear();
    apply_stream(
      &mut terminal,
      &[
        b"a=q,i=7,p=42,f=24,s=1,v=1;AP8A",
        b"a=q,i=7,f=24,s=1,v=1;AA==",
      ],
    );
    assert_eq!(terminal.image_storage.image_count(), 1);
    assert_eq!(terminal.placement_manager.placement_count(), 1);
    assert!(Arc::ptr_eq(
      &before,
      &terminal.image_storage.peek(7).unwrap().render_image
    ));
    let writes = writes.lock().unwrap();
    assert_eq!(writes[0], b"\x1b_Gi=7;OK\x1b\\");
    assert!(writes[1].starts_with(b"\x1b_Gi=7;ENODATA:"));
    assert_eq!(writes[2], b"\x1b[?62;22c");
  }

  #[test]
  fn acknowledgements_errors_and_quiet_requests_are_ordered() {
    let (mut terminal, _, writes, _) = fake_terminal_session(80, 24);
    apply_stream(
      &mut terminal,
      &[
        b"i=1,s=1,v=1,q=1;AAAAAA==", // Store silently, still available for put.
        b"a=p,i=1,p=3,c=1,r=1,C=1",
        b"a=p,i=9,p=4,q=1",   // q=1 preserves errors.
        b"a=p,I=10,p=2",      // Missing image number is not a placement ID.
        b"a=p,i=8,q=2",       // q=2 suppresses errors.
        b"s=1,v=1;AAAAAA==",  // Anonymous upload gets no response.
        b"f=99,i=5,q=2;AAAA", // Quiet metadata may follow an invalid field.
        b"i=6,s=1,v=1,m=1,q=1;AAAA",
        b"m=1;***", // One error with first-chunk identity, not an anonymous error.
        b"m=0;AAAA",
        b"a=d,d=i,i=1", // Delete does not acknowledge.
      ],
    );
    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 5);
    assert_eq!(writes[0], b"\x1b_Gi=1,p=3;OK\x1b\\");
    assert_eq!(writes[1], b"\x1b_Gi=9,p=4;ENOENT:Image not found\x1b\\");
    assert_eq!(writes[2], b"\x1b_GI=10,p=2;ENOENT:Image not found\x1b\\");
    assert!(writes[3].starts_with(b"\x1b_Gi=6;EINVAL:"));
    assert_eq!(writes[4], b"\x1b[?62;22c");
    assert_eq!(terminal.placement_manager.placement_count(), 0);
    assert!(terminal.image_storage.peek(1).is_some());
  }

  #[test]
  fn failed_storage_sends_enospc_instead_of_acknowledging_success() {
    let (mut terminal, _, writes, _) = fake_terminal_session(80, 24);
    let mut command = KittyParser::new()
      .parse(b"a=T,i=42,s=1,v=1;AAAAAA==")
      .unwrap()
      .unwrap();
    let mut image = prepare_image(&mut command).unwrap();
    // Inject the storage-quota boundary without allocating a 320 MiB buffer.
    image.memory_bytes = crate::kitty_graphics::command::MAX_IMAGE_BYTES + 1;
    terminal.execute_graphics_command(&command, Some(image), 0, 0);
    assert_eq!(terminal.image_storage.image_count(), 0);
    assert_eq!(terminal.placement_manager.placement_count(), 0);
    assert!(writes.lock().unwrap()[0].starts_with(b"\x1b_Gi=42;ENOSPC:"));
  }

  #[test]
  fn static_transfers_replace_placements_and_resolve_image_numbers() {
    let (mut terminal, _, writes, _) = fake_terminal_session(80, 24);
    terminal.pending_cnl = Some(Arc::new(AtomicU32::new(0)));
    apply(&mut terminal, b"i=7,s=1,v=1;AAAAAA==", 2, 3);
    assert_eq!(terminal.image_storage.image_count(), 1);
    assert_eq!(terminal.placement_manager.placement_count(), 0);
    apply(&mut terminal, b"a=p,i=7,p=3,c=2,r=2,C=1", 2, 3);
    apply(&mut terminal, b"a=p,i=7,p=3,c=2,r=2,C=1", 4, 5);
    assert_eq!(terminal.placement_manager.placement_count(), 1);
    assert_eq!(
      terminal
        .pending_cnl
        .as_ref()
        .unwrap()
        .load(Ordering::Acquire),
      0
    );
    apply(&mut terminal, b"i=7,s=1,v=1;AAAAAA==", 0, 0);
    assert_eq!(terminal.placement_manager.placement_count(), 0);

    for _ in 0..2 {
      apply(&mut terminal, b"I=9,s=1,v=1;AAAAAA==", 0, 0);
    }
    let newest = terminal.image_storage.newest_for_number(9).unwrap();
    apply(&mut terminal, b"a=p,I=9,p=4,c=2,r=2,C=1", 6, 7);
    assert!(terminal.placement_manager.has_image(newest));
    apply(&mut terminal, b"a=d,d=N,I=9", 0, 0);
    assert!(terminal.image_storage.peek(newest).is_none());
    assert!(terminal.image_storage.newest_for_number(9).is_some());
    assert_eq!(
      *writes.lock().unwrap(),
      [
        b"\x1b_Gi=7;OK\x1b\\".to_vec(),
        b"\x1b_Gi=7,p=3;OK\x1b\\".to_vec(),
        b"\x1b_Gi=7,p=3;OK\x1b\\".to_vec(),
        b"\x1b_Gi=7;OK\x1b\\".to_vec(),
        b"\x1b_Gi=1,I=9;OK\x1b\\".to_vec(),
        b"\x1b_Gi=2,I=9;OK\x1b\\".to_vec(),
        b"\x1b_Gi=2,I=9,p=4;OK\x1b\\".to_vec(),
      ]
    );
  }

  #[test]
  fn deletion_preserves_data_and_unrelated_or_scrollback_placements() {
    let (mut terminal, _, _, _) = fake_terminal_session(80, 24);
    for id in 1..=3 {
      let raw = format!("a=T,i={id},s=1,v=1,c=4,r=3,C=1;AAAAAA==");
      apply(
        &mut terminal,
        raw.as_bytes(),
        if id == 3 { -10 } else { 2 },
        if id == 2 { 20 } else { 3 },
      );
    }
    // Cell (5,4) intersects image 1 away from its origin, not image 2.
    apply(&mut terminal, b"a=d,d=p,x=5,y=4", 0, 0);
    assert!(!terminal.placement_manager.has_image(1));
    assert!(terminal.image_storage.peek(1).is_some());
    assert!(terminal.placement_manager.has_image(2));
    apply(&mut terminal, b"a=d,d=A", 0, 0);
    assert!(terminal.image_storage.peek(2).is_none());
    assert!(terminal.image_storage.peek(1).is_some()); // already unplaced
    assert!(terminal.placement_manager.has_image(3)); // in scrollback
    apply(&mut terminal, b"a=d,d=I,i=1", 0, 0);
    assert!(terminal.image_storage.peek(1).is_none());
    assert!(terminal.image_storage.peek(3).is_some());
  }

  #[test]
  fn scoped_deletions_do_not_clear_other_images_or_placements() {
    for selector in ["x,x=5", "y,y=4", "z,z=-1", "q,x=5,y=4,z=-1", "c"] {
      let (mut terminal, _, _, _) = fake_terminal_session(80, 24);
      apply(
        &mut terminal,
        b"a=T,i=1,p=1,s=1,v=1,c=4,r=3,z=-1,C=1;AAAAAA==",
        2,
        3,
      );
      apply(&mut terminal, b"a=p,i=1,p=2,c=2,r=2,z=2,C=1", 10, 20);
      apply(
        &mut terminal,
        b"a=T,i=2,s=1,v=1,c=1,r=1,C=1;AAAAAA==",
        12,
        30,
      );
      let raw = format!("a=d,d={selector}");
      apply(&mut terminal, raw.as_bytes(), 3, 4);
      assert_eq!(
        terminal.placement_manager.placement_count(),
        2,
        "{selector}"
      );
      assert!(terminal.image_storage.peek(1).is_some());
      assert!(terminal.image_storage.peek(2).is_some());
    }
  }

  #[test]
  fn placement_specific_uppercase_delete_only_frees_unreferenced_data() {
    let (mut terminal, _, _, _) = fake_terminal_session(80, 24);
    apply(
      &mut terminal,
      b"a=T,i=1,p=1,s=1,v=1,c=1,r=1,C=1;AAAAAA==",
      0,
      0,
    );
    apply(&mut terminal, b"a=p,i=1,p=2,c=1,r=1,C=1", 1, 1);
    apply(&mut terminal, b"a=d,d=I,i=1,p=1", 0, 0);
    assert!(terminal.image_storage.peek(1).is_some());
    apply(&mut terminal, b"a=d,d=I,i=1,p=99", 0, 0);
    assert!(terminal.image_storage.peek(1).is_some());
    apply(&mut terminal, b"a=d,d=I,i=1,p=2", 0, 0);
    assert!(terminal.image_storage.peek(1).is_none());
  }
}
