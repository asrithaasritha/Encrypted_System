use rusqlite::{Connection, Result};

use crate::models::Expense;
use crate::crypto;

pub struct Vault {
    conn: Connection,
    key: [u8; 32],
}

impl Vault {
    // 🔐 INIT DB
    pub fn new(password: &str) -> Self {
        let key = crypto::derive_key(password);
        let conn = Connection::open("vault.db").unwrap();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS expenses (
                id INTEGER PRIMARY KEY,
                vendor BLOB,
                amount BLOB,
                date BLOB,
                due_date TEXT,
                warranty_period TEXT,
                category TEXT,
                confidence REAL,
                source_file TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                expense_id INTEGER REFERENCES expenses(id),
                message TEXT NOT NULL,
                remind_on TEXT NOT NULL,
                done INTEGER DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now'))
            );
            ",
        ).unwrap();

        Self { conn, key }
    }

    // 🔐 expose connection (REQUIRED STEP)
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // 🔐 expose key (needed for decryption)
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }

    // 🔐 INSERT EXPENSE
    pub fn insert_expense(&self, exp: &Expense) -> Result<()> {
        let enc_vendor = crypto::encrypt_blob(&self.key, exp.vendor.as_bytes());
        let enc_amount = crypto::encrypt_blob(&self.key, exp.amount.as_bytes());
        let enc_date = crypto::encrypt_blob(&self.key, exp.date.as_bytes());

        self.conn.execute(
            "INSERT INTO expenses 
            (vendor, amount, date, due_date, warranty_period, category, confidence, source_file)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                &enc_vendor,
                &enc_amount,
                &enc_date,
                &exp.due_date,
                &exp.warranty_period,
                &exp.category,
                &exp.confidence,
                &exp.source_file,
            ),
        )?;

        Ok(())
    }

    // ➕ create reminder
    pub fn create_reminder(
        &self,
        expense_id: i64,
        message: &str,
        remind_on: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO reminders (expense_id, message, remind_on)
             VALUES (?1, ?2, ?3)",
            (expense_id, message, remind_on),
        )?;

        Ok(())
    }
}