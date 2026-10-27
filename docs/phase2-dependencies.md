# 第二阶段：依赖与模块

此页保留最初依赖接入记录。后续受控复制原型已按用户要求移除，[触发与状态管理](phase2-trigger-state.md) 仅作为历史记录。当前实现见 [剪贴板取词说明](clipboard-selection.md)。

本次完成依赖安装与模块边界，尚未启用快捷键或系统取词。

## 依赖

- `tauri-plugin-global-shortcut`：桌面目标依赖；由 Rust 的 `hotkey::plugin` 初始化一次。前端配套包由 Tauri 安装器添加，当前没有前端调用，也未开放快捷键 IPC 权限。
- `windows`：仅 Windows 目标引入；启用 Foundation、COM、Accessibility、WindowsAndMessaging、KeyboardAndMouse、DataExchange、Memory 七组 feature。准确版本见 Cargo.lock。
- 未引入额外快捷键系统、编程语言或云服务。

## 模块边界

| 模块 | 当前状态与后续职责 |
| --- | --- |
| `hotkey.rs` | 已集中初始化插件；M1 加入注册/注销、重复过滤、目标窗口捕获。 |
| `capture/mod.rs` | 定义统一结果与错误协议及 CaptureService 边界；占位入口明确返回 unsupported，不对前端开放。M1/M2 再加入带目标窗口和触发时间的请求、串行任务。 |
| `capture/uia.rs` | Windows 专用模块，预留 MTA 工作线程、选区读取与超时隔离。 |
| `capture/clipboard.rs` | Windows 专用模块，预留受限快照、复制归因、条件恢复。 |
| `capture/input.rs` | Windows 专用模块，预留修饰键等待、焦点复查、SendInput。 |
| `app_state.rs` | 预留最新取词、事件/查询补偿、任务失效和编辑保护；尚未增加运行时状态。 |

只有 capture 顶层定义跨层数据；原生通道保持私有。HWND 与 COM 对象不得进入序列化结果。合约代码暂时局部允许 dead_code，实际接入时移除该例外。

## 下一步

M1 实现快捷键注册、冲突反馈、一次有效按下、立即捕获目标及触发记录。随后 M2 用记事本与已知纯文本剪贴板验证预填流程。

本次构建验证不能代表真实取词兼容性；尚未执行系统复制、修改剪贴板或调用模型服务。

## 验证记录（2026-10-26）

- `npm run build` 通过。
- `cargo test --locked --manifest-path src-tauri/Cargo.toml`：34 项通过，8 项真实服务测试按默认设置跳过。
- `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 通过。
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 通过。
- 成分标签与段落工作流检查脚本通过。

依赖接入方式参考 [Tauri 官方快捷键文档](https://v2.tauri.app/plugin/global-shortcut/) 和 [微软 windows-rs 文档](https://microsoft.github.io/windows-docs-rs/doc/windows/)。
