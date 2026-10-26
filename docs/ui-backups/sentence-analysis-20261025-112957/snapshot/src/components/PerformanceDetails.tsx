import type { AnalysisMetrics } from "../types/analysis";
import Icon from "./Icon";
const ms = (value: number) => value >= 1000 ? `${(value / 1000).toFixed(2)} 秒` : `${value.toFixed(1)} ms`;
const tokens = (value: number | null) => value === null ? "服务未提供" : value.toLocaleString();
export default function PerformanceDetails({ metrics }: { metrics: AnalysisMetrics }) {
  return <details className="performance-details">
    <summary><Icon name="clock" size={14} />性能详情<span className="status-pill">{metrics.attempts.length} 次请求{metrics.attempts.some(a=>a.repairReason) ? " · 含修复重试" : ""}</span></summary>
    <div className="performance-body">
      <dl className="metrics-grid"><div><dt>累计后端耗时</dt><dd>{ms(metrics.totalMs)}</dd></div><div><dt>配置读取</dt><dd>{ms(metrics.configurationMs)}</dd></div><div><dt>本地切片与请求准备</dt><dd>{ms(metrics.preparationMs)}</dd></div></dl>
      {metrics.attempts.map((attempt, i) => <section className="attempt-metrics" key={i}>
        <h3>{attempt.stage || (i === 0 ? "首次请求" : "后续请求")}</h3>
        <dl className="metrics-grid">
          <div><dt>HTTP 总耗时</dt><dd>{ms(attempt.requestMs)}</dd></div><div><dt>等待响应</dt><dd>{ms(attempt.responseWaitMs)}</dd></div><div><dt>读取与解析响应</dt><dd>{ms(attempt.responseReadMs)}</dd></div><div><dt>结构校验</dt><dd>{ms(attempt.validationMs)}</dd></div>
          <div><dt>输入 token</dt><dd>{tokens(attempt.inputTokens)}</dd></div><div><dt>输出 token（含推理）</dt><dd>{tokens(attempt.outputTokens)}</dd></div><div><dt>其中推理 token</dt><dd>{tokens(attempt.reasoningTokens)}</dd></div>
        </dl>
        {attempt.repairReason && <p className="field-note">触发修复：{attempt.repairReason}</p>}
      </section>)}
      <p className="field-note">非流式请求的“等待响应”可能包含网络、服务排队、思考和生成时间，不能单独视为模型推理耗时。</p>
    </div>
  </details>;
}
