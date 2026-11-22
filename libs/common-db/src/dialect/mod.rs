pub mod mysql;
pub mod pg;
pub mod sqlite;

/// 当前支持的数据库方言：MySQL / PostgreSQL / SQLite。
///
/// Dameng（达梦）与 MSSQL 已移除：Rust 生态没有可维护的达梦驱动方案，
/// MSSQL 暂无客户诉求；不再为它们保留枚举分支、连接池或迁移路径。
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
        format!("LIMIT {limit} OFFSET {offset}")
    }
}

impl DialectKind {
    /// 从连接 URL 识别方言。
    ///
    /// 未知 scheme（`oracle:` / `mssql:` / `dameng:` / 随意字符串 …）返回 `None`，
    /// 由调用方显式报错——**绝不静默回退到 MySQL**。
    /// `mariadb:` 是 MySQL 协议家族的显式 scheme（sqlx MySqlPool 直接支持），归入 Mysql。
    pub fn from_url(url: &str) -> Option<Self> {
        let scheme = url.split_once(':')?.0.to_ascii_lowercase();
        match scheme.as_str() {
            "mysql" | "mariadb" => Some(Self::Mysql),
            "sqlite" => Some(Self::Sqlite),
            "postgres" | "postgresql" => Some(Self::Postgres),
            _ => None,
        }
    }

    pub fn now_expr(&self) -> &'static str {
        match self {
            Self::Mysql => "UTC_TIMESTAMP()",
            Self::Sqlite => "CURRENT_TIMESTAMP",
            Self::Postgres => "now() AT TIME ZONE 'UTC'",
        }
    }

    pub fn escape_identifier(&self, identifier: &str) -> String {
        match self {
            Self::Mysql => format!("`{}`", identifier.replace('`', "``")),
            Self::Postgres | Self::Sqlite => format!("\"{}\"", identifier.replace('"', "\"\"")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_url_rejects_unsupported_schemes() {
        // 已移除的方言与未知 scheme 必须被明确拒绝，不得静默映射为 MySQL。
        assert_eq!(
            DialectKind::from_url("dameng://user:pwd@host:5236/db"),
            None
        );
        assert_eq!(
            DialectKind::from_url("mssql://sa:pwd@host:1433/master"),
            None
        );
        assert_eq!(DialectKind::from_url("oracle://localhost/db"), None);
        assert_eq!(DialectKind::from_url("some-random-scheme://host/db"), None);
        assert_eq!(DialectKind::from_url("not-a-url"), None);
    }
}
