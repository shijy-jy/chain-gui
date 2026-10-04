---
id: 工具-StoryForge使用指南
type: note
title: 工具·StoryForge使用指南
parent: 遗忘异录
tags: [工具, 工作流, 知识库]
---

# 工具·StoryForge使用指南

StoryForge（主人称为 chain 软件）是本地优先的 AI 小说创作工作台，本知识库 G:\story\story 即由它管理的 dev 模式图谱工程。

## 启动与访问
- 双击 G:\story\启动故事熔炉.cmd（或 G:\story\storyforge 下 npm run dev）
- 浏览器访问 http://localhost:1111/storyforge/
- 命令行窗口关闭即服务停止

## dev 模式知识库规范（本库适用）
- .chain/.mode = dev：手工知识库，AI 只做辅助整理，不套链协议分析模式
- 一个 .chain/nodes/*.md 文件 = 一个节点；id 取文件名，title 写主题名
- frontmatter parent 表达归属，正文 [[双链]] 关联，正文自包含
- 自由拓扑，推荐主题中心（hub）式：大主题一个根节点，细节挂下面
- 不编造知识来源；改完文件软件自动刷新（界面即所见）
- 完整指南见 .chain/AI_GUIDE.md

## 协作约定
- AI 整理知识库优先走软件通道（写 .chain/nodes/*.md 即被软件识别）
- 涉及删除/覆盖先征得主人授权并备份
