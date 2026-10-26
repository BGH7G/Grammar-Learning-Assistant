# 句子成分与语法类型统一展示（2026-10-26）

提示词版本：`layer-v2.9.2-grammar-types`。修改前文件保存在 `docs/prompt-backups/before-grammar-types-20261026/`，附 SHA-256 校验文件。

## 改动后的展示

保留成分角色和现有颜色，新增或补齐其后的语法类型。真实模型结果包括：

| 原文片段 | 界面标签 |
| --- | --- |
| may | 谓语 · 情态动词 |
| struggle | 谓语 · 实义动词 |
| has（has finished） | 谓语 · 助动词 |
| has（has a bicycle） | 谓语 · 实义动词 |
| is（is happy） | 谓语 · 系动词 |
| They | 主语 · 代词 |
| happy | 表语 · 形容词 |
| young | 前置定语 · 形容词 |
| scientist | 中心词 · 名词 |
| quietly | 状语 · 副词 |
| in London | 状语 · 介词短语 |
| in（展开介词短语后） | 中心词 · 介词 |
| London | 宾语 · 专有名词 |
| Well | 独立成分 · 话语标记 |
| to adjust | 宾语 · 不定式结构 |

句子层可以先将 may struggle 显示为一个谓语动词短语；展开后分别说明 may 和 struggle。两者仍共同组成同一个谓语。顶部原句和内部结构均使用同一套类型标签，所选详情中使用“语法类型”而非仅适用于短语/从句的“结构类型”。

## 校验和兼容

扩展既有 `d` 细分类字段，不新增角色枚举。词性标签须符合单词、角色与 ctype，短语形式须符合 phrase/none。冲突词类被移除，保留通用成分；专有名词优先于普通名词，关系代词优先于代词。旧响应缺少细分类仍可使用，无法确认时不按单词拼写猜测。

本次还补充了两个具体边界检查：`d` 使用本层 n 数组从零开始的序号，而非绝对 token ID；无效索引会提示并省略。kind=word 不可覆盖多个由空白分开的原词，连字符单词仍允许跨多个 token；不符合时沿用最多一次结构修复请求。

## 实测和验证

最终版本的 7 个公开样例包含情态动词、完成时助动词、实义 have、系动词、其他词类、名词 can 和话语标记。19 项预设分类检查全部通过，13 次分层请求成功，零重试。原始解析、实际用量及请求耗时见 report.json，独立提示词快照见 prompt.txt。

此前调试版本存在一次细分类遗漏和一次将绝对 token ID 填入子节点索引的问题，已保留对应历史报告；最终结论仅对应本目录的版本和样例。

- Rust 本地测试：31 项通过，7 项在线测试默认跳过；本次在线测试单独显式运行并通过。
- 前端构建与 `scripts/check-component-labels.mjs`：通过。
- Clippy：所有 targets，警告视为错误，通过。
- 实际 React 组件加载同目录 modal.json 回放后，点击谓语可见两个动词细分类；截图为 verb-types.png。回放使用真实返回结构，不额外调用模型，未制作新的桌面安装包。

可复现在线回归：

```powershell
cargo test --locked --manifest-path src-tauri/Cargo.toml grammar_type_teaching_probe -- --ignored --nocapture
```

该命令使用已保存的模型配置并产生 API 用量。模型输出仍有不确定性，这些小样例不能证明所有句子分类都正确；细分类冲突时保留通用标签。已有会话不会自动重写，更新程序后需重新分析。

语法术语参考：[Cambridge：动词类别及 be/have/do 的不同用法](https://dictionary.cambridge.org/grammar/british-grammar/main)、[British Council：动词短语](https://learnenglish.britishcouncil.org/free-resources/grammar/english-grammar-reference/verb-phrases)。
