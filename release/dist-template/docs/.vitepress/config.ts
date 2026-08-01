import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'zh-CN',
  title: 'coder-micro-services',
  description: 'coder-micro-services v1.0.0 部署文档',

  head: [
    ['link', { rel: 'icon', href: '/favicon.ico' }],
  ],

  themeConfig: {
    logo: '/logo.svg',
    nav: [
      { text: '运维部署', link: '/operations/' },
      { text: '业务开发', link: '/development/' },
      { text: '服务', link: '/services/overview' },
      { text: '可观测性', link: '/observability/overview' },
      { text: '故障排查', link: '/troubleshooting' },
    ],
    sidebar: {
      '/': [
        { text: '开始使用', items: [
          { text: '快速开始', link: '/getting-started' },
          { text: '配置参考', link: '/configuration' },
        ]},
        { text: '运维', items: [
          { text: '运维与部署指南', link: '/operations/' },
          { text: '离线部署', link: '/deployment/air-gap' },
          { text: '部署包发布', link: '/deployment/deploy-package' },
          { text: '升级指南', link: '/deployment/upgrade' },
          { text: '备份与恢复', link: '/deployment/backup' },
        ]},
        { text: '业务开发', items: [
          { text: '业务开发指南', link: '/development/' },
        ]},
        { text: '服务', items: [
          { text: '服务概览', link: '/services/overview' },
          { text: 'API 网关', link: '/services/api-gateway' },
        ]},
        { text: '可观测性', items: [
          { text: '监控与追踪', link: '/observability/overview' },
        ]},
        { text: '其他', items: [
          { text: '故障排查', link: '/troubleshooting' },
          { text: '变更日志', link: '/changelog' },
        ]},
      ],
    },
    search: {
      provider: 'local',
    },
    editLink: {
      pattern: 'https://gitlab.example.com/coder-micro-services/edit/main/docs/:path',
      text: '在 GitLab 上编辑此页',
    },
  },

  markdown: {
    theme: {
      light: 'github-light',
      dark: 'github-dark',
    },
  },
})
