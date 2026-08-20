use crate::error::AppError;
use chrono::{DateTime, Utc};
use csv::WriterBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub enum ExportFormat {
    #[serde(rename = "csv")]
    Csv,
    #[serde(rename = "json")]
    Json,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportRequest {
    pub user_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub format: ExportFormat,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub file_name: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

pub trait Exportable {
    fn headers() -> Vec<String>;
    fn to_csv_row(&self) -> Vec<String>;
}

pub fn export_to_csv<T: Exportable>(records: &[T]) -> Result<Vec<u8>, AppError> {
    let mut wtr = WriterBuilder::new()
        .delimiter(b',')
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(vec![]);

    let headers = T::headers();
    wtr.write_record(&headers)
        .map_err(|e| AppError::internal("EXPORT_CSV", e.to_string()))?;

    for record in records {
        let row = record.to_csv_row();
        wtr.write_record(&row)
            .map_err(|e| AppError::internal("EXPORT_CSV", e.to_string()))?;
    }

    wtr.into_inner()
        .map_err(|e| AppError::internal("EXPORT_CSV", e.to_string()))
}

pub fn export_to_json<T: Serialize>(records: &[T]) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec_pretty(records).map_err(|e| AppError::internal("EXPORT_JSON", e.to_string()))
}

pub fn generate_file_name(prefix: &str) -> String {
    let now: DateTime<Utc> = Utc::now();
    format!("{}_{}.csv", prefix, now.format("%Y%m%d_%H%M%S"))
}
