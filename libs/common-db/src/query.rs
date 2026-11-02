//! 方言感知查询宏 —— 业务层**唯一**的 SQL 入口。
//!
//! # 为什么是宏，而不是函数
//!
//! `sqlx::QueryBuilder::push_bind` 会在**运行时**按后端产出 `?` 或 `$N`，但
//! `sqlx::query_as("... ? ...")` 传的是**静态字符串**——它不会自动改写占位符。
//! 因此「同一份 SQL 源码跑两种后端」这件事，必须在**构造期**把 `?` 改写成 `$N`，
//! 而改写需要知道「第 k 个 `?` 对应第 k 个 `.bind()`」——这正是宏能提供、
//! 函数提供不了的信息（宏在展开处**看得见** bind 实参的个数与书写顺序）。
//!
//! # 顺序契约
//!
//! 宏展开后是 `match pool { ... }`，每个后端分支各自 `render` + `bind`：
//!
//! - `?` 的第 k 次出现 ↔ 第 k 个 bind 实参（**同一次宏展开内生成**，不可能错位）。
//! - `render` 之后立刻 `expect_binds(...)` 做**构造期**校验，**MySQL 分支同样校验**
//!   ——这是「MySQL 静默接受编号错位」的唯一封堵点。
//!
//! # 调用形态
//!
//! 宏表达式求值为 `Result<T, sqlx::Error>`（不是 Future——`.await` 已在宏内部完成）。
//! 调用方既可以 `?` 传播，也可以 `.expect()` / `match` 自行处理：
//!
//! ```ignore
//! let rows: Vec<User> = db_fetch_all!(pool, User, "SELECT * FROM iam_users WHERE id = ?", id)?;
//! ```
//!
//! ## `$pool` 可以传**拥有值**，也可以传**引用**
//!
//! 宏内部统一写 `match &$pool`（不是 `match $pool`），因此下列三种写法都成立：
//!
//! ```ignore
//! db_fetch_all!(&pool, ...)?;                                  // 借用
//! db_fetch_all!(pool, ...)?;                                   // 借用（pool: &DbPool）
//! db_fetch_all!(connect_pool(url, 8).await?, ...)?;             // 拥有值
//! db_fetch_all!(state.db, ...)?;                               // 拥有值（字段）
//! ```
//!
//! **为什么不能写 `match $pool`**：sqlx 只为 `&Pool<DB>` 实现 `Executor`
//! （`impl<'p, DB: Database> Executor<'p> for &'_ Pool<DB>`），拥有值的
//! `Pool<DB>` **不实现**。`match $pool` 在 `$pool` 是拥有值时按值绑定出
//! `Pool<DB>` → `error[E0277]: the trait bound `Pool<Postgres>: Executor<'_>` is not satisfied`，
//! 而该错误信息完全不会提示「你该传引用」。`match &$pool` 借 match ergonomics
//! 对拥有值/借用值一致地得到 `&Pool<DB>`，**零运行时成本**。
//!
//! ## `$sql` 可以传 `&str`，也可以传 `String` / `format!()`
//!
//! 宏先把 `$sql` 绑到一个局部量再取引用，因此 inline 临时量不会提前 drop
//! （`.await` 发生在**同一个块内**，不要用 `async move { .. }.await` 把分支包起来，
//! 否则 inline `format!()` 会在 await 之前 drop，触发 E0716）。
//!
//! # feature 约定（重要）
//!
//! 宏内部的 `#[cfg(feature = "...")]` 在**调用方 crate** 求值（`cfg` 的求值主体
//! 永远是「展开发生的 crate」）。因此：
//!
//! 1. 调用方 crate 必须定义与本库**同名**的 `mysql` / `postgres` / `sqlite` feature；
//! 2. 调用方必须把它们**转发**到本库（`mysql = ["digua_common_db/mysql", ...]`），
//!    否则「宏臂存在但库变体不存在」会在编译期报错（这是刻意的：配置错误必须响亮失败）。
//!
//! 本库自己作为调用方时，`cargo test -p digua_common_db --features postgres` 天然满足
//! 这一约定——集成测试与库同属一个 package，feature 集合一致。

/// 内部辅助宏：渲染 SQL 并核对 bind 个数。
///
/// 抽出来是为了让 5 个 `db_*` 宏的「渲染 + 校验」部分逐字一致——
/// 顺序契约的防线不允许出现「某个宏忘了校验」的分叉。
#[doc(hidden)]
#[macro_export]
macro_rules! __db_prepare {
    ($dialect:expr, $sql:expr, $n_binds:expr) => {
        $crate::sql::render($sql, $dialect).and_then(|rq| rq.expect_binds($n_binds))
    };
}

/// 读取多行。
#[macro_export]
macro_rules! db_fetch_all {
    ($pool:expr, $ty:ty, $sql:expr $(, $bind:expr)* $(,)?) => {{
        // 先绑局部量再取引用：支持 `&str` / `String` / inline `format!()`（临时量随块存活）。
        let __sql_val = $sql;
        let __sql_tpl: &str = &__sql_val;
        // 构造期计数：宏能「看到」bind 实参的个数。
        let __n_binds: usize = 0usize $(+ { let _ = &$bind; 1usize })*;
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Mysql,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::MySql, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_all(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Postgres,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::Postgres, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_all(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Sqlite,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::Sqlite, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_all(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}

/// 读取单行（必须有行，否则 `RowNotFound`）。
#[macro_export]
macro_rules! db_fetch_one {
    ($pool:expr, $ty:ty, $sql:expr $(, $bind:expr)* $(,)?) => {{
        // 先绑局部量再取引用：支持 `&str` / `String` / inline `format!()`（临时量随块存活）。
        let __sql_val = $sql;
        let __sql_tpl: &str = &__sql_val;
        let __n_binds: usize = 0usize $(+ { let _ = &$bind; 1usize })*;
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Mysql,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::MySql, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_one(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Postgres,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::Postgres, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_one(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Sqlite,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::Sqlite, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_one(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}

/// 读取单行（可无行 → `Ok(None)`）。
#[macro_export]
macro_rules! db_fetch_optional {
    ($pool:expr, $ty:ty, $sql:expr $(, $bind:expr)* $(,)?) => {{
        // 先绑局部量再取引用：支持 `&str` / `String` / inline `format!()`（临时量随块存活）。
        let __sql_val = $sql;
        let __sql_tpl: &str = &__sql_val;
        let __n_binds: usize = 0usize $(+ { let _ = &$bind; 1usize })*;
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Mysql,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::MySql, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_optional(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Postgres,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::Postgres, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_optional(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Sqlite,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_as::<$crate::sqlx::Sqlite, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_optional(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}

/// 标量查询（`COUNT(*)` / `EXISTS` 等）。
#[macro_export]
macro_rules! db_fetch_scalar {
    ($pool:expr, $ty:ty, $sql:expr $(, $bind:expr)* $(,)?) => {{
        // 先绑局部量再取引用：支持 `&str` / `String` / inline `format!()`（临时量随块存活）。
        let __sql_val = $sql;
        let __sql_tpl: &str = &__sql_val;
        let __n_binds: usize = 0usize $(+ { let _ = &$bind; 1usize })*;
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Mysql,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_scalar::<$crate::sqlx::MySql, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_one(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Postgres,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_scalar::<$crate::sqlx::Postgres, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_one(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Sqlite,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q =
                            $crate::sqlx::query_scalar::<$crate::sqlx::Sqlite, $ty>(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.fetch_one(p).await
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}

/// 写操作（INSERT / UPDATE / DELETE），返回影响行数。
#[macro_export]
macro_rules! db_execute {
    ($pool:expr, $sql:expr $(, $bind:expr)* $(,)?) => {{
        // 先绑局部量再取引用：支持 `&str` / `String` / inline `format!()`（临时量随块存活）。
        let __sql_val = $sql;
        let __sql_tpl: &str = &__sql_val;
        let __n_binds: usize = 0usize $(+ { let _ = &$bind; 1usize })*;
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Mysql,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q = $crate::sqlx::query(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.execute(p).await.map(|r| r.rows_affected())
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Postgres,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q = $crate::sqlx::query(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.execute(p).await.map(|r| r.rows_affected())
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                match $crate::__db_prepare!(
                    $crate::dialect::DialectKind::Sqlite,
                    __sql_tpl,
                    __n_binds
                ) {
                    Ok(__rq) => {
                        let mut __q = $crate::sqlx::query(__rq.sql());
                        $( __q = __q.bind($bind); )*
                        __q.execute(p).await.map(|r| r.rows_affected())
                    }
                    Err(__e) => Err($crate::sqlx::Error::Configuration(Box::new(__e))),
                }
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}

/// 动态 SQL（条件拼装）场景：把同一份 builder body 在各后端展开一次。
///
/// `push_bind` 由 sqlx 自动方言化（PG 产出 `$1..$N`），因此**不走** [`crate::sql::render`]。
#[macro_export]
macro_rules! db_qb_fetch_all {
    ($pool:expr, $ty:ty, |$qb:ident| $body:block) => {{
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::MySql>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_all(p).await
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::Postgres>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_all(p).await
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::Sqlite>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_all(p).await
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}

/// 动态 SQL：读取单行（可无行 → `Ok(None)`）。
#[macro_export]
macro_rules! db_qb_fetch_optional {
    ($pool:expr, $ty:ty, |$qb:ident| $body:block) => {{
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::MySql>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_optional(p).await
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::Postgres>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_optional(p).await
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::Sqlite>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_optional(p).await
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}

/// 动态 SQL：读取单行（必须有行）。
#[macro_export]
macro_rules! db_qb_fetch_one {
    ($pool:expr, $ty:ty, |$qb:ident| $body:block) => {{
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::MySql>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_one(p).await
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::Postgres>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_one(p).await
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::Sqlite>::new("");
                $body
                $qb.build_query_as::<$ty>().fetch_one(p).await
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}

/// 动态 SQL：写操作，返回影响行数。
#[macro_export]
macro_rules! db_qb_execute {
    ($pool:expr, |$qb:ident| $body:block) => {{
        match &$pool {
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::MySql>::new("");
                $body
                $qb.build().execute(p).await.map(|r| r.rows_affected())
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::Postgres>::new("");
                $body
                $qb.build().execute(p).await.map(|r| r.rows_affected())
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                let mut $qb = $crate::sqlx::QueryBuilder::<$crate::sqlx::Sqlite>::new("");
                $body
                $qb.build().execute(p).await.map(|r| r.rows_affected())
            }
            #[allow(unreachable_patterns)]
            _ => Err($crate::sqlx::Error::Configuration(
                "db dialect not enabled for this crate".into(),
            )),
        }
    }};
}
