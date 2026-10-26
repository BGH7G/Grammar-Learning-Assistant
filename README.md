# Grammar Assistant

Windows 桌面英语语法助手。Tauri 2 + React + TypeScript + Rust。

## 已实现

- 英文输入、2,000 Unicode 字符限制、可关闭的自动分句（最多 8 句）、加载和错误状态、旧请求结果失效。
- 分层 v2：可点击的彩色原句、译文和内部结构；按需展开、面包屑、键盘导航、修饰关联及会话缓存，附离线示例。
- 统一“句子成分 · 语法类型”标签，支持情态动词、助动词、系动词、实义动词及其他词类；名词短语优先分成核心短语和后置定语，并细分冠词类别。
- 成分配色页：各成分背景色和文字色、实时预览、内置预设与自定义配色方案，修改自动保存在本机。
- 模型接入页：完整 Chat Completions 端点、模型 ID、API Key、可选 JSON mode、温度滑块（0–2）、思考开关、Low/High/Max 推理强度、保存和连接测试。
- Rust 请求模型；10 秒连接 / 60 秒单次请求超时；格式错误最多修复一次，鉴权失败及截断不重试。
- Rust 本地无损切片，模型只返回节点、翻译及简短说明；校验节点 ID、父子范围、环、同级重叠及角色枚举。

## 启动与验证

```powershell
git clone https://github.com/BGH7G/Grammar-Learning-Assistant.git
cd Grammar-Learning-Assistant
npm.cmd ci
npm.cmd run tauri dev
```

```powershell
npm.cmd run build
node scripts/check-component-labels.mjs
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

需要 Rust MSVC、C++ Build Tools / Windows SDK 和 WebView2。浏览器预览只支持离线示例。

## 配置模型

1. 打开“模型接入”，填写完整接口地址（包含服务所需的 /v1/chat/completions 路径）及精确模型 ID。
2. 填写 API Key 并保存。无需鉴权的本机服务可留空。
3. 服务支持 response_format: json_object 时才启用 JSON mode。
4. 保存后测试连接。测试会发送简短请求，可能产生费用。
5. 返回句子分析页，输入英文并分析。

普通配置存储在 Tauri app_config_dir 下的 model.json；密钥按端点存入 Windows 凭据管理器，不返回前端、不写入普通文件或日志。留空保留该端点密钥；勾选清除并保存可删除。更换端点不会把旧端点密钥自动带到新服务。旧端点凭据可通过重新填入该端点后清除。

远程接口要求 HTTPS，本机 localhost / 127.0.0.1 / ::1 可用 HTTP；禁用 HTTP 重定向。当前只支持 Chat Completions 非流式文本响应，不支持其他专有 API 协议、工具调用响应或模型列表自动发现。环境变量配置尚未接入，以页面保存配置为准。

结构校验不等于语法正确。修改文本只忽略旧结果，不取消服务端请求。固定离线示例不是模型分析。未配置真实服务时只能验证离线流程及本地模拟 HTTP 测试。

应用标识 com.example.grammarassistant 与模板图标为开发占位；后续再加入划词、悬浮窗等功能。

## 界面约定

采用 Windows 11 Fluent 亮色风格：#F3F3F3 背景、白色 8px 圆角卡片、细边框和深靛蓝主色，统一使用 Segoe UI / 微软雅黑与本地 SVG 线性图标。

设置页在标准窗口下使用导航、服务列表、配置卡片三栏；窄窗口中服务列表移至配置上方。自定义服务使用琥珀色标签，内置示例使用蓝色标签，当前选中服务使用绿色边框。JSON 模式为真实配置开关；未保存配置时连接验证卡片置灰。支持键盘焦点和减少动态效果偏好。

## 生成参数与性能

- 旧配置自动补齐新字段：温度 0.3、思考关闭、推理强度 High。开启思考后温度卡片置灰，不发送温度参数；关闭思考时不发送强度（reasoning_effort 兼容模式发送 none 表示关闭）。
- 自动参数格式仅识别 api.deepseek.com 官方端点，显式发送 thinking.type=disabled，避免服务默认开启思考。其他端点在自动模式不发送思考字段；DeepSeek 代理或本机服务可在兼容设置中选择 DeepSeek 格式，或按服务文档选择 reasoning_effort 格式（须支持 none/low/high/max）。“不发送”表示使用服务商默认行为，不等于关闭思考。
- DeepSeek 请求设有输出预算，并为所选思考强度额外预留推理额度；其他服务的 token 上限使用其默认值。遇到截断明确报错，不拼接缺失内容。
- 原文和空白由 Rust 切片保留，模型不再重复输出 pieces；当前提示词包含合法 JSON 示例、短语分组和最多 2 条简短说明，文件位于 src-tauri/src/prompts/layer-v2.txt。analyze.txt 是保留用于基准比较的旧版完整树提示词。
- 结果下方可展开“性能详情”：显示请求次数、修复原因、HTTP 等待/读取/校验耗时和服务返回的输入/输出/推理 token。未提供的计数显示为“服务未提供”，不推断成 0。不会保存密钥、原文或模型推理内容到日志。
- 2026-10-24 两条公开样例在当时配置的服务上、关闭思考时分别耗时约 1.47 秒和 2.17 秒，均一次通过；这不是所有句子的性能保证，详见 docs/performance.md。

真实服务冒烟测试默认跳过。如需手动运行（调用已保存的服务和系统凭据，会产生模型调用费用，发送两条公开样例，且不改写配置）：

```powershell
cargo test --locked --manifest-path src-tauri/Cargo.toml configured_model_smoke -- --ignored --nocapture
```

## 分层 v2 与 Token 对比

当前提示词版本为 `layer-v2.9.2-grammar-types`，采用一次一层协议。旧版完整树提示词及解析器已备份到 `docs/prompt-backups/v1-20261024-165343/`，包含 SHA-256 清单。旧版仅保留作回归和基准对比；历史 v1/v2 Token 对比对应当时的 `layer-v2.5`，不代表当前提示词用量。

- 当前提示词：`src-tauri/src/prompts/layer-v2.txt`。
- 协议与缓存边界：[layered-protocol-v2.md](docs/layered-protocol-v2.md)。
- Token 实测及原始数据索引：[token-comparison.md](docs/token-comparison.md)。
- 最新语法类型标签实测：[测试记录](docs/benchmarks/grammar-types-layer-v2.9.2-grammar-types-1792984179/review.md)。
- 仅用户点击时展开，最多四层。已加载节点从会话缓存读取；模型设置保存后旧会话失效。默认不自动预取，避免无意增加调用量。

运行同模型、相同参数、六条公开样例各两轮的 v1/v2 基准（会调用已配置服务并产生费用；不修改已保存配置）。基准包含旧版完整解析、新版首屏及按标记展开至四层，保留所有重试和失败统计：

```powershell
cargo test --locked --manifest-path src-tauri/Cargo.toml benchmark_v1_v2 -- --ignored --nocapture
```

报告保存在 `docs/benchmarks/v1-v2-时间戳/report.json`。使用 `scripts/summarize_benchmark.py` 可生成 Markdown、CSV 和汇总 JSON。脚本只使用服务实际返回的 usage，不把缺失值当成零。深层全部展开的工作量和旧版一次输出并不完全等价，应结合逐项结构和人工检查评估。
