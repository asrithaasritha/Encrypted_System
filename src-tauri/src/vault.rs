use rusqlite::{Connection, Result};

use crate::models::Expense;
use crate::crypto;

pub struct Vault {
    conn: Connection,
    key: [u8; 32],
}

impl Vault {
    pub fn open(path: &str, password: &str) -> Result<Self> {
        let key = crypto::derive_key(password);
        let conn = Connection::open(path)?;

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
        )?;

        Ok(Self { conn, key })
    }

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

    pub fn save_expense(&self, exp: &Expense) -> Result<i64> {
        self.insert_expense(exp)?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_expenses(&self) -> Result<Vec<Expense>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, vendor, amount, date, due_date, warranty_period, category, confidence, source_file
             FROM expenses
             ORDER BY id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let id = row.get::<_, i64>(0)?;
            let enc_vendor = row.get::<_, Vec<u8>>(1)?;
            let enc_amount = row.get::<_, Vec<u8>>(2)?;
            let enc_date = row.get::<_, Vec<u8>>(3)?;

            let vendor = String::from_utf8(crypto::decrypt_blob(&self.key, &enc_vendor))
                .unwrap_or_else(|_| "Unknown vendor".to_string());
            let amount = String::from_utf8(crypto::decrypt_blob(&self.key, &enc_amount))
                .unwrap_or_else(|_| "0".to_string());
            let date = String::from_utf8(crypto::decrypt_blob(&self.key, &enc_date))
                .unwrap_or_else(|_| "".to_string());

            Ok(Expense {
                id: Some(id),
                vendor,
                amount,
                date,
                due_date: row.get::<_, Option<String>>(4)?,
                warranty_period: row.get::<_, Option<String>>(5)?,
                category: row.get::<_, String>(6)?,
                confidence: row.get::<_, f32>(7)?,
                source_file: row.get::<_, String>(8)?,
            })
        })?;

        rows.collect()
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