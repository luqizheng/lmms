pub mod mysql;
pub mod pg;
pub mod sqlite;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialectKind {
    Mysql,
    Sqlite,
    Postgres,
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
    /// Detect a supported database dialect from its connection URL.
    ///
    /// Unknown schemes are rejected instead of silently defaulting to MySQL.
    pub fn from_url(url: &str) -> Option<Self> {
        let scheme = url.split_once(':')?.0.to_ascii_lowercase();
        match scheme.as_str() {
            "mysql" => Some(Self::Mysql),
            "sqlite" => Some(Self::Sqlite),
            "postgres" | "postgresql" => Some(Self::Postgres),
            _ => None,
        }
    }

    pub fn now_expr(&self) -> &'static str {
        match self {
            Self::Sqlite => "CURRENT_TIMESTAMP",
            Self::Postgres => "now() AT TIME ZONE 'UTC'",
            Self::Mysql => "UTC_TIMESTAMP()",
        }
    }

    pub fn escape_identifier(&self, identifier: &str) -> String {
        match self {
            Self::Mysql => format!("\x60{}\x60", identifier.replace('\x60', "\x60\x60")),
            Self::Postgres | Self::Sqlite => {
                format!("\"{}\"", identifier.replace('"', "\"\""))
            }
        }
    }

    pub fn is_reserved_keyword(&self, word: &str) -> bool {
        let upper = word.to_uppercase();
        match self {
            Self::Postgres => pg::PG_RESERVED_WORDS.contains(&upper.as_str()),
            Self::Mysql | Self::Sqlite => false,
        }
    }
}
