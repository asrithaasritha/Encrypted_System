#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod vault; mod models; mod nlp_client;
mod reminder_engine; mod notifier; mod scheduler; mod crypto;

use tauri::State;
use std::sync::Mutex;
use serde::Serialize;
use vault::Vault;
use nlp_client::NLPClient;
use reminder_engine::ReminderEngine;

struct AppState { vault: Mutex<Option<Vault>> }

#[derive(Serialize)]
struct ExpenseRow {
    id: i64, vendor: String, amount: String,
    date: String, category: String,
    due_date: Option<String>, confidence: f32,
}
#[derive(Serialize)]
struct ReminderRow { id: i64, message: String, remind_on: String }

#[derive(Serialize)]
struct Res<T: Serialize> {
    ok: bool, data: Option<T>, error: Option<String>
}
impl<T: Serialize> Res<T> {
    fn ok(d: T) -> Self { Self { ok:true, data:Some(d), error:None } }
    fn err(e: &str) -> Self { Self { ok:false, data:None, error:Some(e.into()) } }
}

#[tauri::command]
fn unlock_vault(password: String, state: State<AppState>) -> Res<String> {
    match Vault::open("vault.db", &password) {
        Ok(v)  => { *state.vault.lock().unwrap() = Some(v);
                    Res::ok("unlocked".into()) }
        Err(e) => Res::err(&e.to_string()),
    }
}

#[tauri::command]
fn list_expenses(state: State<AppState>) -> Res<Vec<ExpenseRow>> {
    let g = state.vault.lock().unwrap();
    match g.as_ref() {
        None    => Res::err("vault locked"),
        Some(v) => match v.list_expenses() {
            Err(e)   => Res::err(&e.to_string()),
            Ok(rows) => Res::ok(rows.iter().map(|e| ExpenseRow {
                id: e.id.unwrap_or(0), vendor: e.vendor.clone(),
                amount: e.amount.clone(), date: e.date.clone(),
                category: e.category.clone(), due_date: e.due_date.clone(),
                confidence: e.confidence,
            }).collect()),
        }
    }
}

#[tauri::command]
async fn scan_image(
    path: String, state: State<'_, AppState>
) -> Result<Res<ExpenseRow>, String> {
    use std::process::Command;
    let out = Command::new("tesseract")
        .arg(&path).arg("stdout").arg("--psm").arg("6")
        .output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    if text.trim().is_empty() { return Ok(Res::err("No text extracted")); }

    let result = NLPClient::new().analyze(&text);
    let exp = models::Expense {
        id: None,
        vendor: result.vendor.unwrap_or_else(|| "Unknown vendor".to_string()),
        amount: result.amount.unwrap_or_else(|| "Not found".to_string()),
        date: result.date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string()),
        due_date: result.due_date,
        warranty_period: result.warranty_period,
        category: if result.category.trim().is_empty() { "General".to_string() } else { result.category },
        confidence: result.confidence,
        source_file: path.clone(),
    };

    let g = state.vault.lock().unwrap();
    match g.as_ref() {
        None    => Ok(Res::err("vault locked")),
        Some(v) => match v.save_expense(&exp) {
            Err(e) => Ok(Res::err(&e.to_string())),
            Ok(id) => Ok(Res::ok(ExpenseRow {
                id, vendor: exp.vendor, amount: exp.amount,
                date: exp.date, category: exp.category,
                due_date: exp.due_date, confidence: exp.confidence,
            })),
        }
    }
}

#[tauri::command]
fn list_reminders(state: State<AppState>) -> Res<Vec<ReminderRow>> {
    let g = state.vault.lock().unwrap();
    match g.as_ref() {
        None    => Res::err("vault locked"),
        Some(v) => match ReminderEngine::new(v).list_pending() {
            Err(e)   => Res::err(&e.to_string()),
            Ok(rows) => Res::ok(rows.iter().map(|(id,msg,dt)| ReminderRow {
                id:*id, message:msg.clone(), remind_on:dt.clone()
            }).collect()),
        }
    }
}

#[tauri::command]
fn mark_reminder_done(id: i64, state: State<AppState>) -> Res<String> {
    let g = state.vault.lock().unwrap();
    match g.as_ref() {
        None    => Res::err("vault locked"),
        Some(v) => match ReminderEngine::new(v).mark_done(id) {
            Ok(_)  => Res::ok("done".into()),
            Err(e) => Res::err(&e.to_string()),
        }
    }
}

#[tauri::command]
fn export_vault(state: State<AppState>) -> Res<String> {
    if state.vault.lock().unwrap().is_none() { return Res::err("vault locked"); }
    match std::fs::copy("vault.db", "vault_export.db") {
        Ok(_)  => Res::ok("vault_export.db".into()),
        Err(e) => Res::err(&e.to_string()),
    }
}

#[tauri::command]
fn check_reminders_now(state: State<AppState>) -> Res<String> {
    let g = state.vault.lock().unwrap();
    match g.as_ref() {
        None => Res::err("vault locked"),
        Some(v) => {
            let eng = ReminderEngine::new(v);
            eng.auto_create_from_due_dates().ok();
            let n = eng.check_and_notify().unwrap_or(0);
            Res::ok(format!("{n} notifications fired"))
        }
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState { vault: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![
            unlock_vault, list_expenses, scan_image,
            list_reminders, mark_reminder_done,
            export_vault, check_reminders_now,
        ])
        .run(tauri::generate_context!())
        .expect("error running app");
}