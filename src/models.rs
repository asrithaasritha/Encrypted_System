#[derive(Debug, Clone)]
pub struct Expense {
    pub vendor: String,
    pub amount: String,
    pub date: String,
    pub due_date: Option<String>,
    pub warranty_period: Option<String>,
    pub category: String,
    pub confidence: f32,
    pub source_file: String,
}
