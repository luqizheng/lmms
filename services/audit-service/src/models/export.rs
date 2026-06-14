use crate::error::AppResult;
use chrono::{DateTime, Utc};
use csv::Writer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Excel,
    Json,
}

impl ExportFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "excel" | "xlsx" => Some(Self::Excel),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub user_id: Option<String>,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub include_fields: Vec<String>,
}

impl Default for ExportRequest {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            start_date: None,
            end_date: None,
            user_id: None,
            action: None,
            resource: None,
            include_fields: vec![
                "id".to_string(),
                "user_id".to_string(),
                "action".to_string(),
                "resource".to_string(),
                "ip_address".to_string(),
                "user_agent".to_string(),
                "created_at".to_string(),
            ],
        }
    }
}

pub struct ComplianceExporter {
    timezone: String,
}

impl ComplianceExporter {
    pub fn new(timezone: impl Into<String>) -> Self {
        Self {
            timezone: timezone.into(),
        }
    }

    pub fn export_csv<T: Exportable>(&self, records: &[T]) -> AppResult<String> {
        let mut wtr = Writer::from_writer(vec![]);
        
        if let Some(first) = records.first() {
            wtr.write_record(first.headers())?;
        }
        
        for record in records {
            wtr.write_record(record.to_csv_row())?;
        }
        
        let data = wtr.into_inner()
            .map_err(|e| crate::error::AppError::internal("EXPORT_CSV", e.to_string()))?;
        
        String::from_utf8(data)
            .map_err(|e| crate::error::AppError::internal("EXPORT_UTF8", e.to_string()))
    }

    pub fn export_json<T: serde::Serialize>(&self, records: &[T]) -> AppResult<String> {
        serde_json::to_string_pretty(records)
            .map_err(|e| crate::error::AppError::internal("EXPORT_JSON", e.to_string()))
    }
}

pub trait Exportable {
    fn headers() -> Vec<String>;
    fn to_csv_row(&self) -> Vec<String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::OperationLog;

    #[test]
    fn test_export_format_parsing() {
        assert_eq!(ExportFormat::from_str("csv"), Some(ExportFormat::Csv));
        assert_eq!(ExportFormat::from_str("xlsx"), Some(ExportFormat::Excel));
        assert_eq!(ExportFormat::from_str("json"), Some(ExportFormat::Json));
        assert_eq!(ExportFormat::from_str("pdf"), None);
    }

    #[test]
    fn test_export_request_default() {
        let req = ExportRequest::default();
        assert_eq!(req.format, ExportFormat::Csv);
        assert!(req.include_fields.contains(&"user_id".to_string()));
    }
}
