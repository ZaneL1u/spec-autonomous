import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'zh-CN',
  title: 'Spec Autonomous',
  description: '把 OpenSpec / Spec Kit 的规范，变成可验证、可恢复的持续交付流程。',
  base: '/spec-autonomous/',
  cleanUrls: true,
  lastUpdated: true,
  sitemap: {
    hostname: 'https://zanel1u.github.io/spec-autonomous/',
  },
  head: [
    ['meta', { name: 'theme-color', content: '#0b1220' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:title', content: 'Spec Autonomous' }],
    ['meta', { property: 'og:description', content: '让规范真正走到验证和交付。' }],
  ],
  themeConfig: {
    logo: {
      light: '/mark.svg',
      dark: '/mark.svg',
    },
    siteTitle: 'Spec Autonomous',
    nav: [
      { text: '开始使用', link: '/guide/quick-start' },
      { text: '核心概念', link: '/concepts/sdd' },
      { text: '参考', link: '/reference/commands' },
      { text: '进阶', link: '/advanced/extensions' },
      { text: '参与贡献', link: '/contributing' },
      { text: '更新记录', link: '/changelog' },
    ],
    sidebar: {
      '/guide/': [
        {
          text: '入门',
          items: [
            { text: '这是什么', link: '/guide/what-is' },
            { text: '快速开始', link: '/guide/quick-start' },
            { text: '安装方式', link: '/guide/install' },
            { text: '完整工作流', link: '/guide/workflow' },
          ],
        },
      ],
      '/concepts/': [
        {
          text: '核心概念',
          items: [
            { text: '什么是 SDD', link: '/concepts/sdd' },
            { text: 'OpenSpec、Spec Kit 与本项目', link: '/concepts/ecosystem' },
            { text: '什么是 GSD', link: '/concepts/gsd' },
            { text: '核心理念', link: '/concepts/principles' },
          ],
        },
      ],
      '/reference/': [
        {
          text: '使用参考',
          items: [
            { text: '命令', link: '/reference/commands' },
            { text: 'Skills', link: '/reference/skills' },
            { text: 'MCP', link: '/reference/mcp' },
            { text: '配置', link: '/reference/configuration' },
          ],
        },
      ],
      '/advanced/': [
        {
          text: '进阶',
          items: [
            { text: '扩展与集成', link: '/advanced/extensions' },
            { text: '恢复与清理', link: '/advanced/recovery' },
          ],
        },
      ],
    },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/ZaneL1u/spec-autonomous' },
    ],
    editLink: {
      pattern: 'https://github.com/ZaneL1u/spec-autonomous/edit/main/website/:path',
      text: '在 GitHub 上编辑此页',
    },
    lastUpdated: {
      text: '最后更新',
      formatOptions: {
        dateStyle: 'medium',
        timeStyle: 'short',
      },
    },
    docFooter: {
      prev: '上一篇',
      next: '下一篇',
    },
    outline: {
      level: [2, 3],
      label: '本页内容',
    },
    search: {
      provider: 'local',
    },
    footer: {
      message: '基于 MIT License 发布',
      copyright: 'Copyright © 2026 Spec Autonomous contributors',
    },
  },
})
