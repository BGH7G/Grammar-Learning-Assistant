import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import LayeredAnalysisView from "./components/LayeredAnalysisView";
import Icon, { type IconName } from "./components/Icon";
import PerformanceDetails from "./components/PerformanceDetails";
import type { LayerDocument, AnalysisMetrics, AnalysisResult, Settings, ReasoningEffort, ThinkingProtocol } from "./types/analysis";
import { layerSample } from "./layerSample";
import ColorSettings, { loadPalette, paletteKey, paletteStyle, type Palette } from "./components/ColorSettings";
import "./App.css";

const empty: Settings = { endpoint: "", model: "", jsonMode: false, hasKey: false, temperature: 0.3, thinking: false, reasoningEffort: "high", thinkingProtocol: "auto" };
function supportsThinking(s: Settings) {
  if (s.thinkingProtocol === "omit") return false;
  if (s.thinkingProtocol !== "auto") return true;
  try { return new URL(s.endpoint.trim()).hostname === "api.deepseek.com"; } catch { return false; }
}
const message = (e: unknown) => typeof e === "string" ? e : "操作失败，请稍后重试。";

function SettingCard({ icon, title, description, children, disabled = false, id }: {
  icon: IconName; title: string; description?: string; children: ReactNode; disabled?: boolean; id?: string;
}) {
  return <section className={`setting-card${disabled ? " is-disabled" : ""}`} aria-labelledby={id ? `${id}-title` : undefined}>
    <Icon name={icon} className="setting-icon" size={20} />
    <div className="setting-body"><h2 id={id ? `${id}-title` : undefined}>{title}</h2>{description && <p className="description">{description}</p>}{children}</div>
  </section>;
}

export default function App() {
  const [page, setPage] = useState<"analysis" | "settings" | "colors">("analysis");
  const [palette, setPalette] = useState(loadPalette);
  const [paletteError, setPaletteError] = useState("");
  function changePalette(next: Palette) {
    setPalette(next);
    try { localStorage.setItem(paletteKey, JSON.stringify(next)); setPaletteError(""); }
    catch { setPaletteError("配色已应用，但保存失败；重启后可能恢复原配色。"); }
  }
  const [service, setService] = useState<"model" | "sample">("model");
  const [settings, setSettings] = useState<Settings>(empty);
  const [draft, setDraft] = useState<Settings>(empty);
  const [key, setKey] = useState("");
  const [clearKey, setClearKey] = useState(false);
  const [text, setText] = useState("");
  const [result, setResult] = useState<LayerDocument | null>(null);
  const [editing, setEditing] = useState(false);
  const [splitSentences, setSplitSentences] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [noticeError, setNoticeError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [configBusy, setConfigBusy] = useState(false);
  const [ready, setReady] = useState(false);
  const [, setElapsed] = useState<number | null>(null);
  const [metrics, setMetrics] = useState<AnalysisMetrics | null>(null);
  const [liveElapsed, setLiveElapsed] = useState(0);
  const [testElapsed, setTestElapsed] = useState<number | null>(null);
  const current = useRef("");
  const count = Array.from(text).length;

  useEffect(() => {
    if (!isTauri()) { setNotice("浏览器预览：可查看离线示例；模型配置与分析请使用桌面应用。"); return; }
    invoke<Settings>("get_settings").then(s => { setSettings(s); setDraft(s); setReady(true); })
      .catch(e => { setNotice(message(e)); setNoticeError(true); setReady(true); });
  }, []);

  useEffect(() => {
    if (!busy) { setLiveElapsed(0); return; }
    const started = performance.now();
    const timer = window.setInterval(() => setLiveElapsed((performance.now() - started) / 1000), 250);
    return () => window.clearInterval(timer);
  }, [busy]);

  function editText(value: string) {
    current.current = ""; setBusy(false); setText(value); setResult(null); setError(""); setElapsed(null); setMetrics(null);
  }
  function demo() { editText(layerSample.pieces.map(p => p.text).join("")); setResult(layerSample); setEditing(false); setPage("analysis"); }
  async function analyze() {
    if (!text.trim() || count > 2000) return;
    if (!settings.endpoint) { setError("请先在“模型接入”中保存接口配置，或使用离线示例。"); return; }
    const id = crypto.randomUUID(); const started = performance.now();
    current.current = id; setBusy(true); setError(""); setResult(null); setElapsed(null); setMetrics(null);
    try {
      const r = await invoke<AnalysisResult>("analyze_sentence", { text, requestId: id, splitSentences });
      if (current.current === r.requestId) { setResult(r.document); setEditing(false); setMetrics(r.metrics); setElapsed((performance.now() - started) / 1000); }
    } catch (e) { if (current.current === id) setError(message(e)); }
    finally { if (current.current === id) setBusy(false); }
  }
  async function save() {
    setConfigBusy(true); setNotice(""); setNoticeError(false); setTestElapsed(null);
    try {
      await invoke("save_settings", { settings: { endpoint: draft.endpoint, model: draft.model, jsonMode: draft.jsonMode, temperature: draft.temperature, thinking: draft.thinking, reasoningEffort: draft.reasoningEffort, thinkingProtocol: draft.thinkingProtocol }, apiKey: key || null, clearKey });
      const s = await invoke<Settings>("get_settings");
      setSettings(s); setDraft(s); setKey(""); setClearKey(false); setNotice("配置已保存。"); current.current = ""; setBusy(false); setResult(null); setMetrics(null); setElapsed(null);
    } catch (e) { setNotice(message(e)); setNoticeError(true); }
    finally { setConfigBusy(false); }
  }
  const dirty = draft.endpoint !== settings.endpoint || draft.model !== settings.model || draft.jsonMode !== settings.jsonMode || draft.temperature !== settings.temperature || draft.thinking !== settings.thinking || draft.reasoningEffort !== settings.reasoningEffort || draft.thinkingProtocol !== settings.thinkingProtocol || !!key || clearKey;
  const thinkingSupported = supportsThinking(draft);
  const thinkingActive = thinkingSupported && draft.thinking;
  const testDisabled = !ready || configBusy || dirty || !settings.endpoint;
  async function test() {
    const started = performance.now(); setConfigBusy(true); setNoticeError(false); setNotice("正在测试已保存的配置…"); setTestElapsed(null);
    try { setNotice(await invoke<string>("test_connection")); }
    catch (e) { setNotice(message(e)); setNoticeError(true); }
    finally { setConfigBusy(false); setTestElapsed((performance.now() - started) / 1000); }
  }
  function addMetrics(m: AnalysisMetrics) {
    setMetrics(old => old ? { totalMs: old.totalMs + m.totalMs, configurationMs: old.configurationMs + m.configurationMs, preparationMs: old.preparationMs + m.preparationMs, attempts: [...old.attempts,...m.attempts] } : m);
  }
  function openSettings() { setService("model"); setPage("settings"); }

  return <div className="app-shell" style={paletteStyle(palette)}>
    <aside className="sidebar">
      <div className="brand"><span className="brand-mark"><Icon name="grammar" size={28} /></span><div>Grammar Assistant<small>英语语法助手</small></div></div>
      <div className="nav-caption">工作空间</div>
      <nav aria-label="主导航">
        <button className={`nav-item ${page === "analysis" ? "active" : ""}`} aria-current={page === "analysis" ? "page" : undefined} onClick={() => setPage("analysis")}><Icon name="document" /><span>句子分析</span></button>
        <button className={`nav-item ${page === "settings" ? "active" : ""}`} aria-current={page === "settings" ? "page" : undefined} onClick={openSettings}><Icon name="settings" /><span>模型接入</span></button>
        <button className={`nav-item ${page === "colors" ? "active" : ""}`} aria-label="成分配色" aria-current={page === "colors" ? "page" : undefined} onClick={() => setPage("colors")}><Icon name="palette" /><span>成分配色</span></button>
      </nav>
      <div className="sidebar-bottom"><div className="connection-status"><span className={`status-dot ${settings.endpoint ? "configured" : ""}`} />{settings.endpoint ? "模型已配置" : "尚未配置模型"}</div><span className="sidebar-model" title={settings.model}>{settings.model || "在模型接入中添加服务"}</span><span className="version">版本 0.1.0 · Windows</span></div>
    </aside>
    <main className={page === "settings" ? "main settings-main" : "main analysis-main"}>
      <header className="page-header" hidden={page === "colors"}><div><h1>{page === "analysis" ? "句子分析" : page === "colors" ? "成分配色" : "模型接入"}</h1><p>{page === "analysis" ? "点击句中的色块，逐层查看结构、作用与译文。" : page === "colors" ? "自定义句子成分的文字色与背景色。" : "配置分析服务，使用你自己的模型。"}</p></div><span className="status-pill"><span className={`status-dot ${busy || configBusy ? "working" : ""}`} />{busy ? "正在分析" : configBusy ? "处理中" : "就绪"}</span></header>
      {page === "analysis" ? <div className="analysis-content">
        {(!result || editing) && <section className="card input-card"><div className="section-head"><div className="heading-with-icon"><Icon name="document" /><h2>英文句子</h2></div><button className="button subtle" onClick={demo}><Icon name="sample" size={16} />离线示例</button></div>
          <label className="sr-only" htmlFor="sentence">英文句子</label><textarea id="sentence" value={text} onChange={e => editText(e.target.value)} placeholder="输入或粘贴英文句子…" aria-describedby="input-hint character-count" />
          <div className="input-footer"><span id="character-count" className={count > 2000 ? "error-text" : "muted"}>{count.toLocaleString()} / 2,000 字符</span><div className="button-row"><button className="button secondary" onClick={() => editText("")} disabled={!text}><Icon name="trash" size={16} />清空</button><button className="button primary" disabled={busy || !text.trim() || count > 2000 || !ready} onClick={analyze}><Icon name="play" size={16} />{busy ? "正在分析…" : "分析句子"}</button></div></div>
          <label className="checkbox-row sentence-split"><input type="checkbox" checked={splitSentences} onChange={e=>{setSplitSentences(e.target.checked);editText(text);}}/>自动分句（最多 8 句；缩写或特殊标点可关闭后分析）</label>
          <p id="input-hint" className="hint"><Icon name="info" size={14} />分析时，原文将发送至已配置的模型服务。</p>
        </section>}
        {result && !editing && <div className="card study-source-bar"><span className="study-source-summary" title={text}>{text}</span><span className="muted">共 {result.roots.length} 句 · {count} 字符</span><button className="button secondary" onClick={()=>setEditing(true)}>编辑原文</button><button className="button secondary" disabled={busy || !ready || !settings.endpoint} onClick={analyze}>重新分析</button></div>}
        {!result && <div className="analysis-toolbar"><span className="muted">当前模型：{settings.model || "未配置"}</span><button className="button subtle" onClick={openSettings}><Icon name="settings" size={15} />模型设置<Icon name="chevron" size={13} /></button></div>}
        {error && <div role="alert" className="notice error"><Icon name="info" size={17} />{error}</div>}
        {busy && <div className="card loading-state" role="status"><span className="spinner" />正在分析句子结构…<span className="status-pill">已等待 {liveElapsed.toFixed(1)} 秒</span></div>}
        {result ? <><LayeredAnalysisView key={result.sessionId} document={result} onMetrics={addMetrics} />{metrics && <PerformanceDetails metrics={metrics} />}</> : !busy && <section className="empty"><span className="empty-icon"><Icon name="grammar" size={38} /></span><h2>从一个句子开始</h2><p>输入英文后点击分析，或先查看离线示例。</p><button className="button secondary" onClick={demo}><Icon name="sample" size={16} />查看示例</button></section>}
        <p className="workspace-note"><Icon name="info" size={14} />结构校验不等于语法正确，请结合上下文判断。修改输入或保存模型配置会使旧会话失效；已发出的请求仍可能计费。</p>
      </div> : page === "colors" ? <ColorSettings palette={palette} onChange={changePalette} error={paletteError} /> : <div className="settings-layout">
        <section className="service-column" aria-label="分析服务列表"><div className="list-heading"><h2>分析服务</h2><span className="count-badge">2</span></div>
          <button className={`service-item ${service === "model" ? "selected" : ""}`} aria-pressed={service === "model"} onClick={() => setService("model")}><span className="service-icon"><Icon name="server" size={21} /></span><span className="service-info"><strong>自定义模型</strong><small title={settings.model}>{settings.model || "Chat Completions"}</small></span><span className="source-tag custom">自定义</span>{service === "model" && <Icon name="check" size={14} className="selected-check" />}</button>
          <button className={`service-item ${service === "sample" ? "selected" : ""}`} aria-pressed={service === "sample"} onClick={() => setService("sample")}><span className="service-icon"><Icon name="sample" size={21} /></span><span className="service-info"><strong>离线示例</strong><small>查看语法分析效果</small></span><span className="source-tag built-in">内置</span>{service === "sample" && <Icon name="check" size={14} className="selected-check" />}</button>
          <div className="service-explainer"><Icon name="info" size={16} /><p>支持 Chat Completions 兼容服务。模型配置保存在本机，可随时修改。</p></div>
          <div className="list-footer"><Icon name="shield" size={15} /><span>密钥由 Windows 凭据管理器保存</span></div>
        </section>
        <div className="configuration-column">
          <div className="configuration-heading"><span className="heading-with-icon"><Icon name={service === "model" ? "server" : "sample"} size={17} /><h2>{service === "model" ? "自定义模型" : "离线示例"}</h2><span className={`source-tag ${service === "model" ? "custom" : "built-in"}`}>{service === "model" ? "自定义" : "内置"}</span></span><span className="status-pill">{service === "sample" ? "无需联网" : dirty ? "未保存" : settings.endpoint ? "已保存" : "待配置"}</span></div>
          {service === "model" ? <>
            <SettingCard icon="globe" title="接口地址" description="填写完整的 Chat Completions 端点，不会自动追加路径。" id="endpoint"><label className="sr-only" htmlFor="endpoint">完整接口地址</label><input id="endpoint" type="url" value={draft.endpoint} disabled={configBusy} onChange={e => setDraft({ ...draft, endpoint: e.target.value })} placeholder="https://your-provider.com/v1/chat/completions" spellCheck={false} /><p className="field-note">远程服务使用 HTTPS；本机服务支持 HTTP。</p></SettingCard>
            <SettingCard icon="key" title="API 密钥" description="留空保留当前端点的密钥；无需鉴权的本机服务可留空。" id="api-key"><label className="sr-only" htmlFor="api-key">API Key</label><input id="api-key" type="password" autoComplete="new-password" spellCheck={false} value={key} disabled={configBusy || clearKey} onChange={e => setKey(e.target.value)} placeholder={draft.endpoint === settings.endpoint && settings.hasKey ? "已保存密钥 · 输入可替换" : "输入 API Key"} /><label className="checkbox-row"><input type="checkbox" checked={clearKey} disabled={configBusy} onChange={e => { setClearKey(e.target.checked); setKey(""); }} />保存时清除此端点的密钥</label></SettingCard>
            <SettingCard icon="model" title="模型" description="填写服务商提供的精确模型 ID。" id="model"><label className="sr-only" htmlFor="model">模型 ID</label><input id="model" value={draft.model} disabled={configBusy} onChange={e => setDraft({ ...draft, model: e.target.value })} placeholder="例如：服务商提供的模型名称" spellCheck={false} /></SettingCard>
            <SettingCard icon="temperature" title="温度控制" description={thinkingActive ? "思考模式开启时不发送温度参数，关闭后恢复已保存的数值。" : "控制输出的随机性。语法分析建议使用较低温度。"} disabled={thinkingActive}>
              <div className="range-row"><div className="range-control"><label className="sr-only" htmlFor="temperature">温度</label><input id="temperature" type="range" min="0" max="2" step="0.1" value={draft.temperature} disabled={configBusy || thinkingActive} onChange={e => setDraft({ ...draft, temperature: Number(e.target.value) })} style={{ "--range-fill": `${draft.temperature / 2 * 100}%` } as CSSProperties} /></div><output htmlFor="temperature">{draft.temperature.toFixed(1)}</output></div>
            </SettingCard>
            <SettingCard icon="thinking" title="思考模式" description="开启后模型先进行推理，通常耗时更长。单句分析默认关闭。">
              <label className="switch-row"><input className="switch" role="switch" type="checkbox" checked={thinkingActive} disabled={configBusy || !thinkingSupported} onChange={e => setDraft({ ...draft, thinking: e.target.checked })} aria-label="启用思考模式" /><span>{thinkingActive ? "开启" : "关闭"}</span></label>
              {!thinkingSupported && <p className="field-note">当前未发送思考参数。若服务支持，请在下方指定参数格式。</p>}
              <details className="protocol-options"><summary>接口参数兼容设置</summary><label htmlFor="thinking-protocol">思考参数格式</label><select id="thinking-protocol" value={draft.thinkingProtocol} disabled={configBusy} onChange={e => setDraft({ ...draft, thinkingProtocol: e.target.value as ThinkingProtocol })}><option value="auto">自动（识别 DeepSeek 官方接口）</option><option value="deepseek">DeepSeek 兼容 · thinking</option><option value="reasoning_effort">reasoning_effort · none/low/high/max</option><option value="omit">不发送（服务商默认）</option></select><p className="field-note">代理或本机服务请按其文档选择。自动模式对其他端点不发送思考参数。</p></details>
            </SettingCard>
            <SettingCard icon="effort" title="推理强度" description={thinkingActive ? "Low 较快，High 较深入，Max 适合复杂任务。" : "开启思考模式后可调整。保留所选值，关闭时不发送。"} disabled={!thinkingActive}>
              <label className="sr-only" htmlFor="reasoning-effort">推理强度</label><select id="reasoning-effort" value={draft.reasoningEffort} disabled={configBusy || !thinkingActive} onChange={e => setDraft({ ...draft, reasoningEffort: e.target.value as ReasoningEffort })}><option value="low">Low · 较快</option><option value="high">High · 默认</option><option value="max">Max · 深度</option></select>
            </SettingCard>
            <SettingCard icon="code" title="JSON 模式" description="服务支持 JSON mode 时启用，可提高结构化输出的稳定性。"><label className="switch-row"><input className="switch" role="switch" type="checkbox" checked={draft.jsonMode} disabled={configBusy} onChange={e => setDraft({ ...draft, jsonMode: e.target.checked })} aria-label="启用 JSON 模式" /><span>{draft.jsonMode ? "已启用" : "已关闭"}</span></label><p className="field-note">关闭时仍会校验模型输出与原文是否一致。</p></SettingCard>
            <div className="save-bar"><span className="muted">{dirty ? "更改尚未保存" : "修改完成后保存配置"}</span><button className="button primary" disabled={!ready || configBusy || !draft.endpoint.trim() || !draft.model.trim()} onClick={save}><Icon name="save" size={16} />{configBusy ? "处理中…" : "保存配置"}</button></div>
            <SettingCard icon="shield" title="连接验证" description={dirty ? "请先保存更改，再测试模型连接。" : settings.endpoint ? "向已保存的服务发送简短请求，验证连接是否可用。" : "保存接口地址和模型后，可测试连接。"} disabled={testDisabled}><div className="verification-controls"><button className="button secondary" disabled={testDisabled} onClick={test}><Icon name="play" size={16} />测试连接</button>{testElapsed !== null && <span className="status-pill"><Icon name="clock" size={13} />{testElapsed.toFixed(1)} 秒</span>}</div><p className="field-note">测试可能产生少量模型调用费用。</p></SettingCard>
          </> : <>
            <SettingCard icon="sample" title="语法分析示例" description="通过固定句子查看主句成分、中文翻译和定语从句。"><div className="sample-preview">The cat that I saw yesterday sat on the mat.</div><button className="button primary" onClick={demo}><Icon name="play" size={16} />打开示例</button></SettingCard>
            <SettingCard icon="info" title="关于此示例" description="内置样例无需配置模型，不发送网络请求。"><p className="field-note">如需分析自己的句子，请选择“自定义模型”并保存服务配置。</p></SettingCard>
          </>}
          {notice && <div className={`notice ${noticeError ? "error" : ""}`} role={noticeError ? "alert" : "status"}><Icon name="info" size={17} /><span>{notice}</span></div>}
        </div>
      </div>}
      {page === "analysis" && notice && <div className={`notice global-notice ${noticeError ? "error" : ""}`} role="status"><Icon name="info" size={17} /><span>{notice}</span></div>}
    </main>
  </div>;
}
