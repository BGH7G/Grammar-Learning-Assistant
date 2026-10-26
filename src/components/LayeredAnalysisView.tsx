import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AnalysisLayer, AnalysisMetrics, ExpansionResult, LayerDocument, LayerNode, LayerRole } from "../types/analysis";
import Icon from "./Icon";
import { componentRole, componentType, componentHelp, coordinatedPredicate, predicateSubject, isPunctuationOnly } from "../componentLabels";
const roles: Record<LayerRole, string> = { root:'整句',main_clause:'主句',subject:'主语',predicate:'谓语',object:'宾语',attribute:'定语',adverbial:'状语',complement:'补语',predicative:'表语',conjunction:'连接词',coordinate:'并列成分',head:'中心词',determiner:'限定词',marker:'结构标记',punctuation:'标点',other:'其他成分' };
const readableError = (e:unknown) => typeof e === 'string' ? e : '本层解析失败，请稍后重试。';

export default function LayeredAnalysisView({ document:doc, onMetrics }: { document:LayerDocument; onMetrics:(m:AnalysisMetrics)=>void }) {
  const [layers,setLayers]=useState<Record<string,AnalysisLayer>>(() => Object.fromEntries(doc.layers.map(l=>[l.parentId,l])));
  const [selected,setSelected]=useState(doc.roots[0].id);
  const [busyId,setBusyId]=useState<string|null>(null);
  const [error,setError]=useState(''); const [cacheReads,setCacheReads]=useState(0);
  const [,setHover]=useState<string|null>(null);
  const studyRef=useRef<HTMLDivElement>(null);
  const alive=useRef(true); const pending=useRef(false); const navigation=useRef(0);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  const nodes=useMemo(()=>new Map([...doc.roots,...Object.values(layers).flatMap(l=>l.nodes)].map(n=>[n.id,n])),[doc.roots,layers]);
  const parent=nodes.get(selected)!; const layer=layers[selected]; const root=nodes.get(parent.sentenceId)!;

  const [coreOnly,setCoreOnly]=useState(false);
  useEffect(()=>{if(selected!==root.id)studyRef.current?.focus({preventScroll:true});},[selected,root.id]);
  const trail:LayerNode[]=[]; let cursor:LayerNode|undefined=parent;
  while(cursor){trail.unshift(cursor);cursor=cursor.parentId ? nodes.get(cursor.parentId) : undefined;}
  const snippet=(n:LayerNode)=>doc.pieces.slice(doc.tokens[n.first].pieceId,doc.tokens[n.last].pieceId+1).map(p=>p.text).join('');
  const canExpand=(n:LayerNode)=>n.first<n.last && n.depth<doc.maxDepth && n.kind!=='word';
  function navigate(nodeId:string) { navigation.current++;setSelected(nodeId);setError('');setHover(null);if(layers[nodeId])setCacheReads(v=>v+1); }
  async function open(node:LayerNode) {
    if(layers[node.id]){navigate(node.id);return;}
    if(pending.current){navigate(node.id);return;}
    setSelected(node.id);
    if(!canExpand(node)){navigation.current++;setError('');return;}
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
  const rootLayer=layers[root.id];
  const visible=(items:LayerNode[])=>items.filter(n=>!isPunctuationOnly(snippet(n)));
  const topNodes=visible(rootLayer.nodes);
  const childNodes=visible(layer?.nodes||[]);
  const mislabeledPunctuation=[...topNodes,...childNodes].some(n=>n.role==='punctuation');
  const roleLabel=(n:LayerNode)=>componentRole(n,n.parentId?nodes.get(n.parentId):undefined,n.parentId?layers[n.parentId]?.nodes:[],snippet(n));
  const typeLabel=(n:LayerNode)=>componentType(n,layers[n.id]);
  const isParallel=coordinatedPredicate(parent,layer);
  const sharedSubject=predicateSubject(parent,nodes,layers);
  const selectedOwner=parent.parentId?nodes.get(parent.parentId):undefined;
  const isPredicatePart=parent.role==='coordinate'&&parent.kind!=='clause'&&selectedOwner?.role==='predicate';
  const parallelCount=layer?.nodes.filter(n=>n.role==='coordinate').length||0;
  const suspicious=parent.role==='predicate'&&(parent.kind==='clause'||layer?.nodes.some(n=>n.role==='coordinate'&&n.kind==='clause'));

  const roleHelp:Partial<Record<LayerRole,string>>={subject:'主语，表示句子谈论的人、事物或对象。',predicate:'谓语，表达主语的动作或状态。',object:'宾语，是动作涉及的对象或内容。',attribute:'定语，修饰或限定名词性成分。',adverbial:'状语，补充动作或状态的时间、原因、方式等背景。',main_clause:'主句，承载这句话的主要陈述。',predicative:'表语，说明主语的身份、性质或状态。',complement:'补语，补充说明相关成分。',conjunction:'连接词，连接词语或分句。'};
  const description=isParallel?`同一主语的 ${parallelCount} 个并列谓语部分，分别表达动作或状态。`:isPredicatePart?'这是并列谓语中的一个部分，可继续查看其中的动词、宾语和修饰成分。':parent.ctype==='adverbial'?'状语从句，为主句补充背景、条件、原因或时间等信息；具体含义需结合上下文。':roleHelp[parent.role]||`${roles[parent.role]}，可结合下方结构与原文理解。`;
  const segments:{node?:LayerNode;text:string;first:number}[]=[];
  for(const piece of doc.pieces.slice(doc.tokens[root.first].pieceId,doc.tokens[root.last].pieceId+1)){
    const node=topNodes.find(n=>piece.id>=doc.tokens[n.first].pieceId&&piece.id<=doc.tokens[n.last].pieceId);
    const last=segments[segments.length-1];
    if(last&&last.node?.id===node?.id)last.text+=piece.text;
    else segments.push({node,text:piece.text,first:piece.id});
  }
  const topSelected=topNodes.find(n=>parent.first>=n.first&&parent.last<=n.last&&parent.id!==root.id);
  const head=parent.parentId?layers[parent.parentId]?.relations.find(r=>r.sourceId===parent.id)?.headToken:undefined;
  function keyboard(e:React.KeyboardEvent<HTMLElement>){
    if(e.target instanceof HTMLInputElement||e.target instanceof HTMLSelectElement)return;
    if(e.key==='Escape'&&parent.parentId){e.preventDefault();navigate(parent.parentId);}
    if(e.key==='ArrowLeft'||e.key==='ArrowRight'){
      const siblings=parent.parentId?visible(layers[parent.parentId]?.nodes||[]):topNodes;
      if(!siblings.length)return;
      e.preventDefault();const index=siblings.findIndex(n=>n.id===parent.id);const next=(index+(e.key==='ArrowRight'?1:-1)+siblings.length)%siblings.length;
      void open(siblings[next]);
    }
    if(e.key==='Enter'&&e.target===e.currentTarget){e.preventDefault();const next=childNodes.find(n=>canExpand(n));if(next)void open(next);}
  }
  return <div ref={studyRef} className="sentence-study" onKeyDown={keyboard} tabIndex={-1}>
    {doc.roots.length>1&&<nav className="study-tabs" aria-label="句子选择">{doc.roots.map((r,i)=><button key={r.id} className={r.id===root.id?'active':''} aria-current={r.id===root.id?'page':undefined} onClick={()=>navigate(r.id)}>句 {i+1}</button>)}</nav>}
    <section className="card study-overview" aria-label="分层解析"><div className="section-head"><h2>分层解析</h2><div className="study-mode" aria-label="原文显示模式"><button aria-pressed={!coreOnly} onClick={()=>setCoreOnly(false)}>全句</button><button aria-pressed={coreOnly} onClick={()=>setCoreOnly(true)}>主干</button></div></div>
    <div className="sentence study-sentence" aria-label="可点击的完整原句">{segments.map(segment=>{
      const n=segment.node;if(!n)return <span key={segment.first}>{segment.text}</span>;
      const label=roleLabel(n);
      const structure=typeLabel(n);
      return <span key={segment.first} className={`piece study-segment ${n.role} ${topSelected?.id===n.id?'selected':''} ${coreOnly&&['attribute','adverbial','other','marker'].includes(n.role)?'core-muted':''}`} role="button" tabIndex={0} aria-pressed={topSelected?.id===n.id} aria-label={`${label}${structure?' · '+structure:''}：${segment.text}`} onClick={()=>void open(n)} onKeyDown={e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();e.stopPropagation();void open(n);}}} onMouseEnter={()=>setHover(n.id)} onMouseLeave={()=>setHover(null)}><span className="study-segment-label">{label}{structure&&<span className="study-structure-label"> · {structure}</span>}{canExpand(n)?' ›':''}</span>{segment.text}</span>;
    })}</div>
    <p className="study-translation">{rootLayer.translation}</p>
    {mislabeledPunctuation&&<p className="field-note layer-warning">部分含文字的片段被模型误标为标点，已保留原文并标记“待核对”。请重新分析。</p>}</section>
    {parent.id!==root.id?<section className="card study-detail" aria-label="所选成分详情"><div className="study-detail-header"><nav className="breadcrumbs" aria-label="解析路径">{trail.map((n,i)=><span key={n.id}>{i>0&&<span>›</span>}<button title={snippet(n)} aria-current={i===trail.length-1?'location':undefined} onClick={()=>navigate(n.id)}>{i===0?'整句':roleLabel(n)}</button></span>)}</nav><span className="study-shortcuts">← → 切换　Enter 下钻　Esc 返回</span></div>
      {typeLabel(parent)&&<p className="study-type-caption">句子成分：{roleLabel(parent)}<span>语法类型：{typeLabel(parent)}</span></p>}
      {(isParallel||isPredicatePart)&&<div className="shared-subject-note"><strong>{sharedSubject?`共同主语：${snippet(sharedSubject)}`:'共同主语：当前结构未明确提供'}</strong><span>{sharedSubject?'各谓语部分共用这个主语，不是各有主语的独立分句。':'请结合上级结构判断，不自动借用其他分句的主语。'}</span></div>}
      {suspicious&&<p className="field-note layer-warning">模型将谓语范围标记为分句，结构分类存在冲突，请结合原文复核。</p>}
      <div className="study-explanation"><div><h3>作用</h3><p>{componentHelp(parent,snippet(parent))||description}</p>{head!==undefined&&<p>修饰中心词：<strong>{doc.tokens[head].text}</strong></p>}</div><div><h3>译文</h3><p>{layer?.translation||(busyId===parent.id?'正在获取所选片段的译文…':'此成分暂无单独译文，可参考上方整句翻译。')}</p></div></div>
      {childNodes.length>0&&<><h3 className="study-children-caption">内部结构（点击可继续下钻）</h3><div className="study-children">{childNodes.map(n=><button key={n.id} className={`piece study-child ${n.role}`} onClick={()=>void open(n)}><span>{roleLabel(n)}{typeLabel(n)&&<span className="study-structure-label"> · {typeLabel(n)}</span>}{canExpand(n)?' ›':''}</span><span>{snippet(n)}</span></button>)}</div></>}
      {!layer&&canExpand(parent)&&!busyId&&<button className="button secondary" onClick={()=>void open(parent)}>加载内部结构<Icon name="chevron" size={13}/></button>}
      {parent.depth>=doc.maxDepth&&parent.first<parent.last&&<p className="field-note">已展示到当前可分析的最细结构。</p>}
      {layer?.notes.length? <ul className="study-notes">{layer.notes.map((note,i)=><li key={i}>{note}</li>)}</ul>:null}
    </section>:<p className="study-select-hint"><Icon name="info" size={14}/>点击句中色块，查看作用、译文与内部结构。未加载的结构会调用模型。</p>}
    {busyId&&<p className="hint" role="status"><span className="spinner"/>正在解析所选成分…</p>}
    {error&&<div className="notice error" role="alert">{error}</div>}
    {(layer?.warnings?.length||rootLayer.notes.length)?<details className="study-supplement"><summary>本句说明</summary>{rootLayer.notes.map((note,i)=><p key={i}>{note}</p>)}{layer?.warnings?.map((w,i)=><p key={`w${i}`}>{w}</p>)}</details>:null}
    <details className="study-supplement"><summary>结构详情</summary><span className="muted">缓存读取 {cacheReads} · 当前深度 {layer?.depth||parent.depth} / {doc.maxDepth}</span></details>
  </div>;
}
