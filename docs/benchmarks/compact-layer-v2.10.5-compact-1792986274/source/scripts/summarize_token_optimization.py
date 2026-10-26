"""Summarize paired, identical-target comparisons using actual provider usage."""
import csv
import json
import statistics
import sys
from pathlib import Path

path = Path(sys.argv[1])
if path.is_dir():
    path /= "report.json"
report = json.loads(path.read_text(encoding="utf-8-sig"))
if not report.get("complete"):
    raise SystemExit("Comparison is incomplete; no final summary was generated")


def aggregate(rows):
    def total(key):
        values = [row["totals"].get(key) for row in rows]
        return sum(values) if all(value is not None for value in values) else None

    inputs, outputs = total("inputTokens"), total("outputTokens")
    checks = [check for row in rows for check in row.get("checks", [])]
    return {
        "runs": len(rows),
        "successfulRuns": sum(row["status"] == "ok" for row in rows),
        "qualityPassedRuns": sum(row["qualityPassed"] for row in rows),
        "checks": len(checks),
        "passedChecks": sum(check[1] for check in checks),
        "inputTokens": inputs,
        "outputTokens": outputs,
        "totalTokens": None if inputs is None or outputs is None else inputs + outputs,
        "cachedInputTokens": total("cachedInputTokens"),
        "requests": total("requests"),
        "retryRequests": total("retryRequests"),
        "meanHttpMsPerTarget": statistics.mean(row["totals"]["httpMs"] for row in rows) if rows else None,
    }


conditions = {name: aggregate([r for r in report["rows"] if r["condition"] == name]) for name in ("before", "after")}
before, after = conditions["before"], conditions["after"]
reductions = {key: None if before[key] in (None, 0) or after[key] is None else (1 - after[key] / before[key]) * 100
              for key in ("inputTokens", "outputTokens", "totalTokens", "meanHttpMsPerTarget")}
by_scope = {scope: {name: aggregate([r for r in report["rows"] if r["condition"] == name and (r["parent"]["depth"] == 0) == (scope == "first")])
                   for name in ("before", "after")} for scope in ("first", "expansion")}
summary = {"conditions": conditions, "reductionsPercent": reductions, "byScope": by_scope}
(path.parent / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")

with (path.parent / "summary.csv").open("w", encoding="utf-8-sig", newline="") as handle:
    writer = csv.writer(handle)
    writer.writerow(["round", "case", "condition", "status", "quality_passed", "input_tokens", "output_tokens", "cached_input_tokens", "requests", "retry_requests", "http_ms"])
    for row in report["rows"]:
        t = row["totals"]
        writer.writerow([row["round"], row["case"], row["condition"], row["status"], row["qualityPassed"], t.get("inputTokens"), t.get("outputTokens"), t.get("cachedInputTokens"), t["requests"], t["retryRequests"], round(t["httpMs"], 2)])


def number(value):
    return "服务未完整提供" if value is None else f"{value:,.0f}"


lines = [
    "# 保持功能的 Token 优化对比", "",
    f"原版：`{report['beforeVersion']}`；优化版：`{report['afterVersion']}`。",
    f"模型 `{report['model']}`，温度 {report['temperature']}，思考 {report['thinking']}，推理强度 `{report['reasoningEffort']}`，JSON mode {report['jsonMode']}。",
    f"固定相同的 {report['caseCount']} 个目标，分别运行 {report['rounds']} 轮；每对请求交替执行前后版本。参数、目标范围、父节点、深度、输出预算、校验器和修复上限一致。", "",
    "## 实际用量（包含所有修复重试）", "",
    "| 项目 | 原版 | 优化版 |", "| --- | ---: | ---: |",
]
for label, key in [("成功目标数", "successfulRuns"), ("功能检查通过目标数", "qualityPassedRuns"), ("输入 Token", "inputTokens"), ("输出 Token", "outputTokens"), ("总 Token", "totalTokens"), ("已缓存输入 Token（仍属于输入）", "cachedInputTokens"), ("HTTP 请求数", "requests"), ("其中修复请求", "retryRequests")]:
    lines.append(f"| {label} | {number(before[key])} | {number(after[key])} |")
lines += [f"| 每个目标平均 HTTP 耗时，含修复 | {before['meanHttpMsPerTarget']/1000:.3f} 秒 | {after['meanHttpMsPerTarget']/1000:.3f} 秒 |", ""]
for key, label in [("inputTokens", "输入 Token"), ("totalTokens", "总 Token")]:
    if reductions[key] is not None:
        lines.append(f"{label}减少 **{reductions[key]:.2f}%**。")
lines += ["", "## 首层与展开分别计算", "", "| 范围 | 原版总 Token | 优化版总 Token |", "| --- | ---: | ---: |"]
for scope, label in [("first", "整句首层"), ("expansion", "固定目标展开")]:
    lines.append(f"| {label} | {number(by_scope[scope]['before']['totalTokens'])} | {number(by_scope[scope]['after']['totalTokens'])} |")
lines += ["", "## 质量与功能边界", "",
    f"原版预设检查：{before['passedChecks']}/{before['checks']}；优化版：{after['passedChecks']}/{after['checks']}。",
    "检查覆盖：情态/实义/助动词标签、表语 NP 的完整边界、核心 NP 和过去分词后置定语、冠词和复合前置定语、分词内部、共享主语并列谓语、including 与 as 补足语、跨引号的报告内容、关系从句、目的不定式、独立并列分句、存在句与四层上限。",
    "每个版本处理相同的固定目标；不依赖两版自行生成不同的展开数量来制造低用量。回访缓存、不预取、输出协议、语法类型/颜色、设置、交互及错误处理机制保留。",
    "优化移除重复说明和大量示例，但保留关键边界的简短正反例。请求仅发送一份目标 token，子层附完整原句纯文本上下文，保留精确目标文本和绝对 ID。",
    "缓存命中数没有从 Token 总数中扣除。这里衡量 Token 使用量，不直接推断账单下降比例；缓存/非缓存价格由服务商决定。耗时是请求层数据，不是桌面程序端到端首屏耗时。",
    "少量固定样例不能证明所有句子的语法准确率。修改提示词前后的原始返回、错误、重试及每项检查均保留在 report.json；不通过的调试轮次保留在其他目录，不并入最终统计。", "",
    "## 文件与复现", "",
    "- before-prompt.txt / after-prompt.txt：本轮独立提示词快照。",
    "- report.json：逐次实际响应结构、用量、耗时、校验和检查结果。",
    "- summary.json / summary.csv：可独立复算的数据。",
    "- 修改前备份：docs/prompt-backups/before-token-optimization-20261026/。", "",
    "```powershell",
    "cargo test --locked --manifest-path src-tauri/Cargo.toml compact_token_comparison -- --ignored --nocapture",
    "python scripts/summarize_token_optimization.py docs/benchmarks/<本次报告目录>",
    "```", "",
    "上述回归会调用已配置服务并产生用量，不更改保存的模型参数。GRAMMAR_TOKEN_CASES 可选逗号分隔的目标名用于诊断，GRAMMAR_TOKEN_ROUNDS 可设为 1 或 2；默认测试全部目标两轮。",
]
failures = [r for r in report["rows"] if r["status"] != "ok" or not r["qualityPassed"]]
if failures:
    lines += ["", "## 未通过项", ""]
    for row in failures:
        failed = [str(c[0]) for c in row.get("checks", []) if not c[1]]
        lines.append(f"- {row['condition']} / 第 {row['round']} 轮 / {row['case']}：{row.get('error') or ', '.join(failed)}")
(path.parent / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(json.dumps(summary, ensure_ascii=False, indent=2))
