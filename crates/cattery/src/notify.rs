//! Desktop notifications programs post (OSC 9, 99 and 777), as the daemon
//! relays them, shown by the platform.

use std::path::Path;
use std::time::Duration;

use felis_client_core::{ConnectError, Offer};
use felis_protocol::messages::{
    ConnToClientMsg, Notification, NotifyToClientMsg, ObserverKind, Urgency,
};
use felis_transport::{Delivery, Incoming};

/// A notification a session's program posted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posted {
    pub session: u128,
    pub title: String,
    pub body: String,
    pub urgent: bool,
}

/// What the platform shows for `notification`, posted by the session
/// called `session_title`: its own title, or else the session's name.
pub fn posted(session: u128, notification: Notification, session_title: Option<String>) -> Posted {
    let title = notification
        .title
        .filter(|title| !title.is_empty())
        .or(session_title.filter(|title| !title.is_empty()))
        .unwrap_or_else(|| "Cattery".to_owned());
    Posted {
        session,
        title,
        body: notification.body,
        urgent: notification.urgency == Urgency::Critical,
    }
}

/// Watch the daemon at `socket` for notifications, handing each to `post`,
/// for as long as `post` returns true. A daemon not up yet, or gone, is
/// waited for.
pub async fn watch(socket: &Path, post: impl Fn(Posted) -> bool) {
    loop {
        match relay(socket, &post).await {
            Ok(false) => return,
            Ok(true) => {}
            Err(err) => tracing::debug!(?err, "no notifications from the daemon for now"),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// Relay one connection's notifications; whether to go on watching.
async fn relay(socket: &Path, post: &impl Fn(Posted) -> bool) -> Result<bool, ConnectError> {
    let mut conn = felis_client_core::connect(socket, Offer::observer()).await?;
    conn.subscribe_observer(None, vec![ObserverKind::Notification])
        .await?;
    while let Some(frame) = conn.next_frame().await? {
        let msg = match conn.driver.classify(&frame) {
            Ok(Incoming::Payload(payload)) => {
                match conn.driver.decode::<NotifyToClientMsg>(&payload) {
                    Ok(Delivery::Deliver(delivered)) => delivered.msg,
                    Ok(_) => continue,
                    Err(err) => {
                        tracing::warn!(?err, "a notification from the daemon was refused");
                        return Ok(true);
                    }
                }
            }
            Ok(Incoming::Control(ConnToClientMsg::End { .. } | ConnToClientMsg::Error { .. })) => {
                return Ok(true);
            }
            Ok(_) => continue,
            Err(err) => {
                tracing::warn!(?err, "a frame from the daemon was refused");
                return Ok(true);
            }
        };
        if let NotifyToClientMsg::Event {
            session_id,
            notification,
            session_title,
            ..
        } = msg
            && !post(posted(session_id, notification, session_title))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Show `posted` as the platform shows an app's notifications.
pub fn show(posted: &Posted) {
    platform::show(posted);
}

#[cfg(target_os = "macos")]
mod platform {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationInterruptionLevel,
        UNNotificationRequest, UNUserNotificationCenter,
    };

    use super::Posted;

    pub fn show(posted: &Posted) {
        // The notification centre refuses, by raising, a process that is
        // no app: Cattery run as a bare executable has no bundle.
        if NSBundle::mainBundle().bundleIdentifier().is_none() {
            tracing::debug!("not an app bundle; the notification is not shown");
            return;
        }
        let center = UNUserNotificationCenter::currentNotificationCenter();
        // Asked every time: macOS asks the user once, and answers at once
        // after that.
        let asked = RcBlock::new(|granted: Bool, _: *mut NSError| {
            if !granted.as_bool() {
                tracing::debug!("notifications are not allowed");
            }
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &asked,
        );
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&posted.title));
        content.setBody(&NSString::from_str(&posted.body));
        content.setThreadIdentifier(&NSString::from_str(&format!("{:032x}", posted.session)));
        if posted.urgent {
            content.setInterruptionLevel(UNNotificationInterruptionLevel::TimeSensitive);
        }
        let id = NSString::from_str(&format!(
            "{:032x}-{}",
            posted.session,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let request =
            UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
        center.addNotificationRequest_withCompletionHandler(&request, None);
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::Posted;

    pub fn show(posted: &Posted) {
        let mut notification = notify_rust::Notification::new();
        notification
            .appname("Cattery")
            .summary(&posted.title)
            .body(&posted.body);
        #[cfg(all(unix, not(target_os = "macos")))]
        if posted.urgent {
            notification.urgency(notify_rust::Urgency::Critical);
        }
        if let Err(err) = notification.show() {
            tracing::warn!(?err, "the notification was not shown");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(title: Option<&str>, body: &str) -> Notification {
        Notification {
            title: title.map(str::to_owned),
            body: body.to_owned(),
            urgency: Urgency::Normal,
        }
    }

    #[test]
    fn a_notification_is_titled_as_it_asks() {
        let posted = posted(
            7,
            notification(Some("Build"), "done"),
            Some("zsh".to_owned()),
        );
        assert_eq!(posted.title, "Build");
        assert_eq!(posted.body, "done");
        assert_eq!(posted.session, 7);
    }

    #[test]
    fn an_untitled_one_is_titled_after_its_session_or_else_cattery() {
        let named = posted(
            1,
            notification(None, "done"),
            Some("cargo build".to_owned()),
        );
        assert_eq!(named.title, "cargo build");
        let unnamed = posted(1, notification(Some(""), "done"), Some(String::new()));
        assert_eq!(unnamed.title, "Cattery");
    }

    #[test]
    fn only_a_critical_one_is_urgent() {
        let mut critical = notification(None, "disk full");
        critical.urgency = Urgency::Critical;
        assert!(posted(1, critical, None).urgent);
        assert!(!posted(1, notification(None, "done"), None).urgent);
    }
}
