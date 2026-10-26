"""Summarize actual provider usage; keep retries and incomplete expansions visible."""
import csv
import json
import statistics
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
if report_path.is_dir():
    report_path /= "report.json"
report = json.loads(report_path.read_text(encoding="utf-8"))
if not report.get("complete"):
    raise SystemExit("Benchmark is not complete; refusing to publish partial aggregates")

conditions = {
    "v1_full": "v1 整句完整解析",
    "v2_first": "v2 首屏一层",
    "v2_expanded_to_depth4": "v2 按标记展开至四层（含首屏）",
}


def aggregate(rows):
    def total(key):
        values = [(row.get("totals") or {}).get(key) for row in rows]
        return sum(values) if all(value is not None for value in values) else None

    input_tokens, output_tokens = total("inputTokens"), total("outputTokens")
    return {
        "runs": len(rows),
        "completed": sum(row["status"] == "ok" for row in rows),
        "inputTokens": input_tokens,
        "outputTokens": output_tokens,
        "totalTokens": None if input_tokens is None or output_tokens is None else input_tokens + output_tokens,
        "cachedInputTokens": total("cachedInputTokens"),
        "requests": total("requests"),
        "retryRequests": total("retryRequests"),
        "firstPassRuns": sum(row["status"] == "ok" and (row.get("totals") or {}).get("retryRequests") == 0 for row in rows),
        "meanMs": statistics.mean(row["elapsedMs"] for row in rows),
        "medianMs": statistics.median(row["elapsedMs"] for row in rows),
    }


summary = {key: aggregate([row for row in report["rows"] if row["condition"] == key]) for key in conditions}
base, first = summary["v1_full"], summary["v2_first"]
changes = {}
for key in ["inputTokens", "outputTokens", "totalTokens", "meanMs"]:
    changes[key] = None if base[key] is None or first[key] is None else (first[key] / base[key] - 1) * 100
result = {"conditions": summary, "v2FirstChangePercent": changes}
(report_path.parent / "summary.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")

with (report_path.parent / "summary.csv").open("w", encoding="utf-8-sig", newline="") as handle:
    writer = csv.writer(handle)
    writer.writerow(["round", "sample", "condition", "status", "input_tokens", "output_tokens", "total_tokens", "requests", "retry_requests", "elapsed_ms"])
    for row in report["rows"]:
        values = row.get("totals") or {}
        i, o = values.get("inputTokens"), values.get("outputTokens")
        writer.writerow([row["round"], row["sample"], row["condition"], row["status"], i, o, None if i is None or o is None else i + o, values.get("requests"), values.get("retryRequests"), round(row["elapsedMs"], 2)])


def number(value):
    return "未完整提供" if value is None else f"{value:,.0f}"


lines = [
    "# v1 / v2 同条件 Token 对比", "",
    f"模型：`{report['model']}`；服务：`{report['endpointHost']}`；温度 {report['temperature']}；思考关闭；JSON mode：{report['jsonMode']}；输出上限：{report.get('maxOutputTokens', '按版本默认预算')}。",
    f"每版使用相同的 {len(report['samples'])} 条公开样例，运行 {report['rounds']} 轮，交替执行顺序；计入修复重试。v2 版本：`{report['v2Prompt']}`。", "",
    "## 汇总（全部运行累计 Token）", "",
    "| 条件 | 成功/运行 | 输入 Token | 输出 Token | 总 Token | 请求/修复 | 平均耗时 |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
]
for key, name in conditions.items():
    a = summary[key]
    lines.append(f"| {name} | {a['completed']}/{a['runs']} | {number(a['inputTokens'])} | {number(a['outputTokens'])} | {number(a['totalTokens'])} | {number(a['requests'])}/{number(a['retryRequests'])} | {a['meanMs']/1000:.2f} 秒 |")
lines += ["", "v2 首屏相对 v1：" + "；".join(f"{label} {changes[key]:+.1f}%" for key, label in [("inputTokens", "输入"), ("outputTokens", "输出"), ("totalTokens", "总 Token"), ("meanMs", "平均耗时")] if changes[key] is not None) + "。", ""]
lines += [
    "## 每条样例（两轮平均，含重试）", "",
    "| 样例 | v1 总 Token | v2 首屏总 Token | v2 展开至四层总 Token | v1 耗时 | v2 首屏耗时 |",
    "| --- | ---: | ---: | ---: | ---: | ---: |",
]
for sample_id, text in report["samples"]:
    parts = {key: aggregate([row for row in report["rows"] if row["condition"] == key and row["sample"] == sample_id]) for key in conditions}
    def avg(key):
        a = parts[key]
        return "未完整提供" if a["totalTokens"] is None else f"{a['totalTokens']/a['runs']:,.1f}"
    lines.append(f"| {sample_id} | {avg('v1_full')} | {avg('v2_first')} | {avg('v2_expanded_to_depth4')} | {parts['v1_full']['meanMs']/1000:.2f} 秒 | {parts['v2_first']['meanMs']/1000:.2f} 秒 |")

lines += ["", "## 样例", ""] + [f"- **{sample_id}**：{text}" for sample_id, text in report["samples"]]
lines += [
    "", "## 解释与边界", "",
    "- 首屏与整句完整解析的工作量不同。此表反映实际产品路径的成本，不能作为语言学分析等价的证明。",
    "- 展开至四层只遍历模型标记为 expandable 的节点，最多四层；不是无限深完整语法树。未完成的展开会保留父层并列出错误。",
    "- v2 的每次展开会重新发送提示词和上下文，累计输入可能显著增加。因此默认不自动预取，只在用户点击时请求。",
    "- 缓存回访经过 Store 验证，命中不发 HTTP 请求，新增 Token 为 0。服务端提示缓存则只影响输入计费方式，不能从 Token 总数中扣除；原始报告保留 cachedInputTokens。",
    "- 无效可选连线和过长说明会省略并提示。程序只规范化单 token 的词性类型别名，不移动范围或编造结构来通过校验。",
    f"- v1 无修复完成：{base['firstPassRuns']}/{base['runs']}；v2 首屏无修复完成：{first['firstPassRuns']}/{first['runs']}。这是小样本结构校验结果，不是语法准确率，也未证明达到 95% 的长期目标。",
    "- 未返回的推理 Token 保持 null，不能推断为 0；两个条件均显式关闭思考。未返回 usage 的失败调用标为未知，不伪造为 0。",
    "- 此轮仅有六条样例、两轮重复，服务负载和模型随机性会影响结果。应继续使用更大的长句集和人工语法标注回归。",
    "- 原始 report.json 包含逐次使用量、成功/失败、规范化后的结构及缓存验证结果。summary.csv 可供独立复算。",
]
if report.get("manualReview"):
    lines += ["", "## 输出抽查与已知语法问题", ""] + ["- " + item for item in report["manualReview"]]
errors = [row for row in report["rows"] if row["status"] != "ok"]
if errors:
    lines += ["", "## 未完成项", ""]
    for row in errors:
        lines.append(f"- 第 {row['round']} 轮 / {row['sample']} / {row['condition']}：{json.dumps(row.get('errors', row.get('error')), ensure_ascii=False)}")
(report_path.parent / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(json.dumps(result, ensure_ascii=False, indent=2))
