use std::process::Command;
use std::env;
use std::fs;
use regex::Regex;

mod vault;
mod models;

use models::Expense;

// 🔹 Run OCR
fn run_ocr(file_path: &str) -> String {
    let output = Command::new("tesseract")
        .arg(file_path)
        .arg("stdout")
        .arg("--oem").arg("3")
        .arg("--psm").arg("6")
        .output()
        .expect("Tesseract not found. Check PATH.");

    String::from_utf8_lossy(&output.stdout).to_string()
}

// 🔹 Extract amount
fn extract_amount(text: &str) -> Option<String> {
    let re = Regex::new(
        r"(?i)(total|amount|amt|rs\.?|₹|inr)?\s*[:\-]?\s*\d{2,7}(\,\d{3})*(\.\d{1,2})?"
    ).unwrap();

    for mat in re.find_iter(text) {
        let value = mat.as_str().trim();
        if value.chars().any(|c| c.is_numeric()) {
            return Some(value.to_string());
        }
    }
    None
}

// 🔹 Extract date
fn extract_date(text: &str) -> Option<String> {
    let re = Regex::new(r"\b\d{1,2}[/\-.]\d{1,2}[/\-.]\d{2,4}\b").unwrap();
    re.find(text).map(|m| m.as_str().to_string())
}

// 🔹 Extract vendor
fn extract_vendor(text: &str) -> String {
    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.len() > 4
            && !trimmed.starts_with(|c: char| c.is_numeric())
            && !trimmed.to_lowercase().contains("invoice")
            && !trimmed.to_lowercase().contains("bill")
        {
            return trimmed.to_string();
        }
    }
    "Unknown vendor".to_string()
}

// 🔥 MAIN FUNCTION
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run -- <img1> <img2> ...");
        std::process::exit(1);
    }

    // Initialize database
    let conn = vault::init_db();

    let files = &args[1..];
    let mut all_output = String::new();

    for file_path in files {
        println!("\nProcessing: {}\n", file_path);

        let text = run_ocr(file_path);

        if text.trim().is_empty() {
            println!("No text found in {}\n", file_path);
            continue;
        }

        let vendor = extract_vendor(&text);
        let amount = extract_amount(&text).unwrap_or("Not found".into());
        let date = extract_date(&text).unwrap_or("Not found".into());

        println!("Vendor : {}", vendor);
        println!("Amount : {}", amount);
        println!("Date   : {}\n", date);

        // Save to CSV
        all_output.push_str(&format!(
            "{},{},{},{}\n",
            file_path, vendor, amount, date
        ));

        // Save to DB
        let expense = Expense {
            file: file_path.to_string(),
            vendor: vendor.clone(),
            amount: amount.clone(),
            date: date.clone(),
        };

        vault::insert_expense(&conn, &expense).unwrap();
    }

    // Write CSV file
    fs::write("results.csv", format!("file,vendor,amount,date\n{}", all_output))
        .expect("Could not write results.csv");

    println!("\n✅ All results saved to results.csv");
    println!("✅ Data stored in vault.db");
}