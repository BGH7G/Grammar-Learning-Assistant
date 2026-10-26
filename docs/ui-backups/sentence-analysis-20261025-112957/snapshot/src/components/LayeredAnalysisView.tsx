import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AnalysisLayer, AnalysisMetrics, ExpansionResult, LayerDocument, LayerNode, LayerRole, ClauseType } from "../types/analysis";
import Icon from "./Icon";
const roles: Record<LayerRole, string> = { root:'整句',main_clause:'主干分句',subject:'主语',predicate:'谓语',object:'宾语',attribute:'定语',adverbial:'状语',complement:'补语',predicative:'表语',conjunction:'连接词',coordinate:'并列成分',head:'中心词',determiner:'限定词',marker:'结构标记',punctuation:'标点',other:'其他成分' };
const ctypes: Record<ClauseType,string> = { main:'主句',coordinate:'并列分句',relative:'关系从句',noun:'名词性从句',adverbial:'状语从句',infinitive:'不定式结构',participial:'分词结构',gerund:'动名词结构',none:'' };
const nodeLabel = (n:LayerNode) => n.kind==='clause'||['infinitive','participial','gerund'].includes(n.ctype) ? ctypes[n.ctype]||roles[n.role] : roles[n.role];
const typeLabel = (n:LayerNode) => n.kind==='phrase'&&['relative','noun','adverbial'].includes(n.ctype) ? '含'+ctypes[n.ctype] : ctypes[n.ctype];
const readableError = (e:unknown) => typeof e === 'string' ? e : '本层解析失败，请稍后重试。';

export default function LayeredAnalysisView({ document:doc, onMetrics }: { document:LayerDocument; onMetrics:(m:AnalysisMetrics)=>void }) {
  const [layers,setLayers]=useState<Record<string,AnalysisLayer>>(() => Object.fromEntries(doc.layers.map(l=>[l.parentId,l])));
  const [selected,setSelected]=useState(doc.roots[0].id);
  const [busyId,setBusyId]=useState<string|null>(null);
  const [error,setError]=useState(''); const [cacheReads,setCacheReads]=useState(0);
  const [hover,setHover]=useState<string|null>(null);
  const alive=useRef(true); const pending=useRef(false); const navigation=useRef(0);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  const nodes=useMemo(()=>new Map([...doc.roots,...Object.values(layers).flatMap(l=>l.nodes)].map(n=>[n.id,n])),[doc.roots,layers]);
  const parent=nodes.get(selected)!; const layer=layers[selected]; const root=nodes.get(parent.sentenceId)!;
  const rootLayer=layers[root.id];
  const trail:LayerNode[]=[]; let cursor:LayerNode|undefined=parent;
  while(cursor){trail.unshift(cursor);cursor=cursor.parentId ? nodes.get(cursor.parentId) : undefined;}
  const snippet=(n:LayerNode)=>doc.pieces.slice(doc.tokens[n.first].pieceId,doc.tokens[n.last].pieceId+1).map(p=>p.text).join('');
  const canExpand=(n:LayerNode)=>n.first<n.last && n.depth<doc.maxDepth && n.kind!=='word';
  function navigate(nodeId:string) { navigation.current++;setSelected(nodeId);setError('');setHover(null);setCacheReads(v=>v+1); }
  async function open(node:LayerNode) {
    if(layers[node.id]){navigate(node.id);return;}
    if(pending.current)return;
    if(doc.sessionId==='offline'){setError('此离线成分没有更深的示例结构。');return;}
    const ticket=++navigation.current; const requestId=crypto.randomUUID();
    pending.current=true;setBusyId(node.id);setError('');
    try {
      const response=await invoke<ExpansionResult>('expand_node',{sessionId:doc.sessionId,nodeId:node.id,requestId});
      if(!alive.current || response.sessionId!==doc.sessionId || response.requestId!==requestId)return;
      if(response.cached)setCacheReads(v=>v+1);else onMetrics(response.metrics);
      if(response.error)throw response.error;
      const nextLayer=response.layer;
      if(!nextLayer || nextLayer.parentId!==node.id)throw '返回结果不属于当前成分。';
      setLayers(old=>({...old,[node.id]:nextLayer}));
      if(navigation.current===ticket){setSelected(node.id);setHover(null);}
    } catch(e) { if(alive.current && navigation.current===ticket)setError(readableError(e)); }
    finally { if(alive.current){pending.current=false;setBusyId(null);} }
  }
  const hovered=hover ? nodes.get(hover) : undefined;
  const head=layer?.relations.find(r=>r.sourceId===hover)?.headToken;
  // Keep punctuation in the source and protocol spans, but omit standalone punctuation from the UI.
  const visibleNodes=layer.nodes.filter(n=>n.role!=='punctuation' && !/^[\p{P}\s]+$/u.test(snippet(n)));
  const visibleIds=new Set(visibleNodes.map(n=>n.id));
  const visibleRelations=layer.relations.filter(r=>visibleIds.has(r.sourceId));
  const visibleRoles=[...new Set(visibleNodes.map(n=>n.role))];
  return <section className="card layered-view" aria-label="分层语法解析">
    <div className="section-head"><div className="heading-with-icon"><Icon name="document" /><h2>分层解析</h2></div><div className="layer-status"><span className="status-pill">第 {layer?.depth || 1} / {doc.maxDepth} 层</span><span className="status-pill">缓存读取 {cacheReads}</span></div></div>
    {doc.roots.length>1 && <div className="sentence-tabs" aria-label="句子选择">{doc.roots.map((r,i)=><button key={r.id} className={`button ${r.id===root.id?'primary':'secondary'}`} onClick={()=>navigate(r.id)}>句 {i+1}</button>)}</div>}
    {rootLayer.skeleton && <div className="skeleton-view"><span className="muted">主干概述 · 省略修饰成分</span><p>{rootLayer.skeleton}</p></div>}
    <nav className="breadcrumbs" aria-label="解析路径">{trail.map((n,i)=><span key={n.id}>{i>0&&<Icon name="chevron" size={12}/>}<button title={snippet(n)} aria-current={i===trail.length-1?'location':undefined} onClick={()=>navigate(n.id)}>{i===0?`整句${doc.roots.length>1?' '+(doc.roots.indexOf(root)+1):''}`:nodeLabel(n)}</button></span>)}</nav>
    <div className="sentence layered-source" aria-label="原文高亮">{doc.pieces.map(piece=>{
      const child=visibleNodes.find(n=>piece.id>=doc.tokens[n.first].pieceId && piece.id<=doc.tokens[n.last].pieceId);
      const inTarget=piece.id>=doc.tokens[parent.first].pieceId && piece.id<=doc.tokens[parent.last].pieceId;
      const inHover=hovered && piece.id>=doc.tokens[hovered.first].pieceId && piece.id<=doc.tokens[hovered.last].pieceId;
      const isHead=head!==undefined && piece.id===doc.tokens[head].pieceId;
      return <span key={piece.id} className={`${child?'piece '+child.role:''} ${!inTarget||hovered&&!inHover&&!isHead?'dimmed':''} ${isHead?'relation-head':''}`} title={child?roles[child.role]:undefined}>{piece.text}</span>;
    })}</div>
    <div className="legend">{visibleRoles.map(role=><span className={`piece ${role}`} key={role}>{roles[role]}</span>)}</div>
    <div className="layer-translation"><h3>{parent.depth===0?'中文翻译':'当前片段翻译'}</h3><p>{layer.translation}</p></div>
    <div className="component-list" aria-label="当前层直接成分">{visibleNodes.map(node=>{
      const cached=!!layers[node.id]; const expandable=canExpand(node); const loading=busyId===node.id;
      return <div className={`component-row ${hover===node.id?'is-highlighted':''}`} key={node.id} onMouseEnter={()=>setHover(node.id)} onMouseLeave={()=>setHover(null)} onFocus={()=>setHover(node.id)} onBlur={()=>setHover(null)}>
        <div className="component-description"><div className="component-labels"><span className={`piece ${node.role}`}>{roles[node.role]}</span>{node.ctype!=='none'&&<span className="clause-label">{typeLabel(node)}</span>}</div><p>{snippet(node)}</p></div>
        {cached||expandable?<button className="button secondary expand-button" disabled={!!busyId&&!cached} onClick={()=>open(node)} aria-label={`${cached?'查看已缓存结构':'展开'}：${snippet(node)}`}>{loading?<><span className="spinner"/>解析中</>:<>{cached?'查看结构':node.expandable?'展开':'进一步查看'}<Icon name="chevron" size={13}/></>}</button>:<span className="muted component-terminal">{node.depth>=doc.maxDepth&&node.first<node.last?'已达层级上限':'基础成分'}</span>}
      </div>;
    })}</div>
    {visibleNodes.length===0&&<p className="field-note">此片段没有需要继续展开的直接成分。</p>}
    {visibleRelations.length>0&&<div className="attachment-list"><h3>修饰关系</h3>{visibleRelations.map((relation,i)=>{const source=nodes.get(relation.sourceId)!;return <p key={i} onMouseEnter={()=>setHover(source.id)} onMouseLeave={()=>setHover(null)}><span>{snippet(source)}</span><Icon name="chevron" size={13}/><strong>{doc.tokens[relation.headToken].text}</strong></p>;})}</div>}
    {layer.warnings?.map((warning,i)=><p key={i} className="field-note layer-warning"><Icon name="info" size={13}/> {warning}</p>)}
    {layer.notes.length>0&&<ul className="layer-notes">{layer.notes.map((note,i)=><li key={i}>{note}</li>)}</ul>}
    {layer.depth>=doc.maxDepth&&<p className="field-note">已达四层解析上限；更细的结构请结合原文和本层说明理解。</p>}
    {busyId&&<p role="status" className="hint"><span className="spinner"/>正在解析所选成分，当前结果仍可阅读。</p>}
    {error&&<div className="notice error" role="alert"><Icon name="info" size={16}/><span>{error}</span></div>}
    <p className="field-note">悬停成分可定位原文。只在点击展开时请求模型，已加载结构保留在当前会话中。</p>
  </section>;
}
