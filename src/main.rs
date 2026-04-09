use std::process::Command;
use std::env;
use std::fs;
use std::path::Path;
use regex::Regex;

mod nlp_client;
mod vault;
mod models;
mod crypto;
mod notifier;
mod reminder_engine;
mod scheduler;

use nlp_client::NLPClient;
use models::Expense;
use rpassword::read_password;

use vault::Vault;
use reminder_engine::ReminderEngine;
use scheduler::{Scheduler, Job, JobAction};

use chrono::{Local, Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// 🔹 OCR
fn run_ocr(file_path: &str) -> String {
    let output = Command::new("tesseract")
        .arg(file_path)
        .arg("stdout")
        .arg("--oem").arg("3")
        .arg("--psm").arg("6")
        .output()
        .expect("Tesseract not found");

    String::from_utf8_lossy(&output.stdout).to_string()
}

// 🔹 Amount
fn extract_amount(text: &str) -> Option<String> {
    let re = Regex::new(
        r"(?i)(total|amount|amt|rs\.?|₹|inr)?\s*[:\-]?\s*(\d{1,3}(,\d{3})*(\.\d{2})?)"
    ).unwrap();

    let mut values = Vec::new();

    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(2) {
            let cleaned = m.as_str().replace(",", "");
            if let Ok(v) = cleaned.parse::<f64>() {
                values.push(v);
            }
        }
    }

    values.into_iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .map(|v| format!("{:.2}", v))
}

// 🔹 Date
fn extract_date(text: &str) -> Option<String> {
    let patterns = [
        r"\b\d{1,2}[/\-.]\d{1,2}[/\-.]\d{2,4}\b",
        r"\b\d{1,2}\s+(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)(?:uary|ch|il|e|y|ust|tember|ober|ember)?[.,]?\s*\d{2,4}\b",
        r"\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)(?:uary|ch|il|e|y|ust|tember|ober|ember)?\s+\d{1,2}[.,]?\s*\d{2,4}\b",
        r"\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)(?:uary|ch|il|e|y|ust|tember|ober|ember)?\s+\d{2,4}\b",
    ];

    for pat in patterns {
        let re = Regex::new(pat).unwrap();
        if let Some(m) = re.find(text) {
            return Some(m.as_str().trim().to_string());
        }
    }

    None
}

fn normalize_date(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_lowercase();
    if lower == "month" || lower == "date" || lower == "due" || lower == "invoice" {
        return None;
    }

    let month_re = Regex::new(
        r"\b(?:jan|feb|mar|apr|may|jun|jul|aug|sep|sept|oct|nov|dec)(?:uary|ch|il|e|y|ust|tember|ober|ember)?\b",
    ).unwrap();

    if trimmed.chars().any(|c| c.is_ascii_digit()) || month_re.is_match(&lower) {
        return Some(trimmed.to_string());
    }

    None
}

// 🔹 Vendor
fn extract_vendor(text: &str) -> String {
    for line in text.lines() {
        let t = line.trim();

        if t.len() > 4
            && !t.starts_with(|c: char| c.is_numeric())
            && !t.to_lowercase().contains("invoice")
            && !t.to_lowercase().contains("bill")
        {
            return t.to_string();
        }
    }
    "Unknown vendor".into()
}

fn gather_input_files(paths: &[String]) -> Vec<String> {
    let supported_exts = [
        "png", "jpg", "jpeg", "tif", "tiff", "bmp", "gif", "webp",
    ];

    let mut files = Vec::new();

    for raw_path in paths {
        let path = Path::new(raw_path);

        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.filter_map(Result::ok) {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                            let ext = ext.to_lowercase();
                            if supported_exts.contains(&ext.as_str()) {
                                files.push(entry_path.to_string_lossy().into_owned());
                            }
                        }
                    }
                }
            }
        } else if path.is_file() {
            files.push(raw_path.clone());
        } else {
            eprintln!("Skipping invalid path: {}", raw_path);
        }
    }

    files
}

fn main() {
    let args: Vec<String> = env::args().collect();

    println!("Enter vault password:");
    let password = read_password().unwrap();

    let vault = Vault::new(&password);

    // 🔥 COMMAND MODE
    if args.len() > 1 {
        let cmd = args[1].as_str();

        match cmd {

            // 📋 LIST REMINDERS
            "reminders" => {
                let engine = ReminderEngine::new(&vault);

                match engine.list_pending() {
                    Ok(list) if list.is_empty() => {
                        println!("No pending reminders.");
                    }
                    Ok(list) => {
                        println!("\n--- Pending Reminders ---");
                        for (id, msg, date) in &list {
                            println!("[{}] {} {}", id, date, msg);
                        }
                        println!("\nTotal: {}", list.len());
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
                return;
            }

            // ✅ MARK DONE
            "done" => {
                let id: i64 = args.get(2)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                if id == 0 {
                    eprintln!("Usage: cargo run -- done <reminder_id>");
                } else {
                    ReminderEngine::new(&vault).mark_done(id).ok();
                }
                return;
            }

            // ➕ ADD REMINDER
            "remind" => {
                let exp_id: i64 = args.get(2)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                let date = args.get(3).map(|s| s.as_str()).unwrap_or("");
                let message = args.get(4).map(|s| s.as_str()).unwrap_or("Reminder");

                if exp_id == 0 || date.is_empty() {
                    eprintln!("Usage: cargo run -- remind <expense_id> <YYYY-MM-DD> <message>");
                } else {
                    ReminderEngine::new(&vault)
                        .add_manual(exp_id, message, date)
                        .ok();
                }
                return;
            }

            // 🔍 RUN ONE CHECK
            "check" => {
                let engine = ReminderEngine::new(&vault);

                println!("Auto-creating reminders...");
                let created = engine.auto_create_from_due_dates().unwrap_or(0);
                println!("Created: {}", created);

                println!("Checking reminders...");
                let fired = engine.check_and_notify().unwrap_or(0);
                println!("Notifications fired: {}", fired);

                return;
            }

            // 🤖 BACKGROUND DAEMON
            "daemon" => {
                println!("Starting reminder daemon. Ctrl+C to stop.\n");

                let running = Arc::new(AtomicBool::new(true));
                let r = running.clone();

                ctrlc::set_handler(move || {
                    println!("\nStopping daemon...");
                    r.store(false, Ordering::SeqCst);
                }).unwrap();

                let mut sched = Scheduler::new();

                sched.add_job(Job {
                    id: "check_reminders".into(),
                    next_run: Local::now(),
                    interval: Duration::hours(24),
                    action: JobAction::CheckReminders,
                });

                sched.add_job(Job {
                    id: "auto_create".into(),
                    next_run: Local::now(),
                    interval: Duration::hours(24),
                    action: JobAction::AutoCreateFromDueDates,
                });

                while running.load(Ordering::SeqCst) {
                    let jobs = sched.pop_due();
                    let engine = ReminderEngine::new(&vault);

                    for job in jobs {
                        match job.action {
                            JobAction::CheckReminders => {
                                println!("[{}] Checking...",
                                    Local::now().format("%H:%M:%S"));
                                engine.check_and_notify().ok();
                            }
                            JobAction::AutoCreateFromDueDates => {
                                println!("[{}] Auto-creating...",
                                    Local::now().format("%H:%M:%S"));
                                engine.auto_create_from_due_dates().ok();
                            }
                        }

                        sched.reschedule(job);
                    }

                    let sleep = sched.next_run_in_secs()
                        .unwrap_or(60)
                        .min(60);

                    std::thread::sleep(
                        std::time::Duration::from_secs(sleep as u64)
                    );
                }

                println!("Daemon stopped.");
                return;
            }

            _ => {}
        }
    }

    // 🔥 OCR + NLP FLOW
    if args.len() < 2 {
        eprintln!("Usage: cargo run -- <image|directory>");
        return;
    }

    let input_files = gather_input_files(&args[1..]);
    if input_files.is_empty() {
        eprintln!("No supported image files found.");
        return;
    }

    let nlp = NLPClient::new();
    let mut all_output = String::new();

    for file_path in &input_files {

        println!("\nProcessing: {}\n", file_path);

        let text = run_ocr(file_path);

        if text.trim().is_empty() {
            println!("No text found\n");
            continue;
        }

        let result = nlp.analyze(&text);

        let vendor = result.vendor.unwrap_or_else(|| extract_vendor(&text));
        let amount = result.amount.unwrap_or_else(|| {
            extract_amount(&text).unwrap_or("Not found".into())
        });
        let date = result
            .date
            .as_ref()
            .and_then(|d| normalize_date(d))
            .or_else(|| extract_date(&text))
            .unwrap_or_else(|| "Not found".into());

        println!("Vendor : {}", vendor);
        println!("Amount : {}", amount);
        println!("Date   : {}", date);

        all_output.push_str(&format!(
            "{},{},{},{}\n",
            file_path, vendor, amount, date
        ));

        let expense = Expense {
            vendor: vendor.clone(),
            amount: amount.clone(),
            date: date.clone(),
            due_date: result.due_date.clone(),
            warranty_period: result.warranty_period.clone(),
            category: result.category.clone(),
            confidence: result.confidence,
            source_file: file_path.to_string(),
        };

        vault.insert_expense(&expense).unwrap();

        // 🔔 simple reminder
        if let Some(due) = &expense.due_date {
            let msg = format!("{} bill due on {}", vendor, due);
            let _ = vault.create_reminder(0, &msg, due);
        }
    }

    fs::write("results.csv",
        format!("file,vendor,amount,date\n{}", all_output)
    ).unwrap();

    println!("\n✅ Done. Data stored securely.");
}