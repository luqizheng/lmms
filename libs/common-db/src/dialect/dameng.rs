/// 达梦数据库（DM Database）方言
///
/// 达梦数据库兼容 MySQL 协议，但存在以下差异：
/// 1. 自增使用 SEQUENCE 而非 AUTO_INCREMENT
/// 2. 关键字差异（如 USER、SESSION 等）
/// 3. JSON 函数差异
/// 4. 分页与 MySQL 相同

/// 达梦保留字
pub const DAMENG_RESERVED_WORDS: &[&str] = &[
    "USER",       /* 用户相关 */
    "SESSION",    /* 会话相关 */
    "TABLE",      /* 表 */
    "VIEW",       /* 视图 */
    "INDEX",      /* 索引 */
    "SEQUENCE",   /* 序列 */
    "SYNONYM",    /* 同义词 */
    "PROCEDURE",  /* 存储过程 */
    "FUNCTION",   /* 函数 */
    "PACKAGE",    /* 包 */
    "TRIGGER",    /* 触发器 */
    "DATABASE",   /* 数据库 */
    "SCHEMA",     /* 模式 */
    "ROLE",       /* 角色 */
    "GRANT",      /* 授权 */
    "REVOKE",     /* 撤销 */
    "COMMIT",     /* 提交 */
    "ROLLBACK",   /* 回滚 */
    "SAVEPOINT",  /* 保存点 */
    "CONNECT",    /* 连接 */
    "IDENTIFIED", /* 认证 */
    "PROFILE",    /* 配置文件 */
    "DEFAULT",    /* 默认 */
    "TEMP",       /* 临时 */
    "TEMPORARY",  /* 临时 */
];

/// 达梦自增语法
///
/// 与 MySQL 不同，达梦使用序列实现自增：
/// CREATE SEQUENCE seq_name START WITH 1 INCREMENT BY 1;
/// CREATE TABLE t (id INT DEFAULT seq_name.NEXTVAL, ...);
pub mod auto_increment {
    /// 创建序列的 SQL
    pub fn create_sequence(table: &str, column: &str) -> String {
        let seq_name = format!("{}_{}_SEQ", table.to_uppercase(), column.to_uppercase());
        format!(
            "CREATE SEQUENCE {} START WITH 1 INCREMENT BY 1 CACHE 20",
            seq_name
        )
    }

    /// 获取下一个序列值
    pub fn next_val(table: &str, column: &str) -> String {
        let seq_name = format!("{}_{}_SEQ", table.to_uppercase(), column.to_uppercase());
        format!("{}.NEXTVAL", seq_name)
    }

    /// 当前序列值
    pub fn curr_val(table: &str, column: &str) -> String {
        let seq_name = format!("{}_{}_SEQ", table.to_uppercase(), column.to_uppercase());
        format!("{}.CURRVAL", seq_name)
    }
}

/// 达梦分页语法（与 MySQL 兼容）
pub fn paginate_sql(limit: u32, offset: u32) -> String {
    format!("LIMIT {} OFFSET {}", limit, offset)
}

/// 达梦当前时间
pub fn now_expr() -> &'static str {
    "SYSDATE"
}

/// 达梦 UUID 生成
pub fn uuid_sql() -> &'static str {
    "SYS_GUID()"
}

/// 达梦布尔类型映射
pub fn bool_literal(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

/// 达梦 JSON 函数
pub mod json {
    /// 提取 JSON 值
    pub fn extract(json_col: &str, path: &str) -> String {
        // 达梦使用 JSON_VALUE 或 JSON_QUERY
        format!("JSON_VALUE({}, '{}')", json_col, path)
    }

    /// 检查 JSON 路径是否存在
    pub fn exists(json_col: &str, path: &str) -> String {
        format!("JSON_EXISTS({}, '{}')", json_col, path)
    }
}

/// 标识符转义
pub fn escape_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
