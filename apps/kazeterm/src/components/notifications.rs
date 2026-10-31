use std::time::Duration;

use super::main_window::MainWindow;

mod content;
mod delivery;
mod state;
use content::NotificationContent;

#[derive(Clone)]
pub(crate) enum NotificationReason {
  CommandFinished,
  Bell,
  ProgramStatus(terminal::program_status::ProgramStatusRecord),
}

fn notification_is_due(
  config: &config::NotificationConfig,
  idle_duration: Option<Duration>,
  since_last: Option<Duration>,
) -> bool {
  // Only explicit OSC status omits idle time; legacy sources always supply it.
  config.enabled
    && (idle_duration.is_some() || config.osc7501_enabled)
    && idle_duration
      .is_none_or(|idle| idle >= Duration::from_secs(config.long_running_threshold_secs))
    && since_last.is_none_or(|elapsed| elapsed >= Duration::from_secs(config.interval_secs))
}

impl MainWindow {
  fn send_notification(content: NotificationContent) {
    std::thread::spawn(move || {
      #[cfg(target_os = "macos")]
      {
        // Bypass notify-rust on macOS to avoid the "select application" dialog
        // caused by mac-notification-sys. Use NSUserNotificationCenter via objc,
        // falling back to osascript if the (deprecated) class is unavailable.
        if !Self::send_notification_native(&content) {
          Self::send_notification_osascript(&content);
        }
      }
      #[cfg(target_os = "linux")]
      {
        let _ = notify_rust::Notification::new()
          .summary(&content.title)
          .body(&content.linux_body())
          .show();
      }
      #[cfg(target_os = "windows")]
      {
        Self::send_notification_windows(&content);
      }
    });
  }

  #[cfg(target_os = "macos")]
  fn send_notification_native(content: &NotificationContent) -> bool {
    use objc::runtime::Class;

    let body = &content.body;

    unsafe {
      let Some(_) = Class::get("NSUserNotification") else {
        return false;
      };

      let notification: *mut objc::runtime::Object = msg_send![class!(NSUserNotification), new];
      if notification.is_null() {
        return false;
      }

      let title_bytes = content.title.as_bytes();
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
  fn send_notification_osascript(content: &NotificationContent) {
    let script = content.macos_script();
    let _ = std::process::Command::new("osascript")
      .args(["-e", &script])
      .output();
  }

  #[cfg(target_os = "windows")]
  fn send_notification_windows(content: &NotificationContent) {
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

    let xml = content.windows_xml();

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

#[cfg(test)]
mod tests {
  use super::notification_is_due;
  use std::time::Duration;

  #[test]
  fn notification_policy_requires_opt_in_and_respects_time_limits() {
    for (enabled, osc7501_enabled) in [(false, false), (false, true), (true, false), (true, true)] {
      let mut config = config::NotificationConfig {
        enabled,
        osc7501_enabled,
        long_running_threshold_secs: 10,
        interval_secs: 5,
      };
      for (idle, since_last, due) in [
        (0, None, false),
        (9, None, false),
        (10, None, true),
        (20, Some(4), false),
        (20, Some(5), true),
        (20, Some(6), true),
      ] {
        assert_eq!(
          notification_is_due(
            &config,
            Some(Duration::from_secs(idle)),
            since_last.map(Duration::from_secs)
          ),
          enabled && due,
        );
      }
      assert_eq!(
        notification_is_due(&config, None, None),
        enabled && osc7501_enabled
      );
      assert!(!notification_is_due(&config, None, Some(Duration::ZERO)));
      config.long_running_threshold_secs = 0;
      config.interval_secs = 0;
      assert_eq!(
        notification_is_due(&config, Some(Duration::ZERO), None),
        enabled
      );
      assert_eq!(
        notification_is_due(&config, Some(Duration::ZERO), Some(Duration::ZERO)),
        enabled
      );
      assert_eq!(
        notification_is_due(&config, None, Some(Duration::ZERO)),
        enabled && osc7501_enabled
      );
    }
  }
}
