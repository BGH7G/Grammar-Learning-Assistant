# 分层解析协议 v2

当前提示词版本：`layer-v2.9.2-grammar-types`。本地源数据始终无损保存；模型仅生成一层结构。

## 模型输入与输出

输入中的 `tokens` 按原文顺序排列，例如 `['0:The','1:bird','2:sings','3:.']`（实际传输为合法 JSON 双引号字符串）。标签中的数字是绝对 ID。空白不参与模型标注，但仍保存在 `pieces` 中；每个 token 通过 `pieceId` 映射回原文。标点独立保留，连字符可由多个 token 组成连续范围。

请求包含 `target:[first,last]`、`parent` 的 `kind/role/ctype/grammar` 与本次 `depth`。`tokens` 提供整句上下文，`target_tokens` 和 `target_text` 明确本次分析及翻译范围。父节点信息仅为待核对的提示。所有内容通过 JSON 数据字段发送，不拼接为系统指令。

```json
{
  "v": 2,
  "sk": "The bird sings.",
  "tr": "这只鸟在歌唱。",
  "n": [
    ["phrase", "subject", "none", 0, 1, 0],
    ["word", "predicate", "none", 2, 3, 0]
  ],
  "r": [],
  "d": [[0, "noun_phrase"]],
  "notes": []
}
```

节点数组为 `[kind,role,ctype,first,last,expandable]`。模型不生成节点 ID 或 parent ID，程序按会话、句子、父节点路径分配唯一 ID。`main` 表示作为整体保留的主干分句，程序规范化为 `main_clause`，避免与名词短语的 `head` 混淆。

`r` 是可选修饰边 `[本层源节点序号,目标中心词绝对 token ID]`。目标须位于当前分析范围内且在源片段之外；修饰边和父子包含关系分别保存。UI 在所选成分详情显示关联的中心词。

`d` 为可选教学细分类 `[本层源节点序号,标签]`，不改变六项节点数组和配色角色。允许 `noun_phrase`、`premodifier`、`postmodifier`、`past_participial`、`present_participial`；同一节点可有多个兼容标签。后端校验节点类型、修饰边方向与名词中心依据，剔除冲突细分类并给出提示，旧响应缺少 `d` 时仍可解析。删除纯标点节点后，`r` 和 `d` 同步重映射序号。API 将有效标签放入节点的可选 `grammar` 数组。

从 v2.9.1 起，提示词要求始终返回 `d` 并填写有依据的细分类，解析器为兼容旧响应仍接受缺省字段。新增动词类别 `modal_verb / auxiliary_verb / linking_verb / lexical_verb`；基本词类 `noun / proper_noun / pronoun / relative_pronoun / adjective / adverb / preposition / numeral`；连接与标记 `coordinating_conjunction / subordinator / discourse_marker / infinitive_marker`；短语形式 `verb_phrase / adjective_phrase / adverb_phrase / prepositional_phrase`。界面统一按“成分 · 语法类型”展示，非谓语与从句类型优先于普通词类，避免将“宾语 · 不定式结构”降为“宾语 · 动词短语”。

词类只附着于符合条件的单词节点，普通短语形式只附着于 `phrase/none`；动词类别须属于谓语、动词中心或并列动词。冲突词类或短语形式退回通用标签，专有名词优先于普通名词、关系代词优先于代词。`be/have/do` 根据上下文由模型分析，前端不按拼写猜测。`may struggle` 在句子层仍作为一个谓语组，展开该组可显示“谓语 · 情态动词 may”和“谓语 · 实义动词 struggle”，并说明二者共同组成同一谓语。四层上限仍然有效。

含后置修饰语的 NP 优先展示“核心名词短语＋后置定语”，核心部分使用 `phrase/head/none` 与 `noun_phrase` 细分类，UI 标为“核心名词短语 · NP”；它不是单个名词中心词。下一层才展开限定词、前置定语与名词中心。完整 NP 仍保留原来的主语、宾语或表语功能。冠词等细分类只对已标为 `determiner` 的片段按完整原文匹配：`a/an` 为不定冠词，`the` 为定冠词；其他已知类型包括物主、指示、数量、分配限定词。未知类型保留“限定词”，不把同形的代词或从句标记改成限定词，也不制造原文中不存在的零冠词节点。

`sk` 仅首层提供，是省略修饰成分的主干概述，不宣称与原文逐字一致。超过 35 词时省略并提示；`tr` 为本层片段翻译。说明最多显示两条、每条不超过 60 个 Unicode 字符。无效可选边、过长说明会省略并提示，不为这些展示信息重新生成整层。

## 严格校验

- 六项数组类型、角色和结构类型枚举有效。
- 节点范围有序、不重叠、完全覆盖本层所有 token，包含标点。
- 单次展开必须产生更小范围；不能返回唯一一个与父范围相同的节点。终止时返回空节点数组并说明。
- ID 和 parent 由程序生成，禁止模型创建重复 ID、跨会话父节点或环。
- 对单个 token 的词性结构类型别名（如 conjunction）可规范化为 word；不修改范围、不猜测缺失片段、不改写原文来强行通过校验。
- 本层最多修复一次；失败保留上一层，并保留已知 Token 统计。HTTP 失败未提供 usage 时显示未知。

## 命令与缓存

- `analyze_sentence(text,requestId,splitSentences)` 返回 `document` 和 `metrics`。自动分句可关闭，最多 8 句，最多两个首层请求同时执行。
- `expand_node(sessionId,nodeId,requestId)` 返回本层、metrics 和 cached。解析失败可返回 error 与 metrics，父层继续可用。
- 每次新分析创建新会话；当前会话按节点缓存。返回已加载节点不再请求模型。设置指纹包含提示词版本和模型参数，保存配置或更换密钥会清空缓存并提高配置修订号。
- 最多 4 层；默认不预取。每会话最多缓存 48 层，应用最多保留 8 个内存会话。退出应用后缓存消失。
- 同节点重复请求会被阻止；原文或配置变化后前端丢弃旧响应。丢弃响应不取消服务端费用。

## 展示与性能

首屏显示可点击完整句子、译文和本层成分；选中后在下方显示详情与内部结构，面包屑可返回，原句始终保留。主干概述仍在协议中返回但不重复显示。expandable 是建议；有多个 token 的连续短语可手动尝试进一步查看。超出四层不再请求模型。

性能详情累计显示首次请求、展开和修复的 Token 及耗时。多句并发时 HTTP 耗时的总和可能超过首屏墙钟时间。具体 v1/v2 实测以 `docs/token-comparison.md` 链接的最终报告为准。

尚未自动预取或流式渲染：逐层请求的累计输入可能更高，先以按需展开和缓存控制调用量。自动分句为保守规则，对缩写、特殊标点有歧义时可手动关闭。
