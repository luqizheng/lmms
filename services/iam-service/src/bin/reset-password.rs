//! `iam_reset_password`：容器内运维兜底 CLI，重置指定用户的登录密码。
//!
//! 设计（spec：docs/superpowers/specs/2026-11-24-iam-reset-password-cli-design.md）：
//! 二进制随 iam-service 镜像发布，必须 `docker exec` 进入容器执行——入口控制等价
//! 宿主机访问权，不新增网络攻击面。DB 语义与 `SelfServiceSvc::reset_password`
//! 完全一致（argon2 哈希 + 撤销全部 refresh token）。
//!
//! ```text
//! iam_reset_password <username>                  # 随机生成密码并打印
//! iam_reset_password <username> -p <new_password>
//! ```
//!
//! 连接零新增配置：直接读容器内已注入的 `DATABASE_URL`，无需 Redis / 网关 / RabbitMQ。

use anyhow::Context;
use digua_common_crypto::password::hash;
use digua_common_db::{db_execute, db_fetch_optional, init_pool_with};
use nanoid::nanoid;

const USAGE: &str = "\
iam_reset_password — 重置 iam 用户密码（容器内运维工具）

USAGE:
    iam_reset_password <username> [--password <new_password>]

说明:
  - 省略 --password 时自动生成 24 位随机密码并打印（推荐，避免密码进 shell history）;
  - 重置成功即撤销该用户全部 refresh token（与邮件重置流程同语义）;
  - 已登录持有的短期 access JWT 不主动失效，至 exp 自然过期;
  - 需要容器内已注入 DATABASE_URL。";

const MIN_PASSWORD_LEN: usize = 8;
const GENERATED_PASSWORD_LEN: usize = 24;

struct Args {
    username: String,
    password: Option<String>,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut username: Option<String> = None;
    let mut password: Option<String> = None;
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--password" | "-p" => {
                let value = raw.get(i + 1).ok_or("--password 需要一个值")?;
                if password.is_some() {
                    return Err("--password 只能出现一次".into());
                }
                password = Some(value.clone());
                i += 2;
            }
            flag if flag.starts_with('-') => return Err(format!("未知参数: {flag}")),
            word => {
                if username.is_some() {
                    return Err("只能指定一个 username".into());
                }
                username = Some(word.to_string());
                i += 1;
            }
        }
    }
    Ok(Args {
        username: username.ok_or("缺少 username")?,
        password,
    })
}

fn generate_password() -> String {
    nanoid!(GENERATED_PASSWORD_LEN)
}

async fn run(raw: &[String]) -> anyhow::Result<String> {
    let args = parse_args(raw).map_err(|e| anyhow::anyhow!("{e}\n\n{USAGE}"))?;

    let db_url = std::env::var("DATABASE_URL")
        .context("缺少 DATABASE_URL 环境变量（容器内由 compose 注入）")?;
    let pool = init_pool_with(&db_url, 2).await?;

    let row: Option<(String, String)> = db_fetch_optional!(
        &pool,
        (String, String),
        "SELECT id, tenant_id FROM iam_users WHERE username = ? AND deleted_at IS NULL",
        &args.username,
    )?;
    let (user_id, tenant_id) =
        row.ok_or_else(|| anyhow::anyhow!("用户不存在: {}", args.username))?;

    let new_password = args.password.unwrap_or_else(generate_password);
    anyhow::ensure!(
        new_password.chars().count() >= MIN_PASSWORD_LEN,
        "密码过短: 至少 {MIN_PASSWORD_LEN} 个字符"
    );
    let new_hash = hash(&new_password)?;

    let updated = db_execute!(
        &pool,
        "UPDATE iam_users SET password_hash = ?, password_expired_at = NULL, updated_at = NOW() WHERE id = ?",
        &new_hash,
        &user_id,
    )?;
    let revoked = db_execute!(
        &pool,
        "UPDATE iam_refresh_tokens SET revoked = TRUE, revoked_at = NOW() WHERE user_id = ? AND revoked = FALSE",
        &user_id,
    )?;

    let username = args.username;
    Ok(format!(
        "OK 用户 {username} (id={user_id}, tenant={tenant_id}) 密码已重置\n\
         更新行数: {updated}, 撤销 refresh token: {revoked} 条\n\
         新密码: {new_password}",
    ))
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return std::process::ExitCode::SUCCESS;
    }
    match run(&raw).await {
        Ok(msg) => {
            println!("{msg}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("FAILED: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_requires_username() {
        assert!(parse_args(&args(&[])).is_err());
        assert!(parse_args(&args(&["alice"])).is_ok());
    }

    #[test]
    fn parse_with_explicit_password() {
        let parsed = parse_args(&args(&["alice", "--password", "s3cret!pw"])).unwrap();
        assert_eq!(parsed.username, "alice");
        assert_eq!(parsed.password.as_deref(), Some("s3cret!pw"));
    }

    #[test]
    fn parse_rejects_unknown_flag_and_dangling_value() {
        assert!(parse_args(&args(&["alice", "--force"])).is_err());
        assert!(parse_args(&args(&["alice", "--password"])).is_err());
        assert!(parse_args(&args(&["alice", "bob"])).is_err());
    }

    #[test]
    fn generated_password_has_expected_length_and_alphabet() {
        const NANOID_ALPHABET: &str =
            "-0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
        let pw = generate_password();
        assert_eq!(pw.chars().count(), GENERATED_PASSWORD_LEN);
        assert!(pw.chars().all(|c| NANOID_ALPHABET.contains(c)));
    }
}
