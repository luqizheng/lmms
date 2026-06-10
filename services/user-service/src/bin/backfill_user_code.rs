// TODO 子 plan A 实施：循环更新 member_user SET UserCode = <NanoID> WHERE UserCode = '' OR UserCode IS NULL
// 实施时使用 nanoid 0.4 crate 在 Rust 端生成 8 位 NanoID，保证唯一性
fn main() {
    println!("backfill_user_code: stub (see 子 plan A)");
}
