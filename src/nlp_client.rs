use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct TextInput {
    pub text: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct NLPResult {
    pub vendor: Option<String>,
    pub amount: Option<String>,
    pub date: Option<String>,
    pub due_date: Option<String>,        // ✅ ADD THIS
    pub warranty_period: Option<String>, // ✅ ADD THIS
    pub category: String,
    pub confidence: f32,
}
pub struct NLPClient {
    base_url: String,
}

impl NLPClient {
    pub fn new() -> Self {
        Self {
            base_url: "http://127.0.0.1:8765".to_string(),
        }
    }

    pub fn analyze(&self, text: &str) -> NLPResult {
        let payload = TextInput {
            text: text.to_string(),
        };

        let response = ureq::post(&format!("{}/extract", self.base_url))
            .send_json(ureq::json!(payload));

        match response {
            Ok(res) => res.into_json().unwrap_or_default(),
            Err(_) => {
                println!("⚠ NLP server not running, using fallback");
                NLPResult::default()
            }
        }
    }
}