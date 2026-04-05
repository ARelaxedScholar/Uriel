use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum IntentEnum {
    Note,
    Draft,
    Event,
    Disambiguate,
    Query,
    Crawl,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UrielClassification {
    pub intent: IntentEnum,
    pub target_folder: String,
    pub entities_found: Vec<String>,
    pub formatted_content: String,
}
