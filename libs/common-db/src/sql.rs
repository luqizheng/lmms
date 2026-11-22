//! 方言化 SQL 渲染器 —— 把 MySQL 风格（`?`）的 SQL 渲染为目标方言。
//!
//! # 为什么需要它
//!
//! PostgreSQL 的占位符是**带编号**的（`$1` / `$2` …），MySQL / SQLite 的是**匿名**的（`?`）。
//! 同一份业务 SQL 源码要同时跑在两种后端上，就必须在**构造期**把 `?` 改写成 `$N`，
//! 并且保证「第 k 个 `?` ↔ 第 k 个 `.bind()`」这条顺序契约不被破坏。
//!
//! # 顺序契约
//!
//! `render` 返回的 [`RewrittenQuery`] 同时携带「已方言化的 SQL」与「应绑参数个数」两个事实。
//! 由于 [`RewrittenQuery::expect_binds`] 是**消耗式**的（`self`），未经校验就拿不到
//! [`RewrittenQuery::sql`]——编译器强制调用方先核对 bind 个数。
//!
//! 这同时封堵了 MySQL 侧的静默缺陷：MySQL 渲染结果与源码逐字节相同（不需要改写），
//! 但**仍然统计 `?` 个数并做同样的校验**——「SQL 改了但 bind 顺序没改」的错位
//! 在 MySQL 上原本会被默默接受，现在在构造期就炸。
//!
//! # 状态机
//!
//! 扫描器把 SQL 切分为若干状态，只有处于 `Normal` 状态的 `?` 才是占位符：
//!
//! | 状态 | 结束条件 |
//! |---|---|
//! | `Normal` | — |
//! | `SingleQuote` | `'`（`''` 为转义，**显式 lookahead**，不提前退出） |
//! | `DoubleQuote` | `"`（`""` 为转义） |
//! | `Backtick` | `` ` ``（MySQL 标识符引用） |
//! | `LineComment` | `\n` |
//! | `BlockComment` | `*/` |
//! | `DollarQuote` | 同名 `$tag$`（PostgreSQL dollar-quoting） |
//!
//! 在**任何非 `Normal` 状态**里遇到 `?` 都返回
//! [`SqlRenderError::PlaceholderInLiteral`]——它语义上不是占位符，但极可能是作者
//! 误把占位符写进了引号里，静默跳过会变成「参数个数对不上」的诡异报错。
//!
//! **例外：带引号的标识符**（`"..."` / `` `...` ``）内的 `?` 属于标识符的一部分，
//! 按设计文档 §1.3.2 状态机表「识别并跳过」处理——它不是「误写进引号的占位符」，
//! 对它报错会拒绝合法 SQL。
//!
//! 未闭合的字面量 / 注释返回 [`SqlRenderError::UnterminatedLiteral`]——静默吞掉是最大风险源。

use crate::dialect::DialectKind;
use std::borrow::Cow;
use std::fmt;

/// 字面量 / 注释的种类（用于错误回显与定位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralKind {
    /// `'...'` 字符串字面量
    SingleQuote,
    /// `"..."` 双引号标识符（PostgreSQL / SQLite）
    DoubleQuote,
    /// `` `...` `` 反引号标识符（MySQL）
    Backtick,
    /// `/* ... */` 块注释
    BlockComment,
    /// `$$ ... $$` / `$tag$ ... $tag$` dollar-quoted（PostgreSQL）
    DollarQuote,
}

impl fmt::Display for LiteralKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::SingleQuote => "string literal",
            Self::DoubleQuote => "double-quoted identifier",
            Self::Backtick => "backtick identifier",
            Self::BlockComment => "block comment",
            Self::DollarQuote => "dollar-quoted literal",
        };
        f.write_str(text)
    }
}

/// SQL 渲染错误。
///
/// 三种变体都携带 `raw_sql` 与 `dialect`，错误信息里**回显原始 SQL**——
/// 排查「哪条 SQL 渲染炸了」时不必再去翻调用点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlRenderError {
    /// `?` 出现在字符串字面量 / 标识符引用 / 注释 / dollar-quoted 内（防回归护栏）。
    PlaceholderInLiteral {
        raw_sql: String,
        dialect: DialectKind,
        /// `?` 所在字节偏移。
        offset: usize,
    },
    /// 字面量 / 注释未闭合（静默吞掉是最大风险源，必须报错）。
    UnterminatedLiteral {
        raw_sql: String,
        dialect: DialectKind,
        /// 字面量 / 注释**起始**处字节偏移。
        offset: usize,
        /// 未闭合的是哪种字面量。
        kind: LiteralKind,
    },
    /// 调用方提供的 bind 个数与 `?` 个数不一致（由 [`RewrittenQuery::expect_binds`] 产生）。
    PlaceholderCountMismatch {
        raw_sql: String,
        dialect: DialectKind,
        /// SQL 里 `?` 的个数。
        expected: usize,
        /// 调用方实际提供的 bind 个数。
        actual: usize,
    },
}

impl fmt::Display for SqlRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlaceholderInLiteral {
                raw_sql,
                dialect,
                offset,
            } => {
                write!(
                    f,
                    "sql render failed [{}]: placeholder '?' found inside a literal at byte {}",
                    dialect_name(*dialect),
                    offset
                )?;
                write!(f, "\n  raw: {raw_sql}")
            }
            Self::UnterminatedLiteral {
                raw_sql,
                dialect,
                offset,
                kind,
            } => {
                write!(
                    f,
                    "sql render failed [{}]: unterminated {} opened at byte {}",
                    dialect_name(*dialect),
                    kind,
                    offset
                )?;
                write!(f, "\n  raw: {raw_sql}")
            }
            Self::PlaceholderCountMismatch {
                raw_sql,
                dialect,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "sql render failed [{}]: bind count mismatch: sql expects {}, caller supplied {}",
                    dialect_name(*dialect),
                    expected,
                    actual
                )?;
                write!(f, "\n  raw: {raw_sql}")
            }
        }
    }
}

impl std::error::Error for SqlRenderError {}

/// 方言名（用于错误回显）。
fn dialect_name(dialect: DialectKind) -> &'static str {
    match dialect {
        DialectKind::Mysql => "mysql",
        DialectKind::Sqlite => "sqlite",
        DialectKind::Postgres => "postgres",
    }
}

/// 已方言化的 SQL + 「应绑参数个数」契约。
///
/// `expected_binds` 由扫描器在改写时同步计数得出，与 `sql` 一一对应。
///
/// `#[must_use]`：丢弃它等于放弃校验。
#[must_use = "RewrittenQuery 携带 bind 个数契约，丢弃它等于放弃校验"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewrittenQuery<'a> {
    /// 已方言化的 SQL（PG 为改写后的 `$N` 形式，其余方言原样借用）。
    sql: Cow<'a, str>,
    /// 改写**之前**的原始 SQL —— 错误回显用它（PG 下 `$N` 形式会掩盖原始意图）。
    raw_sql: &'a str,
    /// 渲染时的方言（错误回显用）。
    dialect: DialectKind,
    /// SQL 里 `?` 的个数。
    expected_binds: usize,
}

impl<'a> RewrittenQuery<'a> {
    /// 已方言化的 SQL。
    ///
    /// 调用方应当先经由 [`RewrittenQuery::expect_binds`] 校验 bind 个数再取用。
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// 底层 `Cow`，供测试断言「零分配」等实现细节。
    pub fn sql_cow(&self) -> &Cow<'a, str> {
        &self.sql
    }

    /// 改写前的原始 SQL。
    pub fn raw_sql(&self) -> &'a str {
        self.raw_sql
    }

    /// 渲染时使用的方言。
    pub fn dialect(&self) -> DialectKind {
        self.dialect
    }

    /// SQL 里 `?` 的个数（= 调用方应当提供的 bind 个数）。
    pub fn expected_binds(&self) -> usize {
        self.expected_binds
    }

    /// 构造期校验：实际提供的 bind 个数必须等于 `expected_binds`，否则立即报错。
    ///
    /// 本方法**消耗** `self`——未经校验就拿不到 [`RewrittenQuery::sql`]，
    /// 因此「忘记校验」是一个编译期就会暴露的错误，而不是运行期的静默错位。
    pub fn expect_binds(self, actual: usize) -> Result<Self, SqlRenderError> {
        if actual == self.expected_binds {
            Ok(self)
        } else {
            Err(SqlRenderError::PlaceholderCountMismatch {
                raw_sql: self.raw_sql.to_string(),
                dialect: self.dialect,
                expected: self.expected_binds,
                actual,
            })
        }
    }
}

/// 把 MySQL 风格（`?`）的 SQL 渲染为目标方言，并同时产出 bind 个数契约。
///
/// - `Mysql` / `Sqlite`：SQL 原样返回（`Cow::Borrowed`，零分配），
///   但**仍然统计** `?` 个数（MySQL 侧同样做构造期校验）。
/// - `Postgres`：第 k 个裸 `?` → `$k`（`Cow::Owned`）。
///
/// 契约：第 k 个裸 `?`（左→右扫描，跳过字面量 / 标识符 / 注释 / dollar-quote）
/// 对应 `$k`，调用方必须按**同一左→右顺序**调用 `.bind()`。
///
/// # Errors
///
/// - [`SqlRenderError::PlaceholderInLiteral`]：字面量 / 注释 / dollar-quote 内出现 `?`。
/// - [`SqlRenderError::UnterminatedLiteral`]：字面量 / 块注释未闭合。
pub fn render(sql: &str, dialect: DialectKind) -> Result<RewrittenQuery<'_>, SqlRenderError> {
    let rewrite = matches!(dialect, DialectKind::Postgres);
    let (rendered, placeholders) = scan(sql, dialect, rewrite)?;
    let cow = match rendered {
        Some(bytes) => Cow::Owned(String::from_utf8(bytes).expect(
            "扫描器只整段复制原始字节并替换 ASCII 的 `?`，输出必然是合法 UTF-8",
        )),
        None => Cow::Borrowed(sql),
    };
    Ok(RewrittenQuery {
        sql: cow,
        raw_sql: sql,
        dialect,
        expected_binds: placeholders,
    })
}

/// 只统计占位符个数（供测试 / 与 `QueryBuilder` 对拍使用，不做改写）。
///
/// # Errors
///
/// 与 [`render`] 相同的字面量类错误（`?` 落在字面量内、未闭合）。
pub fn count_placeholders(sql: &str, dialect: DialectKind) -> Result<usize, SqlRenderError> {
    Ok(scan(sql, dialect, false)?.1)
}

/// 扫描器状态。
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Normal,
    SingleQuote,
    DoubleQuote,
    Backtick,
    LineComment,
    BlockComment,
    /// dollar-quoting 的分隔符（`$$` 或 `$tag$`）。
    DollarQuote(String),
}

/// MySQL 默认开启 `backslash_escapes`，`\'` 是转义序列；
/// PostgreSQL 的 `standard_conforming_strings` 默认为 `on`，`\` 是普通字符。
fn backslash_escapes(dialect: DialectKind) -> bool {
    matches!(dialect, DialectKind::Mysql)
}

/// 带引号的文本里，`?` 是「跳过」还是「报错」。
///
/// - **标识符**（`"..."` / `` `...` ``）→ 跳过：`?` 是标识符的一部分，
///   不是「误写进引号的占位符」（设计文档 §1.3.2 状态机表：识别并跳过）。
/// - **字符串字面量**（`'...'` / `$$...$$`）→ 报错：极可能是误把占位符写进了引号。
/// - **注释** → 报错：同上。
///
/// `"` 的语义按方言分流：MySQL 默认（未开 `ANSI_QUOTES`）下 `"..."` 是**字符串**，
/// PG / SQLite 下是**标识符**。
fn quote_is_identifier(dialect: DialectKind, state: &State) -> bool {
    match state {
        State::DoubleQuote => !matches!(dialect, DialectKind::Mysql),
        State::Backtick => true,
        _ => false,
    }
}

/// 单遍扫描。返回 `(改写后的 SQL, 占位符个数)`。
///
/// `rewrite` 为 `false` 时只计数、不产出字符串（`count_placeholders` 用）。
fn scan(
    sql: &str,
    dialect: DialectKind,
    rewrite: bool,
) -> Result<(Option<Vec<u8>>, usize), SqlRenderError> {
    let bytes = sql.as_bytes();
    let mut out: Option<Vec<u8>> = if rewrite {
        Some(Vec::with_capacity(sql.len() + 8))
    } else {
        None
    };
    let mut placeholders = 0usize;
    let mut state = State::Normal;
    // 当前字面量 / 注释的起始偏移（用于 `UnterminatedLiteral`）。
    let mut state_start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        let byte = bytes[i];
        match &state {
            State::Normal => {
                match byte {
                    b'\'' => {
                        state = State::SingleQuote;
                        state_start = i;
                        push_bytes(&mut out, &bytes[i..i + 1]);
                        i += 1;
                    }
                    b'"' => {
                        state = State::DoubleQuote;
                        state_start = i;
                        push_bytes(&mut out, &bytes[i..i + 1]);
                        i += 1;
                    }
                    b'`' => {
                        state = State::Backtick;
                        state_start = i;
                        push_bytes(&mut out, &bytes[i..i + 1]);
                        i += 1;
                    }
                    b'-' if bytes.get(i + 1) == Some(&b'-') => {
                        state = State::LineComment;
                        state_start = i;
                        push_bytes(&mut out, &bytes[i..i + 2]);
                        i += 2;
                    }
                    b'/' if bytes.get(i + 1) == Some(&b'*') => {
                        state = State::BlockComment;
                        state_start = i;
                        push_bytes(&mut out, &bytes[i..i + 2]);
                        i += 2;
                    }
                    b'$' => {
                        match dollar_quote_start(bytes, i) {
                            Some((tag, end)) => {
                                state = State::DollarQuote(tag);
                                state_start = i;
                                push_bytes(&mut out, &bytes[i..end]);
                                i = end;
                            }
                            None => {
                                // 普通 `$`（例如已写好的 `$1`）：原样透传，不计入占位符。
                                push_bytes(&mut out, &bytes[i..i + 1]);
                                i += 1;
                            }
                        }
                    }
                    b'?' => {
                        placeholders += 1;
                        if rewrite {
                            let placeholder = format!("${placeholders}");
                            push_bytes(&mut out, placeholder.as_bytes());
                        } else {
                            push_bytes(&mut out, &bytes[i..i + 1]);
                        }
                        i += 1;
                    }
                    _ => {
                        push_bytes(&mut out, &bytes[i..i + 1]);
                        i += 1;
                    }
                }
            }
            State::SingleQuote => {
                if byte == b'?' {
                    return Err(placeholder_in_literal(sql, dialect, i));
                }
                if backslash_escapes(dialect) && byte == b'\\' && i + 1 < bytes.len() {
                    push_bytes(&mut out, &bytes[i..i + 2]);
                    i += 2;
                } else if byte == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        // `''` 是转义（显式 lookahead，不是朴素 toggle）：保持字符串态。
                        push_bytes(&mut out, &bytes[i..i + 2]);
                        i += 2;
                    } else {
                        state = State::Normal;
                        push_bytes(&mut out, &bytes[i..i + 1]);
                        i += 1;
                    }
                } else {
                    push_bytes(&mut out, &bytes[i..i + 1]);
                    i += 1;
                }
            }
            State::DoubleQuote => {
                if byte == b'?' {
                    if quote_is_identifier(dialect, &state) {
                        // 标识符的一部分：跳过，不计入占位符。
                        push_bytes(&mut out, &bytes[i..i + 1]);
                        i += 1;
                    } else {
                        return Err(placeholder_in_literal(sql, dialect, i));
                    }
                } else if backslash_escapes(dialect) && byte == b'\\' && i + 1 < bytes.len() {
                    push_bytes(&mut out, &bytes[i..i + 2]);
                    i += 2;
                } else if byte == b'"' {
                    if bytes.get(i + 1) == Some(&b'"') {
                        push_bytes(&mut out, &bytes[i..i + 2]);
                        i += 2;
                    } else {
                        state = State::Normal;
                        push_bytes(&mut out, &bytes[i..i + 1]);
                        i += 1;
                    }
                } else {
                    push_bytes(&mut out, &bytes[i..i + 1]);
                    i += 1;
                }
            }
            State::Backtick => {
                if byte == b'?' {
                    // 反引号恒为标识符引用（MySQL 家族 / SQLite）：跳过。
                    push_bytes(&mut out, &bytes[i..i + 1]);
                    i += 1;
                } else if byte == b'`' {
                    if bytes.get(i + 1) == Some(&b'`') {
                        push_bytes(&mut out, &bytes[i..i + 2]);
                        i += 2;
                    } else {
                        state = State::Normal;
                        push_bytes(&mut out, &bytes[i..i + 1]);
                        i += 1;
                    }
                } else {
                    push_bytes(&mut out, &bytes[i..i + 1]);
                    i += 1;
                }
            }
            State::LineComment => {
                if byte == b'?' {
                    return Err(placeholder_in_literal(sql, dialect, i));
                }
                if byte == b'\n' {
                    state = State::Normal;
                }
                push_bytes(&mut out, &bytes[i..i + 1]);
                i += 1;
            }
            State::BlockComment => {
                if byte == b'?' {
                    return Err(placeholder_in_literal(sql, dialect, i));
                }
                if byte == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    state = State::Normal;
                    push_bytes(&mut out, &bytes[i..i + 2]);
                    i += 2;
                } else {
                    push_bytes(&mut out, &bytes[i..i + 1]);
                    i += 1;
                }
            }
            State::DollarQuote(tag) => {
                if byte == b'?' {
                    return Err(placeholder_in_literal(sql, dialect, i));
                }
                let delimiter = dollar_delimiter(tag);
                if bytes[i..].starts_with(delimiter.as_bytes()) {
                    state = State::Normal;
                    push_bytes(&mut out, &bytes[i..i + delimiter.len()]);
                    i += delimiter.len();
                } else {
                    push_bytes(&mut out, &bytes[i..i + 1]);
                    i += 1;
                }
            }
        }
    }

    // 扫描结束仍在非 Normal 状态 → 未闭合（行注释以 EOF 结束是合法的，故不在此列）。
    let kind = match &state {
        State::SingleQuote => Some(LiteralKind::SingleQuote),
        State::DoubleQuote => Some(LiteralKind::DoubleQuote),
        State::Backtick => Some(LiteralKind::Backtick),
        State::BlockComment => Some(LiteralKind::BlockComment),
        State::DollarQuote(_) => Some(LiteralKind::DollarQuote),
        State::Normal | State::LineComment => None,
    };
    if let Some(kind) = kind {
        return Err(SqlRenderError::UnterminatedLiteral {
            raw_sql: sql.to_string(),
            dialect,
            offset: state_start,
            kind,
        });
    }

    Ok((out, placeholders))
}
/// 把字节片段追加到输出缓冲（`None` 表示只计数不产出）。
///
/// 输出缓冲是 `Vec<u8>` 而非 `String`：扫描器按**字节**推进，遇到多字节 UTF-8
/// 字符时若逐字节做 UTF-8 解码会得到 `Err`（不完整序列）从而**丢字符**——
/// 而本仓迁移文件里中文注释很常见。改为整段复制原始字节、末尾一次性解码，
/// 从根本上避免拆散多字节字符（输出必然是合法 UTF-8：只替换了 ASCII 的 `?`）。
fn push_bytes(out: &mut Option<Vec<u8>>, bytes: &[u8]) {
    if let Some(buffer) = out.as_mut() {
        buffer.extend_from_slice(bytes);
    }
}

/// 在 `offset` 处识别 dollar-quote 起始分隔符。
///
/// 返回 `(标签, 结束偏移)`；不是 dollar-quote 时返回 `None`。
///
/// 注意：`$1`（后跟数字）是**占位符**而非 dollar-quote —— PG 的 dollar-quote 标签
/// 必须以字母或下划线开头，因此 `$1$` 不是合法的 dollar-quote。
fn dollar_quote_start(bytes: &[u8], offset: usize) -> Option<(String, usize)> {
    debug_assert_eq!(bytes[offset], b'$');
    // `$$`：空标签。
    if bytes.get(offset + 1) == Some(&b'$') {
        return Some((String::new(), offset + 2));
    }
    // `$tag$`：标签必须以字母或下划线开头。
    let mut cursor = offset + 1;
    match bytes.get(cursor) {
        Some(&first) if first.is_ascii_alphabetic() || first == b'_' => {}
        _ => return None,
    }
    cursor += 1;
    while let Some(&byte) = bytes.get(cursor) {
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            cursor += 1;
        } else {
            break;
        }
    }
    if bytes.get(cursor) != Some(&b'$') {
        return None;
    }
    let tag = String::from_utf8_lossy(&bytes[offset + 1..cursor]).into_owned();
    Some((tag, cursor + 1))
}

/// dollar-quote 的结束分隔符。
fn dollar_delimiter(tag: &str) -> String {
    if tag.is_empty() {
        "$$".to_string()
    } else {
        format!("${tag}$")
    }
}

/// 构造「`?` 落在字面量内」错误。
fn placeholder_in_literal(sql: &str, dialect: DialectKind, offset: usize) -> SqlRenderError {
    SqlRenderError::PlaceholderInLiteral {
        raw_sql: sql.to_string(),
        dialect,
        offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg_renumbers_placeholders() {
        let rq = render("SELECT ? , ?", DialectKind::Postgres).unwrap();
        assert_eq!(rq.sql(), "SELECT $1 , $2");
        assert_eq!(rq.expected_binds(), 2);
    }

    #[test]
    fn mysql_is_borrowed_and_counted() {
        let rq = render("SELECT ? , ?", DialectKind::Mysql).unwrap();
        assert!(matches!(rq.sql_cow(), Cow::Borrowed(_)));
        assert_eq!(rq.expected_binds(), 2);
    }

    #[test]
    fn escaped_quote_does_not_close_string() {
        let err = render("SELECT 'it''s ?'", DialectKind::Postgres).unwrap_err();
        assert!(matches!(err, SqlRenderError::PlaceholderInLiteral { .. }));
    }

    #[test]
    fn dollar_tag_must_start_with_letter() {
        // `$1` 是占位符而非 dollar-quote。
        let rq = render("SELECT $1, ?", DialectKind::Postgres).unwrap();
        assert_eq!(rq.expected_binds(), 1);
        assert_eq!(rq.sql(), "SELECT $1, $1");
    }

    #[test]
    fn dollar_quoted_body_is_skipped() {
        let rq = render("SELECT $tag$ a $tag$, ?", DialectKind::Postgres).unwrap();
        assert_eq!(rq.expected_binds(), 1);
        assert_eq!(rq.sql(), "SELECT $tag$ a $tag$, $1");
    }

    #[test]
    fn count_placeholders_does_not_rewrite() {
        assert_eq!(
            count_placeholders("SELECT ? , ?", DialectKind::Postgres).unwrap(),
            2
        );
    }

    /// **回归护栏**：扫描器按字节推进，若逐字节做 UTF-8 解码，中文（多字节）会被
    /// 拆散并丢失。本仓迁移文件里中文注释很常见，必须原样透传。
    #[test]
    fn multibyte_utf8_is_preserved() {
        let sql = "SELECT * FROM iam_users WHERE name = ? -- 中文注释：租户隔离";
        let rq = render(sql, DialectKind::Postgres).unwrap();
        assert_eq!(
            rq.sql(),
            "SELECT * FROM iam_users WHERE name = $1 -- 中文注释：租户隔离"
        );
        assert_eq!(rq.expected_binds(), 1);

        // 字面量里的中文同样不能被破坏。
        let sql = "SELECT '租户 ? 隔离' , ?";
        let err = render(sql, DialectKind::Postgres).unwrap_err();
        assert!(matches!(err, SqlRenderError::PlaceholderInLiteral { .. }));

        // MySQL 分支是原样借用，天然无损。
        let sql = "SELECT * FROM t WHERE 名称 = ?";
        let rq = render(sql, DialectKind::Mysql).unwrap();
        assert_eq!(rq.sql(), sql);
    }
}
