# v1 / v2 同条件 Token 对比

模型：`deepseek-v4-flash`；服务：`api.deepseek.com`；温度 0.3；思考关闭；JSON mode：False；输出上限：8192。
每版使用相同的 6 条公开样例，运行 2 轮，交替执行顺序；计入修复重试。v2 版本：`layer-v2.5`。

## 汇总（全部运行累计 Token）

| 条件 | 成功/运行 | 输入 Token | 输出 Token | 总 Token | 请求/修复 | 平均耗时 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| v1 整句完整解析 | 12/12 | 6,772 | 4,036 | 10,808 | 12/0 | 2.11 秒 |
| v2 首屏一层 | 12/12 | 7,406 | 1,425 | 8,831 | 12/0 | 1.50 秒 |
| v2 按标记展开至四层（含首屏） | 12/12 | 48,153 | 6,829 | 54,982 | 79/4 | 8.75 秒 |

v2 首屏相对 v1：输入 +9.4%；输出 -64.7%；总 Token -18.3%；平均耗时 -29.1%。

## 每条样例（两轮平均，含重试）

| 样例 | v1 总 Token | v2 首屏总 Token | v2 展开至四层总 Token | v1 耗时 | v2 首屏耗时 |
| --- | ---: | ---: | ---: | ---: | ---: |
| simple | 524.0 | 632.0 | 632.0 | 1.53 秒 | 1.17 秒 |
| relative | 845.5 | 694.5 | 3,990.5 | 2.03 秒 | 1.34 秒 |
| coordination | 772.5 | 693.5 | 2,973.0 | 1.86 秒 | 1.48 秒 |
| nonfinite | 868.5 | 765.5 | 3,464.0 | 2.02 秒 | 1.61 秒 |
| long | 1,308.5 | 877.5 | 10,621.5 | 2.59 秒 | 1.78 秒 |
| attachment | 1,085.0 | 752.5 | 5,810.0 | 2.62 秒 | 1.60 秒 |

## 样例

- **simple**：The cat sleeps.
- **relative**：The cat that I saw yesterday sat on the mat.
- **coordination**：I wanted to leave, but the train was late.
- **nonfinite**：To finish the project on time, Maya worked late while her team reviewed the final report.
- **long**：Sweeping restrictions have been imposed on demonstrations across France but hundreds of thousands of protesters are still expected to gather in Paris and elsewhere for a second round of protests organised by students, trade unions and wider anti-government groups.
- **attachment**：The report published by the committee yesterday suggests that the measures introduced last year have reduced pollution.

## 解释与边界

- 首屏与整句完整解析的工作量不同。此表反映实际产品路径的成本，不能作为语言学分析等价的证明。
- 展开至四层只遍历模型标记为 expandable 的节点，最多四层；不是无限深完整语法树。未完成的展开会保留父层并列出错误。
- v2 的每次展开会重新发送提示词和上下文，累计输入可能显著增加。因此默认不自动预取，只在用户点击时请求。
- 缓存回访经过 Store 验证，命中不发 HTTP 请求，新增 Token 为 0。服务端提示缓存则只影响输入计费方式，不能从 Token 总数中扣除；原始报告保留 cachedInputTokens。
- 无效可选连线和过长说明会省略并提示。程序只规范化单 token 的词性类型别名，不移动范围或编造结构来通过校验。
- v1 无修复完成：12/12；v2 首屏无修复完成：12/12。这是小样本结构校验结果，不是语法准确率，也未证明达到 95% 的长期目标。
- 未返回的推理 Token 保持 null，不能推断为 0；两个条件均显式关闭思考。未返回 usage 的失败调用标为未知，不伪造为 0。
- 此轮仅有六条样例、两轮重复，服务负载和模型随机性会影响结果。应继续使用更大的长句集和人工语法标注回归。
- 原始 report.json 包含逐次使用量、成功/失败、规范化后的结构及缓存验证结果。summary.csv 可供独立复算。

## 输出抽查与已知语法问题

- 本次由助手抽查第一轮六条首层输出（非独立人工标注评测），发现 coordination 样例将逗号标为 conjunction、把 but 留在后一分句中。这是语法标注问题，范围/覆盖校验不能识别；首层 12/12 通过只表示结构有效。
- 主干概述由模型生成，不保证删除所有可省略修饰成分。类型标签也须结合 kind、父角色和实际展开结构理解。
- 当前仍须使用更大样本和人工标注继续评估语法准确性；本次没有声称通过 50–100 条人工语法回归。
