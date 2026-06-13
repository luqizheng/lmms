pub mod sqlite;
pub mod mysql;
pub mod dameng;
pub mod mssql;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialectKind {
    Mysql,
    Dameng,
    Mssql,
    Sqlite,
}

pub trait Paginate {
    fn paginate(&self, limit: u32, offset: u32) -> String;
}

impl Paginate for DialectKind {
    fn paginate(&self, limit: u32, offset: u32) -> String {
        match self {
            DialectKind::Mysql | DialectKind::Dameng | DialectKind::Sqlite =>
                format!("LIMIT {} OFFSET {}", limit, offset),
            DialectKind::Mssql =>
                format!("OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", offset, limit),
        }
    }
}

impl DialectKind {
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("sqlite:") { Self::Sqlite }
        else if url.starts_with("dameng:") { Self::Dameng }
        else if url.starts_with("mssql:") || url.starts_with("sqlserver:") { Self::Mssql }
        else { Self::Mysql }
    }
    pub fn now_expr(&self) -> &'static str {
        match self {
            Self::Mssql => "SYSUTCDATETIME()",
            Self::Sqlite => "CURRENT_TIMESTAMP",
            _ => "UTC_TIMESTAMP()",
        }
    }
}