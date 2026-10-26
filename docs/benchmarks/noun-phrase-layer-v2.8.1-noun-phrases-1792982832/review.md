# 名词短语教学层级改进（2026-10-26）

版本：`layer-v2.8.1-noun-phrases`。修改前文件在 `docs/prompt-backups/before-noun-phrase-20261026/`，含校验哈希。

## 实际结果

BBC 原句使用已保存的模型参数独立运行两轮，两轮都得到：

1. 整句：主语 / is（谓语）/ 完整名词短语（表语）。
2. 展开表语：`a British public service broadcaster`（核心名词短语 NP）＋ `headquartered at Broadcasting House in London, England.`（后置定语，过去分词短语）。
3. 展开核心：a（限定词·不定冠词）、British（前置定语）、public service（整体前置定语）、broadcaster（中心词）。

这种分层是教学展示方式；完整的表语仍是包含后置定语的名词短语，不把后置定语从语法归属上移出 NP。

同时检查了 `an old stone bridge built across the river`（不规则过去分词）、`the young scientist working in the laboratory`（现在分词）以及 `very proud of her daughter`（形容词短语）。三个对照均通过预设的层级/细分类断言，形容词补足语未被显示成名词后置定语。

## 校验与界面

- 最终一轮共 18 个分层请求，全部成功，零修复重试。原始结果及用量见同目录 report.json，提示词见 prompt.txt。
- 29 项 Rust 本地测试通过，6 项需显式运行的在线测试默认跳过；单独运行本次在线回归通过。
- 前端构建、成分标签脚本、Clippy（含所有 targets，警告视为错误）通过。
- 通过本地浏览器用实际 React 组件回放 report.json 的已获取结果，核对两块结构和下一层冠词细分类。回放使用默认颜色；不额外调用模型，不代表已构建新的桌面安装包。
- `np-overview.png` 和 `np-core.png` 是上述真实返回数据的界面截图，没有手动替换解析节点。

可复现命令：

```powershell
npm.cmd run build
node scripts/check-component-labels.mjs
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
# 以下命令调用已配置模型，产生 API 用量
cargo test --locked --manifest-path src-tauri/Cargo.toml noun_phrase_teaching_probe -- --ignored --nocapture
```

## 范围与限制

新增细分类为可选元数据，不改变原有角色颜色。限定词细分类按已确认的 determiner 角色和完整原文识别；未知类别保留通用名称。前后置修饰标签须有对应的名词中心依据，冲突标签退回通用显示。原有四层上限和按需请求机制保留；新的整体分组可能增加一次用户主动展开，不进行自动预取。

这些测试覆盖本次教学层级需求，不能证明所有英语句子或更深层的介词附着都正确。BBC 例句中进一步展开地点表达时，模型仍可能将地点短语分析为补足语或状语；此处没有强行替换模型判断。新结果需在更新程序后点击“重新分析”，已有会话不会自动重写。

参考：[Cambridge：名词短语的前后置修饰成分](https://dictionary.cambridge.org/us/grammar/british-grammar/noun-phrases-dependent-words)、[Cambridge：限定词类别](https://dictionary.cambridge.org/uk/grammar/british-grammar/modifiers)。这些来源支持术语区分；界面的分层粒度为本项目的教学设计。
