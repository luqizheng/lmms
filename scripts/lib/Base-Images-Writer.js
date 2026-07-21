'use strict';

const fs = require('node:fs');
const path = require('node:path');

const VERSION = 1;
const DEFAULT_REGISTRY = 'zhcoder-docker-registry.com:8000/official';
const DEFAULT_BUILDER_REGISTRY = 'zhcoder-docker-registry.com:8000';

// 判断是否为 builder 镜像 ref：以默认 builder registry 前缀开头。
// old manifest 中可能已被前一次 sync 重写过前缀，传入 currentBuilderRegistry
// 时也识别该前缀，以便正确还原。
function isBuilderRef(ref, currentBuilderRegistry) {
  if (typeof ref !== 'string' || !ref) return false;
  if (ref.startsWith(`${DEFAULT_BUILDER_REGISTRY}/`)) return true;
  if (currentBuilderRegistry && currentBuilderRegistry !== DEFAULT_BUILDER_REGISTRY) {
    return ref.startsWith(`${currentBuilderRegistry}/`);
  }
  return false;
}

// 把 builder ref 的 registry 前缀替换为 targetBuilderRegistry。
// ref 可能是原始形式（DEFAULT_BUILDER_REGISTRY 前缀）或已被前一次 sync 重写过
// 的形式（currentBuilderRegistry 前缀）；两种都先还原成原始形式再重写。
function rewriteBuilderRef(ref, targetBuilderRegistry, currentBuilderRegistry) {
  let canonical = ref;
  if (
    currentBuilderRegistry &&
    currentBuilderRegistry !== DEFAULT_BUILDER_REGISTRY &&
    ref.startsWith(`${currentBuilderRegistry}/`)
  ) {
    // 还原成默认前缀
    canonical = `${DEFAULT_BUILDER_REGISTRY}${ref.slice(currentBuilderRegistry.length)}`;
  }
  if (targetBuilderRegistry === DEFAULT_BUILDER_REGISTRY) {
    return canonical; // 默认就是原始形式
  }
  if (canonical.startsWith(`${DEFAULT_BUILDER_REGISTRY}/`)) {
    return `${targetBuilderRegistry}${canonical.slice(DEFAULT_BUILDER_REGISTRY.length)}`;
  }
  return canonical;
}

// mergeMirror(scanned, old, config)
//   config: { baseRegistry?: string, builderRegistry?: string }
//     - baseRegistry:    公共基础镜像来源。空字符串 = docker hub（mirror=null）；
//                        非空字符串 = 作为 mirror 写入，images-pull 会先尝试 `<baseRegistry>/<ref>`。
//     - builderRegistry: 私有 builder 镜像 registry。默认 DEFAULT_BUILDER_REGISTRY。
//                        与默认不同时，重写 builder ref 的前缀为该值，mirror 设为 null。
//   兼容旧签名：第三参数为字符串时按 baseRegistry 处理（builderRegistry 用默认）。
function mergeMirror(scanned, old, config) {
  const cfg = config || {};
  const baseRegistry = typeof cfg === 'string' ? cfg : (cfg.baseRegistry || '');
  const builderRegistry = cfg.builderRegistry || DEFAULT_BUILDER_REGISTRY;

  // old 参数保留用于未来扩展（如手动覆盖单镜像 mirror），当前实现完全由
  // baseRegistry / builderRegistry 决定输出，确保配置变更幂等生效。
  void old;

  return scanned.map((img) => {
    if (isBuilderRef(img.ref, builderRegistry)) {
      const newRef = rewriteBuilderRef(img.ref, builderRegistry, builderRegistry);
      return { ref: newRef, source: img.source, mirror: null };
    }

    // 公共镜像：baseRegistry 是用户当前选择的来源，完全决定 mirror。
    //   baseRegistry=''  -> mirror=null（直接拉 docker hub）
    //   baseRegistry='x' -> mirror='x'（images-pull 先试 x/<ref> 再试 <ref>）
    // 不再读 old.mirror：旧 manifest 残留的 mirror 不应跨配置变更保留。
    return { ref: img.ref, source: img.source, mirror: baseRegistry || null };
  });
}

function writeManifest(outPath, images) {
  const data = {
    version: VERSION,
    generatedAt: new Date().toISOString(),
    images,
  };
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, JSON.stringify(data, null, 2) + '\n');
}

module.exports = {
  mergeMirror,
  writeManifest,
  DEFAULT_REGISTRY,
  DEFAULT_BUILDER_REGISTRY,
  isBuilderRef,
  rewriteBuilderRef,
};
