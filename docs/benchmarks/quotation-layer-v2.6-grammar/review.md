# 引语案例：提示词修复实测

模型：deepseek-v4-flash；温度：0.3；思考模式：False。两次使用同一保存配置，未修改用户配置。每个条件仅一轮，模型存在随机性，不能据此估计总体正确率。

本轮仅修改提示词及 promptVersion；没有改变分句、分词、协议、UI 或模型参数。旧版在本次重测中也正确识别出了宾语从句，不能把截图的错误当作本次旧版测量结果。

## 首屏对比（自动分句开启，两句合计）

|提示词|输入 Token|输出 Token|总 Token|HTTP 累计耗时|请求数|
|---|---:|---:|---:|---:|---:|
|原版 v2.5|1263|252|1515|3.67 秒|2|
|新版 v2.6-grammar|2815|280|3095|3.44 秒|2|

注意：耗时是顺序诊断请求的 HTTP 耗时之和，不是桌面端并发首屏耗时；Token 来自服务实际 usage，含缓存输入 Token。提示词变长，输入用量增加。

## 新版真实结构（自动分句开启）

### Referencing the title of the film, Night Flower, Zeteo editor-in-chief Mehdi Hasan says AIPAC “blossoms in the dark, shrivels in the light.

|文本|role|kind / ctype|
|---|---|---|
|Referencing the title of the film, Night Flower,|adverbial|phrase / participial|
|Zeteo editor-in-chief Mehdi Hasan|subject|phrase / none|
|says|predicate|word / none|
|AIPAC “blossoms in the dark, shrivels in the light|object|clause / noun|
|.|punctuation|word / none|

模型说明：句首分词状语，逻辑主语为主句主语。；引号内为宾语从句，含并列谓语。

### We’re trying to put some sunlight onto AIPAC.”

|文本|role|kind / ctype|
|---|---|---|
|We’re|subject|phrase / none|
|trying to put some sunlight onto AIPAC|predicate|phrase / none|
|.”|punctuation|word / none|

模型说明：引号并入句末标点；谓语为进行体加不定式

### AIPAC “blossoms in the dark, shrivels in the light

|文本|role|kind / ctype|
|---|---|---|
|AIPAC|subject|phrase / none|
|“blossoms in the dark, shrivels in the light|predicate|phrase / coordinate|

模型说明：引号并入谓语短语；共享主语的两个谓语并列

### trying to put some sunlight onto AIPAC

|文本|role|kind / ctype|
|---|---|---|
|trying|predicate|word / none|
|to put some sunlight onto AIPAC|complement|phrase / infinitive|

模型说明：不定式短语作trying的补足语

### “blossoms in the dark, shrivels in the light

|文本|role|kind / ctype|
|---|---|---|
|“blossoms in the dark|coordinate|phrase / none|
|,|punctuation|word / none|
|shrivels in the light|coordinate|phrase / none|

模型说明：引号并入首分句；两个并列谓语共享主语

### “blossoms in the dark

展开失败：本层修复后仍未通过校验：没有缩小目标范围；请返回更小的直接成分，或 n=[] 并说明已无法继续拆分。 上一层结果已保留。

### shrivels in the light

|文本|role|kind / ctype|
|---|---|---|
|shrivels|predicate|word / none|
|in the light|adverbial|phrase / none|

模型说明：省略主语，与前一分句共享。；介词短语作地点状语。

## 已确认改善与剩余问题

- 新版首层将 Referencing ... Night Flower, 整体保留为分词状语；旧版本次自动分句条件下错误把 Night Flower 放进主语。
- 新版宾语从句内部保留 AIPAC 为主语，两个动词短语归于共享谓语，再拆为 coordinate 短语。
- 一处 predicate 短语仍输出 ctype=coordinate，UI 可能显示成并列分句；这属于角色/类型一致性问题，尚未增加硬校验。
- 一个以开引号起始的第四层目标，两次输出仍未缩小范围，已按现有机制失败降级。
- 某些 notes 仍使用“分句/省略主语”措辞；不能声称说明一致性已完全修复。
- 自动分句仍在引语中切断，第二句缺少跨句引语上下文；关闭分句时模型对两句关系的表示也尚未完全理想。
- 未实现 independent/appositive/fn/结构化 notes，本轮保持现有协议。

原始输出与完整指标见同目录 report.json；document-split-true.json 为此次实际模型结果，非手工样例。