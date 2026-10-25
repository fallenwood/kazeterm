use gpui::{Context, Entity};
use terminal::TerminalView;

use super::main_window::MainWindow;

pub(crate) enum NotificationReason {
  CommandFinished,
  Bell,
}

impl MainWindow {
  /// Possibly send a desktop notification, respecting idle-time and throttle config.
  pub(crate) fn maybe_send_notification(
    &mut self,
    terminal_view: &Entity<TerminalView>,
    reason: NotificationReason,
    cx: &mut Context<Self>,
  ) {
    let threshold_secs = cx
      .global::<config::Config>()
      .notification
      .long_running_threshold_secs;
    let idle_duration = terminal_view
      .read(cx)
      .terminal()
      .read(cx)
      .last_input_time
      .elapsed();

    let interval_secs = cx.global::<config::Config>().notification.interval_secs;
    let interval_ok = match self.last_notification_time {
      Some(last) => last.elapsed() >= std::time::Duration::from_secs(interval_secs),
      None => true,
    };

    if idle_duration >= std::time::Duration::from_secs(threshold_secs) && interval_ok {
      let body = {
        let terminal = terminal_view.read(cx).terminal().read(cx);
        match reason {
          NotificationReason::CommandFinished => {
            // Show the process that just finished (the title before prompt returned).
            let prev = &terminal.previous_title_text;
            if prev.is_empty() {
              terminal.title_text.clone()
            } else {
              prev.clone()
            }
          }
          NotificationReason::Bell => {
            // Show the process that sent the bell.
            terminal.title_text.clone()
          }
        }
      };

      let tab_title = self
        .items
        .iter()
        .find(|item| {
          item
            .split_container
            .all_terminals()
            .iter()
            .any(|(_pos, t)| t.entity_id() == terminal_view.entity_id())
        })
        .map(|item| item.display_title().to_string());
      Self::send_notification(tab_title, body);
      self.last_notification_time = Some(std::time::Instant::now());
    }
  }

  fn send_notification(tab_title: Option<String>, body: String) {
    std::thread::spawn(move || {
      let body = if body.is_empty() {
        tab_title.as_deref().unwrap_or("Terminal").to_string()
      } else if let Some(tab) = &tab_title {
        format!("{tab} — {body}")
      } else {
        body
      };
      #[cfg(target_os = "macos")]
      {
        // Bypass notify-rust on macOS to avoid the "select application" dialog
        // caused by mac-notification-sys. Use NSUserNotificationCenter via objc,
        // falling back to osascript if the (deprecated) class is unavailable.
        if !Self::send_notification_native(&body) {
          Self::send_notification_osascript(&body);
        }
      }
      #[cfg(target_os = "linux")]
      {
        let _ = notify_rust::Notification::new()
          .summary("Kazeterm")
          .body(&format!("{body}"))
          .show();
      }
      #[cfg(target_os = "windows")]
      {
        Self::send_notification_windows(&body);
      }
    });
  }

  #[cfg(target_os = "macos")]
  fn send_notification_native(body: &str) -> bool {
    use objc::runtime::Class;

    unsafe {
      let Some(_) = Class::get("NSUserNotification") else {
        return false;
      };

      let notification: *mut objc::runtime::Object = msg_send![class!(NSUserNotification), new];
      if notification.is_null() {
        return false;
      }

      let title_bytes = "Kazeterm";
      let title_ns: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
      let title_ns: *mut objc::runtime::Object = msg_send![
        title_ns,
        initWithBytes: title_bytes.as_ptr()
        length: title_bytes.len()
        encoding: 4usize // NSUTF8StringEncoding
      ];

      let body_ns: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
      let body_ns: *mut objc::runtime::Object = msg_send![
        body_ns,
        initWithBytes: body.as_ptr()
        length: body.len()
        encoding: 4usize
      ];

      let _: () = msg_send![notification, setTitle: title_ns];
      let _: () = msg_send![notification, setInformativeText: body_ns];

      let center: *mut objc::runtime::Object = msg_send![
        class!(NSUserNotificationCenter),
        defaultUserNotificationCenter
      ];
      if center.is_null() {
        return false;
      }
      let _: () = msg_send![center, deliverNotification: notification];
      true
    }
  }

  #[cfg(target_os = "macos")]
  fn send_notification_osascript(body: &str) {
    let escaped = body.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(r#"display notification "{escaped}" with title "Kazeterm""#);
    let _ = std::process::Command::new("osascript")
      .args(["-e", &script])
      .output();
  }

  #[cfg(target_os = "windows")]
  fn send_notification_windows(body: &str) {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
    use windows::core::HSTRING;

    // Use PowerShell's AUMID — it is always registered on Windows, so
    // CreateToastNotifierWithId works without any shortcut or registry setup.
    const POWERSHELL_AUMID: &str =
      "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";

    unsafe {
      let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let escaped = body
      .replace('&', "&amp;")
      .replace('<', "&lt;")
      .replace('>', "&gt;")
      .replace('"', "&quot;");
    let xml = format!(
      r#"<toast><visual><binding template="ToastGeneric"><text>Kazeterm</text><text>{escaped}</text></binding></visual></toast>"#
    );

    let Ok(doc) = XmlDocument::new() else { return };
    if doc.LoadXml(&HSTRING::from(&xml)).is_err() {
      return;
    }
    let Ok(toast) = ToastNotification::CreateToastNotification(&doc) else {
      return;
    };
    let Ok(notifier) =
      ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(POWERSHELL_AUMID))
    else {
      return;
    };
    let _ = notifier.Show(&toast);
  }

  pub(crate) fn play_bell_sound(&self) {
    #[cfg(target_os = "windows")]
    {
      std::thread::spawn(|| {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC};
        use windows::core::w;
        unsafe {
          let _ = PlaySoundW(w!("SystemAsterisk"), None, SND_ALIAS | SND_ASYNC);
        }
      });
    }

    #[cfg(target_os = "macos")]
    {
      #[link(name = "AudioToolbox", kind = "framework")]
      unsafe extern "C" {
        fn AudioServicesPlayAlertSound(inSystemSoundID: u32);
      }
      // kSystemSoundID_UserPreferredAlert plays the user's preferred alert sound
      unsafe {
        AudioServicesPlayAlertSound(0x00001000);
      }
    }

    #[cfg(target_os = "linux")]
    {
      std::thread::spawn(|| {
        if which::which("canberra-gtk-play").is_err() {
          tracing::debug!("Skipping bell sound because canberra-gtk-play is not available");
          return;
        }

        if let Err(error) = std::process::Command::new("canberra-gtk-play")
          .args(["--id", "bell"])
          .stdin(std::process::Stdio::null())
          .stdout(std::process::Stdio::null())
          .stderr(std::process::Stdio::null())
          .spawn()
        {
          tracing::debug!("Failed to spawn canberra-gtk-play for bell sound: {error}");
        }
      });
    }
  }
}
