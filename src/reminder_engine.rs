use chrono::{Duration, Local, NaiveDate};
use rusqlite::{params, Result};

use crate::notifier::Notifier;
use crate::vault::Vault;
use crate::crypto;

pub struct ReminderEngine<'a> {
    vault: &'a Vault,
}

impl<'a> ReminderEngine<'a> {
    pub fn new(vault: &'a Vault) -> Self {
        Self { vault }
    }

    /// Scan expenses with a due_date and auto-insert reminders
    /// 3 days before the due date (if not already inserted).
    pub fn auto_create_from_due_dates(&self) -> Result<usize> {
        let conn = self.vault.conn();
        let key = self.vault.key(); // 🔐 assume Vault exposes key()

        let mut stmt = conn.prepare(
            "SELECT e.id, e.vendor, e.amount, e.due_date
             FROM expenses e
             WHERE e.due_date IS NOT NULL
             AND e.due_date != ''
             AND NOT EXISTS (
                 SELECT 1 FROM reminders r WHERE r.expense_id = e.id
             )",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?, // 🔐 encrypted vendor
                row.get::<_, Vec<u8>>(2)?, // 🔐 encrypted amount
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut created = 0;

        for row in rows {
            let (id, enc_vendor, enc_amount, due_date_str) = row?;

            // 🔓 decrypt vendor
            let vendor = match String::from_utf8(
                crypto::decrypt_blob(key, &enc_vendor),
            ) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // 🔓 decrypt amount
            let amount = match String::from_utf8(
                crypto::decrypt_blob(key, &enc_amount),
            ) {
                Ok(a) => a,
                Err(_) => continue,
            };

            if let Some(due) = parse_date(&due_date_str) {
                let remind_on = due - Duration::days(3);

                let message = format!(
                    "Payment due in 3 days — {} | {} | Due: {}",
                    vendor, amount, due_date_str
                );

                conn.execute(
                    "INSERT INTO reminders (expense_id, message, remind_on)
                     VALUES (?1, ?2, ?3)",
                    params![
                        id,
                        message,
                        remind_on.format("%Y-%m-%d").to_string()
                    ],
                )?;

                created += 1;

                println!(
                    " [reminder] Auto-created for: {} (due {})",
                    vendor, due_date_str
                );
            }
        }

        Ok(created)
    }

    /// Check for reminders due today or overdue → fire notifications.
    pub fn check_and_notify(&self) -> Result<usize> {
        let conn = self.vault.conn();
        let today = Local::now().format("%Y-%m-%d").to_string();

        let mut stmt = conn.prepare(
            "SELECT id, message, remind_on
             FROM reminders
             WHERE done = 0 AND remind_on <= ?1
             ORDER BY remind_on ASC",
        )?;

        let rows = stmt.query_map(params![today], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut fired = 0;

        for row in rows {
            let (rid, message, remind_on) = row?;

            let overdue = remind_on < today;

            if overdue {
                Notifier::send_urgent("Overdue bill reminder", &message);
            } else {
                Notifier::send("Bill reminder", &message);
            }

            println!(
                " [fired] id={} overdue={} — {}",
                rid, overdue, message
            );

            fired += 1;
        }

        Ok(fired)
    }

    /// Mark a reminder as done by id.
    pub fn mark_done(&self, reminder_id: i64) -> Result<()> {
        self.vault.conn().execute(
            "UPDATE reminders SET done = 1 WHERE id = ?1",
            params![reminder_id],
        )?;

        println!(" [done] Reminder {} marked complete.", reminder_id);
        Ok(())
    }

    /// Add a manual reminder from CLI.
    pub fn add_manual(
        &self,
        expense_id: i64,
        message: &str,
        remind_on: &str, // "YYYY-MM-DD"
    ) -> Result<i64> {
        let conn = self.vault.conn();

        conn.execute(
            "INSERT INTO reminders (expense_id, message, remind_on)
             VALUES (?1, ?2, ?3)",
            params![expense_id, message, remind_on],
        )?;

        let id = conn.last_insert_rowid();

        println!(" [added] Reminder id={} on {}", id, remind_on);

        Ok(id)
    }

    /// List all pending reminders.
    pub fn list_pending(&self) -> Result<Vec<(i64, String, String)>> {
        let conn = self.vault.conn();

        let mut stmt = conn.prepare(
            "SELECT r.id, r.message, r.remind_on
             FROM reminders r
             WHERE r.done = 0
             ORDER BY r.remind_on ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        rows.collect()
    }
}

/// Parse common date formats from OCR / NLP output.
fn parse_date(s: &str) -> Option<NaiveDate> {
    let fmts = [
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%Y-%m-%d",
        "%d-%m-%Y",
        "%d.%m.%Y",
        "%-d %B %Y",
        "%d %B %Y",
        "%B %d, %Y",
    ];

    for fmt in fmts {
        if let Ok(d) = NaiveDate::parse_from_str(s.trim(), fmt) {
            return Some(d);
        }
    }

    None
}