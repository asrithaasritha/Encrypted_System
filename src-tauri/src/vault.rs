use rusqlite::{params, Connection, OptionalExtension, Result};

use crate::models::Expense;
use crate::crypto;

pub struct Vault {
    conn: Connection,
    key: [u8; 32],
}

const PASSWORD_VERIFIER_KEY: &str = "password_verifier_v1";
const PASSWORD_VERIFIER_VALUE: &[u8] = b"billvault-password-verifier";

impl Vault {
    pub fn needs_setup(path: &str) -> Result<bool> {
        let conn = Connection::open(path)?;
        Self::init_schema(&conn)?;

        let stored_verifier: Option<Vec<u8>> = conn
            .query_row(
                "SELECT meta_value FROM vault_meta WHERE meta_key = ?1",
                params![PASSWORD_VERIFIER_KEY],
                |row| row.get(0),
            )
            .optional()?;

        Ok(stored_verifier.is_none())
    }

    pub fn open(path: &str, password: &str) -> Result<Self> {
        let key = crypto::derive_key(password);
        let conn = Connection::open(path)?;

        Self::init_schema(&conn)?;
        Self::validate_or_initialize_password_verifier(&conn, &key)?;

        Ok(Self { conn, key })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
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

            CREATE TABLE IF NOT EXISTS vault_meta (
                meta_key TEXT PRIMARY KEY,
                meta_value BLOB NOT NULL
            );
            ",
        )?;

        Ok(())
    }

    fn validate_or_initialize_password_verifier(conn: &Connection, key: &[u8; 32]) -> Result<()> {
        let stored_verifier: Option<Vec<u8>> = conn
            .query_row(
                "SELECT meta_value FROM vault_meta WHERE meta_key = ?1",
                params![PASSWORD_VERIFIER_KEY],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(enc_verifier) = stored_verifier {
            let plain = crypto::decrypt_blob(key, &enc_verifier);
            if plain == PASSWORD_VERIFIER_VALUE {
                return Ok(());
            }

            return Err(rusqlite::Error::InvalidParameterName(
                "invalid vault password".to_string(),
            ));
        }

        // Migration support: old vaults had no verifier row.
        // If there is existing encrypted data, only accept the password
        // if at least one decrypted vendor looks valid.
        let expense_count: i64 = conn.query_row(
            "SELECT COUNT(1) FROM expenses",
            [],
            |row| row.get(0),
        )?;

        if expense_count > 0 {
            let sample_vendor: Option<Vec<u8>> = conn
                .query_row(
                    "SELECT vendor FROM expenses LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(enc_vendor) = sample_vendor {
                let dec = crypto::decrypt_blob(key, &enc_vendor);
                let looks_valid = !dec.is_empty()
                    && std::str::from_utf8(&dec)
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false);

                if !looks_valid {
                    return Err(rusqlite::Error::InvalidParameterName(
                        "invalid vault password".to_string(),
                    ));
                }
            }
        }

        let enc_verifier = crypto::encrypt_blob(key, PASSWORD_VERIFIER_VALUE);
        conn.execute(
            "INSERT OR REPLACE INTO vault_meta (meta_key, meta_value) VALUES (?1, ?2)",
            params![PASSWORD_VERIFIER_KEY, enc_verifier],
        )?;

        Ok(())
    }

    // 🔐 INIT DB
    pub fn new(password: &str) -> Self {
        let key = crypto::derive_key(password);
        let conn = Connection::open("vault.db").unwrap();

        Self::init_schema(&conn).unwrap();
        Self::validate_or_initialize_password_verifier(&conn, &key).unwrap();

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