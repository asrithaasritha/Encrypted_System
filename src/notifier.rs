use notify_rust::{Notification, Timeout, Urgency};

pub struct Notifier;

impl Notifier {
    // 🔔 Normal notification (auto disappears)
    pub fn send(title: &str, body: &str) {
        let result = Notification::new()
            .summary(title)
            .body(body)
            .icon("dialog-information")
            .timeout(Timeout::Milliseconds(8000))
            .show();

        match result {
            Ok(_) => println!("🔔 Notification sent"),
            Err(e) => eprintln!("❌ Notification failed: {}", e),
        }
    }

    // 🚨 Urgent notification (stays until user clicks)
    pub fn send_urgent(title: &str, body: &str) {
        let result = Notification::new()
            .summary(title)
            .body(body)
            .icon("dialog-warning")
            .urgency(Urgency::Critical)
            .timeout(Timeout::Never)
            .show();

        if let Err(e) = result {
            eprintln!("❌ Urgent notification failed: {}", e);
        }
    }
}