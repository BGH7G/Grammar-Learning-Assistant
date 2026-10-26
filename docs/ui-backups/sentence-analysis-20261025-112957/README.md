# 句子分析 UI 与逻辑基线备份

snapshot/ 保留原相对路径，涵盖前端源码、后端分析逻辑、提示词、配置定义及依赖锁文件。manifest.json 记录每项文件的 SHA-256、大小及备份时间。未修改当前功能代码。

## UI 结构

工作空间导航 → 句子分析标题/状态 → 输入卡片（示例、清空、分析、计数、自动分句）→ 模型工具条 → 错误/加载反馈 → 首屏耗时 → 分层解析 → 请求性能及 Token 明细 → 使用说明。

分层解析内部：层级/缓存状态 → 多句切换 → 主干概述 → 面包屑 → 原文高亮 → 成分图例 → 中文翻译 → 直接成分卡片 → 修饰关系 → 警告/说明。

## 逻辑关系

- App.tsx 管理输入、自动分句、模型配置、requestId、加载状态、结果、错误和累计指标。启动 get_settings；输入上限 2000 个 Unicode 字符，自动分句最多 8 句。
- 分析：输入 → analyze_sentence → 配置和系统凭据 → 分词及首层模型请求 → LayerDocument + metrics → LayeredAnalysisView。仅当前 requestId 的响应可更新结果。
- 修改输入、清空或切换分句使旧结果失效；保存模型配置清空旧会话。已发出的模型请求仍可能计费。
- 离线示例来自 layerSample.ts；AnalysisView.tsx / sample.ts 为旧版参考，当前入口是 LayeredAnalysisView.tsx。
- 分层视图持有当前节点、已加载层、悬停、错误和待处理请求。parentId 形成面包屑，sentenceId 关联句子根，first/last 映射 tokens，再由 pieceId 定位无损原文。
- 展开：本地层缓存优先 → expand_node → 后端会话缓存或模型 → 校验并缓存 → 子层。最多四层，不自动预取；失败保留当前层。切换面包屑和句子优先使用已加载结果。
- 悬停成分定位原文；sourceId / headToken 建立修饰来源与中心词关系。
- 标点角色及纯标点节点不显示独立卡片、图例和高亮，原文标点保留；协议仍保留标点用于完整覆盖校验。
- PerformanceDetails 汇总首层和展开的耗时及 Token；缓存命中不增加请求 Token，未提供的用量保持未知。
- 后端 layered.rs 负责协议校验、ID、会话缓存与四层限制；llm.rs 负责请求与用量；最多两句并发，每次解析最多一次修复。

## 样式及共享依赖

App.css 负责 Fluent 浅色主题、卡片、布局、断点及窗口右边缘滚动。ColorSettings.tsx 定义角色颜色，通过 App 根节点 CSS 变量影响原文、图例和标签。

用户配色和方案存于 localStorage：grammar-assistant.component-colors.v1、grammar-assistant.color-schemes.v1。这里只备份行为源码，不读取或备份设备中的个人配色、API Key、凭据或 model.json。

## 对照与恢复

逐项对比 snapshot/<原路径> 与当前源码；恢复前先保存当前版本。App.tsx、App.css、Icon.tsx、ColorSettings.tsx 和后端入口为共享文件，整体覆盖也会回退其他页面；只恢复句子分析时应合并相关片段。

这是源码基线，不包含依赖安装目录、编译产物、安装程序和运行截图，不能作为独立安装包。文件校验：计算 snapshot/ 中文件的 SHA-256 并与 manifest.json 对比。
