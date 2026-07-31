#!/usr/bin/env python3
"""
rename_maven.py — 把 sdk/java/* + examples/* 的 groupId / artifactId 改为 io.digua + digua-* 前缀。

规则：
  - com.kudi.coder          → io.digua
  - com.kudi.coder.demo     → io.digua.demo
  - kudi-coder-sdk-parent   → digua-sdk-parent
  - kudi-coder-sdk-common   → digua-sdk-common
  - kudi-http-client        → digua-http-client
  - notification-service-sdk → digua-notification-sdk
  - token-server-client-sdk → digua-token-client-sdk
  - token-server-user-sdk   → digua-token-user-sdk
  - user-service-client-sdk → digua-user-client-sdk
  - user-service-user-sdk   → digua-user-user-sdk
  - java8-webapi-parent     → (parent of demo) — 不动（不发布）
"""
import re
from pathlib import Path

ROOT = Path('/home/kudi/projects/low-memory-micro-services')

# 改 groupId: com.kudi.coder → io.digua
# 改 artifactId: 加 digua- 前缀（具体看映射）

# groupId 全局替换（限于 pom.xml 文件）
GROUP_RENAMES = {
    'com.kudi.coder': 'io.digua',
    'com.kudi.coder.demo': 'io.digua.demo',
}

# artifactId 精确映射（kebab-case Java 约定）
ARTIFACT_RENAMES = {
    'kudi-coder-sdk-parent': 'digua-sdk-parent',
    'kudi-coder-sdk-common': 'digua-sdk-common',
    'kudi-http-client': 'digua-http-client',
    'notification-service-sdk': 'digua-notification-sdk',
    'token-server-client-sdk': 'digua-token-client-sdk',
    'token-server-user-sdk': 'digua-token-user-sdk',
    'user-service-client-sdk': 'digua-user-client-sdk',
    'user-service-user-sdk': 'digua-user-user-sdk',
}

# Java 代码里包声明（package com.kudi.coder.sdk.xxx → io.digua.sdk.xxx）
JAVA_PACKAGE_RENAMES = {
    'com.kudi.coder': 'io.digua',
}

# 收集所有需要改的文件
files = list(ROOT.rglob('pom.xml')) + list(ROOT.rglob('*.java'))
count = 0
total = 0
for p in files:
    if '/target/' in str(p) or '/node_modules/' in str(p):
        continue
    try:
        text = p.read_text(encoding='utf-8')
    except UnicodeDecodeError:
        continue
    new_text = text
    file_n = 0
    # groupId 替换（pom.xml）
    if p.suffix == '.xml':
        for old, new in GROUP_RENAMES.items():
            new_text, n = re.subn(re.escape(old), new, new_text)
            file_n += n
        for old, new in ARTIFACT_RENAMES.items():
            new_text, n = re.subn(re.escape(old), new, new_text)
            file_n += n
    # Java 包声明
    elif p.suffix == '.java':
        for old, new in JAVA_PACKAGE_RENAMES.items():
            # 只在 package / import 行
            new_text = re.sub(
                rf'^(package |import )(static )?{re.escape(old)}(\.\w+)*',
                lambda m: m.group(1) + (m.group(2) or '') + new + (m.group(3) or ''),
                new_text,
                flags=re.MULTILINE,
            )
    if new_text != text:
        p.write_text(new_text, encoding='utf-8')
        count += 1
        total += file_n
print(f'fixed {count} files, {total} replacements')

# 移动 Java 源码目录（从 com/kudi/coder → io/digua）
src_dirs = [
    ('sdk/java/common/src/main/java/com/kudi/coder', 'sdk/java/common/src/main/java/io/digua'),
    ('sdk/java/kudi-http-client/src/main/java/com/kudi/coder', 'sdk/java/kudi-http-client/src/main/java/io/digua'),
    ('sdk/java/notification-service-sdk/src/main/java/com/kudi/coder', 'sdk/java/notification-service-sdk/src/main/java/io/digua'),
    ('sdk/java/token-server-client-sdk/src/main/java/com/kudi/coder', 'sdk/java/token-server-client-sdk/src/main/java/io/digua'),
    ('sdk/java/token-server-user-sdk/src/main/java/com/kudi/coder', 'sdk/java/token-server-user-sdk/src/main/java/io/digua'),
    ('sdk/java/user-service-client-sdk/src/main/java/com/kudi/coder', 'sdk/java/user-service-client-sdk/src/main/java/io/digua'),
    ('sdk/java/user-service-user-sdk/src/main/java/com/kudi/coder', 'sdk/java/user-service-user-sdk/src/main/java/io/digua'),
    ('examples/java8-webapi/demo-shared/src/main/java/com/kudi/coder', 'examples/java8-webapi/demo-shared/src/main/java/io/digua'),
    ('examples/java8-webapi/demo-webapp/src/main/java/com/kudi/coder', 'examples/java8-webapi/demo-webapp/src/main/java/io/digua'),
]
import shutil
move_count = 0
for src, dst in src_dirs:
    src_p = ROOT / src
    dst_p = ROOT / dst
    if src_p.exists():
        dst_p.parent.mkdir(parents=True, exist_ok=True)
        if dst_p.exists():
            shutil.rmtree(dst_p)
        shutil.move(str(src_p), str(dst_p))
        move_count += 1
        print(f'  moved: {src} → {dst}')
print(f'moved {move_count} directories')