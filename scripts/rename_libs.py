#!/usr/bin/env python3
"""
rename_libs.py — 把 libs/* + sdk/rust/* crate 的 [package].name 加 digua- 前缀，
并更新所有其他 Cargo.toml 的 path 依赖。

只动：
  - [package].name = "<old>"
  - <old> = { path = "..." }  (Cargo dependency 行)

绝不改：
  - workspace members 路径 (libs/* sdk/rust/* 字面路径)
  - crate 内部 use 语句 (依赖 Rust rename 自动迁移)
  - 注释
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

LIBS = {
    "authz": "digua-authz",
    "common": "digua-common",
    "common-cache": "digua-common-cache",
    "common-config": "digua-common-config",
    "common-crypto": "digua-common-crypto",
    "common-db": "digua-common-db",
    "common-error": "digua-common-error",
    "common-event": "digua-common-event",
    "common-health": "digua-common-health",
    "common-http": "digua-common-http",
    "common-jwt": "digua-common-jwt",
    "common-log": "digua-common-log",
    "common-ratelimit": "digua-common-ratelimit",
    "common-telemetry": "digua-common-telemetry",
    "common-telemetry-axum": "digua-common-telemetry-axum",
    "common-validator": "digua-common-validator",
    "jwt-ctx": "digua-jwt-ctx",
}

SDK_RUST = {
    "audit-service-sdk": "digua-audit-sdk",
    "config-service-sdk": "digua-config-sdk",
    "file-service-sdk": "digua-file-sdk",
    "iam-service-sdk": "digua-iam-sdk",
    "notification-service-sdk": "digua-notification-sdk",
    "token-service-sdk": "digua-token-sdk",
    "kudi-http-client": "digua-http-client",
}

ALL = {**LIBS, **SDK_RUST}

# Rust use 语句里 crate 名是 underscore 形式（hyphen → underscore）
USE_FORM = {k.replace('-', '_'): v.replace('-', '_') for k, v in ALL.items()}

# 按字符串长度倒序匹配，避免 "common" 误吃 "common-error"
SORTED = sorted(ALL.items(), key=lambda kv: -len(kv[0]))
USE_SORTED = sorted(USE_FORM.items(), key=lambda kv: -len(kv[0]))


def rename_package_name(text, old, new):
    pkg_re = re.compile(
        r'(\[package\][^\[]*?^\s*name\s*=\s*")' + re.escape(old) + r'(")',
        re.DOTALL | re.MULTILINE,
    )
    return pkg_re.sub(rf'\g<1>{new}\g<2>', text, count=1)


def rename_dep_line(text, old, new):
    dep_re = re.compile(
        r'(^[\s]*)' + re.escape(old) + r'(\s*=\s*)',
        re.MULTILINE,
    )
    return dep_re.sub(rf'\g<1>{new}\g<2>', text)


def patch_text(text, old, new, *, in_package=False):
    if in_package:
        new_text = rename_package_name(text, old, new)
    else:
        new_text = rename_dep_line(text, old, new)
    return new_text, 1 if new_text != text else 0


def main():
    changes = []

    print("[1/4] 改 [package].name")
    for old, new in LIBS.items():
        p = ROOT / "libs" / old / "Cargo.toml"
        if not p.exists():
            print(f"  ✗ missing: {p}")
            continue
        text = p.read_text(encoding="utf-8")
        new_text, n = patch_text(text, old, new, in_package=True)
        if n:
            p.write_text(new_text, encoding="utf-8")
            print(f"  ✓ libs/{old}: name → {new}")
            changes.append(str(p))

    for old, new in SDK_RUST.items():
        p = ROOT / "sdk" / "rust" / old / "Cargo.toml"
        if not p.exists():
            print(f"  ✗ missing: {p}")
            continue
        text = p.read_text(encoding="utf-8")
        new_text, n = patch_text(text, old, new, in_package=True)
        if n:
            p.write_text(new_text, encoding="utf-8")
            print(f"  ✓ sdk/rust/{old}: name → {new}")
            changes.append(str(p))

    print("\n[2/4] 改 path 依赖")
    cargo_files = [p for p in ROOT.rglob("Cargo.toml")
                   if "/target/" not in str(p)
                   and "/node_modules/" not in str(p)
                   and "/.cargo/registry/" not in str(p)]
    for p in cargo_files:
        text = p.read_text(encoding="utf-8")
        new_text = text
        file_changes = 0
        for old, new in SORTED:
            new_text, n = patch_text(new_text, old, new, in_package=False)
            file_changes += n
        if new_text != text:
            p.write_text(new_text, encoding="utf-8")
            print(f"  ✓ {p.relative_to(ROOT)} ({file_changes} 处)")
            changes.append(str(p))

    print("\n[3/4] 改 .rs 源码的 use 语句")
    rs_files = [p for p in ROOT.rglob("*.rs")
                if "/target/" not in str(p)
                and "/node_modules/" not in str(p)
                and "/.cargo/registry/" not in str(p)]
    for p in rs_files:
        text = p.read_text(encoding="utf-8")
        new_text = text
        file_changes = 0
        for old, new in USE_SORTED:
            pattern = re.compile(
                rf'(^use\s+){re.escape(old)}(?=::|;|\s*\{{)',
                re.MULTILINE,
            )
            new_text, n = pattern.subn(rf'\g<1>{new}', new_text)
            file_changes += n
        if new_text != text:
            p.write_text(new_text, encoding="utf-8")
            print(f"  ✓ {p.relative_to(ROOT)} ({file_changes} 处)")
            changes.append(str(p))

    print(f"\n=== 总改动: {len(set(changes))} 个唯一文件 ===")

if __name__ == "__main__":
    main()