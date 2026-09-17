---
layout: home

hero:
  name: Spec Autonomous
  text: 让规范真正走到交付
  tagline: 你继续用 OpenSpec 或 Spec Kit 写需求。它负责把里程碑拆成可领取的工作包，协调宿主中的全新 Agent 上下文，并用 Git、测试和证据确认工作真的完成。
  image:
    src: /mark.svg
    alt: Spec Autonomous
  actions:
    - theme: brand
      text: 5 分钟开始
      link: /guide/quick-start
    - theme: alt
      text: 它到底是什么
      link: /guide/what-is
    - theme: alt
      text: 查看 GitHub
      link: https://github.com/ZaneL1u/spec-autonomous

features:
  - title: 不发明第四套规范
    details: OpenSpec 和 Spec Kit 仍然是需求、设计和任务的事实源。本项目只补上跨阶段执行、验证、恢复和交付。
  - title: CLI 不创建 Agent
    details: CLI 只生成不可变工作包。Codex、Claude Code 等宿主负责创建全新上下文并返回结构化回执。
  - title: 不是“模型说完成”
    details: 每份结果都要经过身份、实际 diff、写入范围、测试、Git revision 和原生任务回写检查。
  - title: 并行但不冒进
    details: 只有依赖满足、写集不冲突且宿主容量允许的任务才会并行；未知写集默认串行。
  - title: 中断后能接着走
    details: Run、attempt、证据和持久化意图写入账本。恢复时先核对 Git 与真实状态，不盲目重跑副作用。
  - title: CLI、Skills、MCP 同一套能力
    details: 人可以用命令，Agent 可以用 Skills，宿主可以走 MCP；底层都调用同一个确定性能力核心。
---

## 一句话说明

**Spec Autonomous 是 SDD 工作流的“交付控制层”。**

OpenSpec / Spec Kit 帮你说明“要做什么”，宿主 Agent 负责“动脑和写代码”，Spec Autonomous 负责“该轮到谁、在哪改、结果是否可信、失败后怎么恢复”。

<div class="concept-flow">
  <div>目标与规范</div>
  <div>阶段与任务图</div>
  <div>宿主工作包</div>
  <div>验证与集成</div>
  <div>回写与归档</div>
</div>

::: tip 当前发行状态
当前版本为 `0.1.0-alpha.13`。GitHub Release 安装已支持 macOS arm64；公开 npm registry 尚未发布。文档不会把尚未验证的平台描述成可用。
:::
