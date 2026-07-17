import { defineConfig } from 'vitepress'

// Coder Micro Services 文档站点配置
// 内容源：../docs（通过 symlink 引入），单一真相源，本目录只管渲染配置。
export default defineConfig({
  lang: 'zh-CN',
  title: 'Coder Micro Services',
  description: 'Rust + Axum 微服务基础框架文档',
  cleanUrls: true,
  lastUpdated: true,
  // docs/ 内链接按物理结构编写，经 symlink 引入后 dead-link 检测器路径前缀错位会误报；
  // 实际生成的链接基于页面 URL 解析，正确。localhost 与跨出 docs/ 的链接也一并忽略。
  ignoreDeadLinks: true,
  // docs-site/README.md 是工程说明（GitHub 浏览用），不进文档站
  srcExclude: ['README.md'],
  // VitePress 目录索引是 index.md 而非 README.md，把各 section 的 README 映射为目录首页
  rewrites: {
    'onboarding/README.md': 'onboarding/index.md',
    'user-manual/README.md': 'user-manual/index.md'
  },

  head: [
    ['meta', { name: 'theme-color', content: '#3c8772' }],
    ['meta', { name: 'author', content: 'Coder Micro Services Team' }]
  ],

  markdown: {
    lineNumbers: false,
    image: { lazy: true }
  },

  themeConfig: {
    outline: { level: [2, 3], label: '本页导航' },
    docFooter: { prev: '上一篇', next: '下一篇' },
    lastUpdatedText: '最后更新',
    returnToTopLabel: '回到顶部',
    sidebarMenuLabel: '目录',
    darkModeSwitchLabel: '主题',
    lightModeSwitchTitle: '切换到浅色模式',
    darkModeSwitchTitle: '切换到深色模式',
    skipToContentLabel: '跳到正文',

    search: {
      provider: 'local',
      options: {
        translations: {
          button: { buttonText: '搜索文档', buttonAriaLabel: '搜索文档' },
          modal: {
            displayDetails: '显示详情',
            resetButtonTitle: '清除查询',
            backButtonTitle: '返回',
            noResultsText: '没有找到结果',
            footer: {
              selectText: '选择',
              navigateText: '切换',
              navigateUpKeyAriaLabel: '上一个',
              navigateDownKeyAriaLabel: '下一个',
              closeText: '关闭',
              closeKeyAriaLabel: '关闭'
            }
          }
        }
      }
    },

    nav: [
      { text: '首页', link: '/' },
      { text: '新人入门', link: '/onboarding/' },
      { text: '用户手册', link: '/user-manual/' },
      { text: '专题指南', link: '/guides/testing' },
      { text: '架构决策', link: '/decisions/ADR-001-api-gateway-multi-file-router-config' },
      { text: '运维手册', link: '/runbooks/oncall' },
      { text: 'AI 协作', link: '/ai-context' }
    ],

    sidebar: {
      '/onboarding/': [
        {
          text: '新人入职',
          collapsed: false,
          items: [
            { text: '导读', link: '/onboarding/' },
            { text: '01 · 项目概览', link: '/onboarding/01-项目概览' },
            { text: '02 · 环境准备', link: '/onboarding/02-环境准备' },
            { text: '03 · 第一次跑通', link: '/onboarding/03-第一次跑通' },
            { text: '04 · 后端框架与配置', link: '/onboarding/04-后端框架与配置' },
            { text: '05 · 数据库与迁移', link: '/onboarding/05-数据库与迁移' },
            { text: '06 · 前端开发', link: '/onboarding/06-前端开发' },
            { text: '07 · 日志与排查 Bug', link: '/onboarding/07-日志与排查Bug' },
            { text: '08 · 常见任务速查', link: '/onboarding/08-常见任务速查' },
            { text: '09 · 出问题怎么办', link: '/onboarding/09-出问题怎么办' },
            { text: '附录 · 术语与链接', link: '/onboarding/附录-术语与链接' }
          ]
        }
      ],

      '/user-manual/': [
        {
          text: '用户手册',
          collapsed: false,
          items: [
            { text: '导读', link: '/user-manual/' },
            { text: '01 · 快速上手', link: '/user-manual/01-quick-start' },
            { text: '02 · 架构总览', link: '/user-manual/02-architecture' },
            { text: '03 · 本地开发', link: '/user-manual/03-local-development' },
            { text: '04 · 测试', link: '/user-manual/04-testing' },
            { text: '05 · 可观测性', link: '/user-manual/05-observability' },
            { text: '06 · 部署', link: '/user-manual/06-deployment' },
            { text: '07 · 限流', link: '/user-manual/07-rate-limiting' },
            { text: '08 · 灰度发布', link: '/user-manual/08-canary-rollout' },
            { text: '09 · AI 协作工作流', link: '/user-manual/09-ai-workflows' },
            { text: '10 · 故障排查', link: '/user-manual/10-troubleshooting' },
            { text: '11 · 参考', link: '/user-manual/11-reference' },
            { text: '12 · 发布与分发', link: '/user-manual/12-release-and-distribute' }
          ]
        }
      ],

      '/guides/': [
        {
          text: '专题指南',
          collapsed: false,
          items: [
            { text: '测试', link: '/guides/testing' },
            { text: '可观测性', link: '/guides/observability' },
            { text: 'OTel / Jaeger 跨服务 Trace', link: '/guides/otel-tracing' },
            { text: '灰度发布', link: '/guides/canary-rollout' },
            { text: 'API Gateway 路由配置', link: '/guides/api-gateway-routing' },
            { text: '服务注册', link: '/guides/service-registration' },
            { text: '服务间认证', link: '/guides/service-to-service-auth' },
            { text: '前端鉴权', link: '/guides/frontend-auth' },
            { text: '新增服务', link: '/guides/add-new-service' },
            { text: '迁移状态端点', link: '/guides/migrations-state' },
            { text: 'config-service 使用', link: '/guides/config-service-usage' },
            { text: '菜单发布', link: '/guides/menu-publish' },
            { text: 'Java 集成', link: '/guides/java-integration' },
            { text: 'Rust SDK', link: '/guides/rust-sdk' },
            { text: '提交规范', link: '/guides/commit-convention' }
          ]
        }
      ],

      '/decisions/': [
        {
          text: '架构决策记录 (ADR)',
          collapsed: false,
          items: [
            {
              text: 'ADR-001 · API Gateway 多文件路由配置',
              link: '/decisions/ADR-001-api-gateway-multi-file-router-config'
            },
            {
              text: 'ADR-002 · 服务规范基线',
              link: '/decisions/ADR-002-service-standard-baseline'
            }
          ]
        }
      ],

      '/runbooks/': [
        {
          text: '运维手册 (Runbooks)',
          collapsed: false,
          items: [
            { text: 'On-Call 值班', link: '/runbooks/oncall' },
            { text: '凭据管理', link: '/runbooks/credentials' },
            { text: '密钥轮换', link: '/runbooks/secret-rotation' },
            { text: '数据库迁移', link: '/runbooks/database-migration' },
            { text: '日志', link: '/runbooks/logging' },
            { text: '故障排查', link: '/runbooks/troubleshooting' }
          ]
        }
      ],

      '/plans/': [
        {
          text: '长期计划',
          collapsed: false,
          items: [
            {
              text: '长期 AI 效率计划',
              link: '/plans/2026-06-09-long-term-ai-efficiency-plan'
            },
            {
              text: 'i18n 硬编码清理',
              link: '/plans/2026-07-09-i18n-hardcode-cleanup'
            },
            {
              text: 'TypeScript Strict 采纳',
              link: '/plans/2026-07-09-typescript-strict-adoption'
            }
          ]
        }
      ],

      // 顶级散页（首页 / AI 协作 / 开发者工作流）：分别给 key，
      // 避免 `'/': [...]` 在 vitepress 中作为 catch-all 副作用覆盖其它 section 的 sidebar
      '/': [
        {
          text: '快速入口',
          collapsed: false,
          items: [
            { text: '首页', link: '/' },
            { text: 'AI 协作上下文', link: '/ai-context' },
            { text: '开发者工作流总图', link: '/developer-workflow' }
          ]
        }
      ],
      '/ai-context/': [
        {
          text: '快速入口',
          collapsed: false,
          items: [
            { text: '首页', link: '/' },
            { text: 'AI 协作上下文', link: '/ai-context' },
            { text: '开发者工作流总图', link: '/developer-workflow' }
          ]
        }
      ],
      '/developer-workflow/': [
        {
          text: '快速入口',
          collapsed: false,
          items: [
            { text: '首页', link: '/' },
            { text: 'AI 协作上下文', link: '/ai-context' },
            { text: '开发者工作流总图', link: '/developer-workflow' }
          ]
        }
      ]
    },

    footer: {
      message: '基于 Rust + Axum 构建的微服务基础框架',
      copyright: 'Copyright © 2026 Coder Micro Services'
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/coder-micro-services' }
    ]
  }
})
