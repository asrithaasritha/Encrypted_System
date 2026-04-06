use rusqlite::{Connection, Result};
use crate::models::Expense;

pub fn init_db() -> Connection {
    let conn = Connection::open("vault.db").unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS expenses (
            id INTEGER PRIMARY KEY,
            file TEXT,
            vendor TEXT,
            amount TEXT,
            date TEXT
        )",
        [],
    ).unwrap();

    conn
}

pub fn insert_expense(conn: &Connection, expense: &Expense) -> Result<()> {
    conn.execute(
        "INSERT INTO expenses (file, vendor, amount, date)
        VALUES (?1, ?2, ?3, ?4)",
        (
            &expense.file,
            &expense.vendor,
            &expense.amount,
            &expense.date,
        ),
    )?;
    Ok(())
}