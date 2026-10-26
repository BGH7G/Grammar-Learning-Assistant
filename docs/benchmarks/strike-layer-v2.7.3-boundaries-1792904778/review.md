# 分词修饰语与 strike-as 结构修复实测

当前版本：layer-v2.7.3-boundaries。模型：deepseek-v4-flash；温度：0.3；思考模式：False。使用保存的配置，未修改模型设置。

## 本轮改动

- 统一 phrase/clause 与并列谓语规则；明确补充性分词修饰语不属于主句谓语。
- 明确 strike + 对象 + as 描述性补足成分；including 补充结构归入它所说明的宾语。
- 请求增加 target_text 和 target_tokens；完整句子 tokens 仅供范围外上下文参考。
- 后端拒绝实词误标标点，以及谓语/分句类型冲突；特定 strike-as 模式下拒绝 including 与 as 补足语被吞为一块。
- 标点归属规范化只移动纯标点的边界，不修复实词遗漏或重叠；修复节点索引后同步映射修饰关系。
- 前端仅按源文本过滤纯标点；旧结果中含文字的标点节点保留显示为“待核对”。

## 实测范围与结果

原句重复两次；同结构迁移例句一次；动词过去式与独立主语并列句各一条对照。对有目标结构的样本展开关键节点，最多四层；不是全量英语语法评测。

|样本|层查询数|HTTP 请求数|修复重试|输入 Token|输出 Token|首层 HTTP 耗时|
|---|---:|---:|---:|---:|---:|---:|
|original-1|7|7|0|56397|636|2.63 秒|
|original-2|7|7|0|56397|638|1.61 秒|
|transfer|5|5|0|40091|449|1.51 秒|
|finite-control|1|1|0|7961|78|1.15 秒|
|clause-control|1|1|0|7959|98|1.24 秒|

21 次层查询均返回可接受结构，实际 HTTP 请求 21 次。输入 Token 包含缓存输入，并非计费金额；当前提示词较长。

## 原句实际输出（第二轮）

### His political views, disseminated to his 240m followers on X, strike many as plain-speaking and strike many more, including The Economist, as plainly bigoted.

|原文|role|kind / ctype|
|---|---|---|
|His political views,|subject|phrase / none|
|disseminated to his 240m followers on X,|attribute|clause / participial|
|strike many as plain-speaking and strike many more, including The Economist, as plainly bigoted.|predicate|phrase / none|

模型译文：他的政治观点，在X上传播给他的2.4亿粉丝，让许多人觉得是直言不讳，也让更多人——包括《经济学人》——觉得明显偏执。

模型说明：分词短语补充说明主语 views；as 补足语说明主语给人的印象

### His political views,

|原文|role|kind / ctype|
|---|---|---|
|His|determiner|word / none|
|political|attribute|word / none|
|views,|head|word / none|

模型译文：他的政治观点，

模型说明：

### disseminated to his 240m followers on X,

|原文|role|kind / ctype|
|---|---|---|
|disseminated|predicate|word / none|
|to his 240m followers on X,|complement|phrase / none|

模型译文：传播给他在X上的2.4亿粉丝，

模型说明：过去分词短语作后置定语，逻辑主语是views；to短语表示传播对象

### strike many as plain-speaking and strike many more, including The Economist, as plainly bigoted.

|原文|role|kind / ctype|
|---|---|---|
|strike many as plain-speaking|coordinate|phrase / none|
|and|conjunction|word / none|
|strike many more, including The Economist, as plainly bigoted.|coordinate|phrase / none|

模型译文：让许多人觉得直言不讳，也让包括《经济学人》在内的更多许多人觉得明显偏执。

模型说明：两个并列谓语共用主语；as 短语说明主语给人的印象

### strike many as plain-speaking

|原文|role|kind / ctype|
|---|---|---|
|strike|predicate|word / none|
|many|object|word / none|
|as plain-speaking|complement|phrase / none|

模型译文：给许多人以直言不讳的印象

模型说明：as 补足语说明主语给人的印象

### strike many more, including The Economist, as plainly bigoted.

|原文|role|kind / ctype|
|---|---|---|
|strike|predicate|word / none|
|many more, including The Economist,|object|phrase / none|
|as plainly bigoted.|complement|phrase / none|

模型译文：让更多人——包括《经济学人》——觉得明显偏执

模型说明：as 补足语说明主语给人的印象；including 短语补充说明宾语

### many more, including The Economist,

|原文|role|kind / ctype|
|---|---|---|
|many|head|word / none|
|more,|attribute|word / none|
|including The Economist,|attribute|phrase / none|

模型译文：更多许多人，包括《经济学人》，

模型说明：including 短语补充说明 many more

## 复核结论与限制

两轮原句的主语、分词修饰语、并列谓语范围，以及两个 strike / object / as-complement 的拆分均符合本轮目标；including The Economist 在两轮中均保留于 many more 的宾语范围。迁移例句也满足同样边界。对照句中的有限动词 disseminated 没有被误当主语修饰语，两个各有主语的分句保留为 coordinate clauses。

仍不能把结构校验通过当作所有语法判断正确：原句第一轮整句译文加入“他”作为被评价对象，和 views 的语义指向不够一致；many more 的词组内部 head/attribute 划分仍可疑，译文“更多许多人”也不自然。没有针对这些输出进行手工替换，也没有把报告结果注入产品作为固定答案。

保留早期试验目录用于复核：v2.7.1 有标点覆盖失败；v2.7.2 曾把 including 并入 as 补足语并出现片段翻译超范围。最终结果只引用本目录，不能混用中间试验统计。

## 验证

27 项后端本地测试通过；组件角色、共享主语边界和实词可见性检查通过；前端生产构建通过。真实回归为人工检查的有限公开样本，不代表生产正确率。

原始记录：report.json；每个样本的完整树位于对应 JSON；prompt.txt 为当次实际系统提示词。源码快照及 SHA-256 记录在 source/ 和 source-sha256.json。