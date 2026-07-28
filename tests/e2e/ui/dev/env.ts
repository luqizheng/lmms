import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

// 本机无 sudo 时把 .deb 解包到 ~/playwright-libs/root，chrome 子进程经 LD_LIBRARY_PATH 命中；
// 其他环境（CI、容器、已装系统依赖）目录不存在，自动 no-op。
const localRoot = join(homedir(), 'playwright-libs', 'root');
const localLib = join(localRoot, 'usr', 'lib', 'x86_64-linux-gnu');
if (existsSync(localLib)) {
  process.env.LD_LIBRARY_PATH = [localLib, process.env.LD_LIBRARY_PATH].filter(Boolean).join(':');
}
const fontsConf = join(homedir(), 'playwright-libs', 'fonts.conf');
if (existsSync(fontsConf)) {
  process.env.FONTCONFIG_FILE ??= fontsConf;
}
