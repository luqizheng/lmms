pub mod dameng;
pub mod mysql;
pub mod sqlite;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialectKind {
    Mysql,
    Dameng,
    Sqlite,
}

pub trait Paginate {
    fn paginate(&self, limit: u32, offset: u32) -> String;
}

impl Paginate for DialectKind {
    fn paginate(&self, limit: u32, offset: u32) -> String {
        format!("LIMIT {} OFFSET {}", limit, offset)
    }
}

impl DialectKind {
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("sqlite:") {
            Self::Sqlite
        } else if url.starts_with("dameng:") {
            Self::Dameng
        } else {
            Self::Mysql
        }
    }
    pub fn now_expr(&self) -> &'static str {
        match self {
            Self::Sqlite => "CURRENT_TIMESTAMP",
            _ => "UTC_TIMESTAMP()",
        }
    }
}
