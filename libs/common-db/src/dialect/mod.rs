pub mod dameng;
pub mod mssql;
pub mod mysql;
pub mod pg;
pub mod sqlite;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialectKind {
    Mysql,
    Dameng,
    Sqlite,
    Mssql,
    Postgres,
}

pub trait Paginate {
    fn paginate(&self, limit: u32, offset: u32) -> String;
}

impl Paginate for DialectKind {
    fn paginate(&self, limit: u32, offset: u32) -> String {
        match self {
            Self::Mssql => format!("OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", offset, limit),
            _ => format!("LIMIT {} OFFSET {}", limit, offset),
        }
    }
}

impl DialectKind {
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("sqlite:") {
            Self::Sqlite
        } else if url.starts_with("dameng:") {
            Self::Dameng
        } else if url.starts_with("mssql:") {
            Self::Mssql
        } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
            Self::Postgres
        } else {
            Self::Mysql
        }
    }

    pub fn now_expr(&self) -> &'static str {
        match self {
            Self::Sqlite => "CURRENT_TIMESTAMP",
            Self::Mssql => "GETUTCDATE()",
            Self::Dameng => "SYSDATE",
            Self::Postgres => "now() AT TIME ZONE 'UTC'",
            Self::Mysql => "UTC_TIMESTAMP()",
        }
    }

    pub fn escape_identifier(&self, identifier: &str) -> String {
        match self {
            Self::Mssql => format!("[{}]", identifier.replace(']', "]]")),
            Self::Dameng => format!("\"{}\"", identifier.replace('"', "\"\"")),
            Self::Mysql => format!("`{}`", identifier.replace('`', "``")),
            Self::Postgres | Self::Sqlite => format!("\"{}\"", identifier.replace('"', "\"\"")),
        }
    }

    pub fn is_reserved_keyword(&self, word: &str) -> bool {
        let upper = word.to_uppercase();
        match self {
            Self::Mssql => matches!(
                upper.as_str(),
                "USER" | "GROUP" | "ORDER" | "TABLE" | "SELECT" | "WHERE" | "FROM"
            ),
            Self::Dameng => matches!(
                upper.as_str(),
                "USER" | "SESSION" | "TABLE" | "VIEW" | "INDEX"
            ),
            Self::Postgres => pg::PG_RESERVED_WORDS.contains(&upper.as_str()),
            _ => false,
        }
    }
}
