import {useEffect,useMemo,useRef,useState,type CSSProperties,type KeyboardEvent,type PointerEvent as ReactPointerEvent} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {AnalysisLayer,AnalysisMetrics,ExpansionResult,LayerDocument} from '../types/analysis';
import {HoverIntent,SentenceRequestQueue,type SentenceState} from '../analysisQueue';
import LayeredAnalysisView from './LayeredAnalysisView';
import PerformanceDetails from './PerformanceDetails';
import Icon from './Icon';

type Props={editing?:boolean;draftText?:string;onEditText?:(text:string)=>void;document:LayerDocument;metrics:AnalysisMetrics|null;onMetrics:(m:AnalysisMetrics)=>void;onLayerLoaded:(layer:AnalysisLayer)=>void;onLoadingChange?:(count:number)=>void;loadSentence?:(id:string)=>Promise<AnalysisLayer>};
const statusText={idle:'未分析',queued:'等待分析',loading:'分析中',ready:'已就绪',error:'分析失败，可重试'};

export default function ParagraphAnalysis({document:doc,editing=false,draftText,onEditText,metrics,onMetrics,onLayerLoaded,onLoadingChange,loadSentence}:Props){
  const initialLayers=()=>Object.fromEntries(doc.layers.map(l=>[l.parentId,l]));
  const [layers,setLayers]=useState<Record<string,AnalysisLayer>>(initialLayers);
  const [states,setStates]=useState<Record<string,SentenceState<AnalysisLayer>>>(()=>Object.fromEntries(doc.roots.map(root=>{
    const layer=doc.layers.find(l=>l.parentId===root.id);return [root.id,layer?{status:'ready',value:layer}:{status:'idle'}];
  })));
  const [selected,setSelected]=useState<string|null>(null);
  const [hovered,setHovered]=useState<string|null>(null);
  const [tooltip,setTooltip]=useState<{id:string;left:number;top:number}|null>(null);
  const [share,setShare]=useState(40);
  const [minimumShare,setMinimumShare]=useState(20);
  const queue=useRef<SentenceRequestQueue<AnalysisLayer>|null>(null);
  const hoverIntent=useRef(new HoverIntent());
  const stage=useRef<HTMLDivElement>(null);const drawer=useRef<HTMLElement>(null);
  const drawerBody=useRef<HTMLDivElement>(null);
  const inlineEditor=useRef<HTMLTextAreaElement>(null);
  const originalText=useMemo(()=>doc.pieces.map(p=>p.text).join(""),[doc.pieces]);
  const draftChanged=draftText!==undefined&&draftText!==originalText;
  useEffect(()=>{if(editing){stopHover();inlineEditor.current?.focus({preventScroll:true});}},[editing]);
  const sourceRefs=useRef(new Map<string,HTMLSpanElement>());
  const translationRefs=useRef(new Map<string,HTMLSpanElement>());
  const callbacks=useRef({onMetrics,onLayerLoaded,onLoadingChange,loadSentence});
  callbacks.current={onMetrics,onLayerLoaded,onLoadingChange,loadSentence};
  const commitLayer=(layer:AnalysisLayer)=>{setLayers(old=>({...old,[layer.parentId]:layer}));callbacks.current.onLayerLoaded(layer);};
  useEffect(()=>{setLayers(old=>({...old,...Object.fromEntries(doc.layers.map(l=>[l.parentId,l]))}));},[doc.layers]);
  useEffect(()=>{
    const cache=new Map(doc.layers.map(l=>[l.parentId,l]));
    const q=new SentenceRequestQueue(doc.roots.map(r=>r.id),async id=>{
      if(callbacks.current.loadSentence)return callbacks.current.loadSentence(id);
      if(doc.sessionId==='offline')throw '离线示例没有提供这句的解析。';
      const requestId=crypto.randomUUID();
      const response=await invoke<ExpansionResult>('expand_node',{sessionId:doc.sessionId,nodeId:id,requestId});
      if(response.requestId!==requestId||response.sessionId!==doc.sessionId)throw '返回结果不属于当前会话。';
      if(!response.cached)callbacks.current.onMetrics(response.metrics);
      if(response.error)throw response.error;
      if(!response.layer||response.layer.parentId!==id)throw '本句未返回有效的第一层结果。';
      return response.layer;
    },(id,state)=>{
      setStates(old=>({...old,[id]:state}));
      if(state.status==='ready'&&state.value)commitLayer(state.value);
    },cache);
    queue.current=q;q.enqueueAll();
    return()=>{q.dispose();hoverIntent.current.cancel();};
    // A queue belongs to one session; callbacks and subsequently loaded layers
    // are read through refs/cache, not used to recreate in-flight work.
  },[doc.sessionId]);
  const loading=Object.values(states).filter(s=>s.status==='loading').length;
  useEffect(()=>{callbacks.current.onLoadingChange?.(loading);},[loading]);
  useEffect(()=>()=>{callbacks.current.onLoadingChange?.(0);},[]);
  useEffect(()=>{if(selected){drawer.current?.focus({preventScroll:true});drawerBody.current?.scrollTo({top:0});}},[selected]);
  useEffect(()=>{
    if(!stage.current)return;
    const observer=new ResizeObserver(entries=>{
      if(entries[0].contentRect.height<=0)return;
      const min=Math.min(50,Math.max(20,170/Math.max(1,entries[0].contentRect.height)*100));
      setMinimumShare(min);setShare(value=>Math.max(min,value));
    });
    observer.observe(stage.current);return()=>observer.disconnect();
  },[]);
  const index=doc.roots.findIndex(r=>r.id===selected);
  const state=selected?states[selected]:undefined;
  const selectedLayer=selected?layers[selected]:undefined;
  const fullDocument=useMemo(()=>({...doc,layers:Object.values(layers)}),[doc,layers]);
  const readyCount=Object.values(states).filter(s=>s.status==='ready').length;
  const failedCount=Object.values(states).filter(s=>s.status==='error').length;
  const waitingCount=doc.roots.length-readyCount-failedCount;
  const parts=useMemo(()=>{
    const result:{id?:string;text:string}[]=[];let offset=0;
    for(const root of doc.roots){
      const first=doc.tokens[root.first].pieceId,last=doc.tokens[root.last].pieceId;
      if(first>offset)result.push({text:doc.pieces.slice(offset,first).map(p=>p.text).join('')});
      result.push({id:root.id,text:doc.pieces.slice(first,last+1).map(p=>p.text).join('')});offset=last+1;
    }
    if(offset<doc.pieces.length)result.push({text:doc.pieces.slice(offset).map(p=>p.text).join('')});
    return result;
  },[doc.pieces,doc.tokens,doc.roots]);
  function stopHover(){hoverIntent.current.cancel();setHovered(null);setTooltip(null);}
  function hover(id:string,target:HTMLElement){
    stopHover();setHovered(id);
    const rect=target.getBoundingClientRect();
    const position={id,left:Math.max(8,Math.min(rect.left,window.innerWidth-360)),top:Math.max(8,rect.top-40)};
    hoverIntent.current.run(()=>{
      setTooltip(position);queue.current?.enqueue(id,true);
      const counterpart=target.closest('.translation-text')?sourceRefs.current.get(id):translationRefs.current.get(id);
      counterpart?.scrollIntoView({block:'nearest',inline:'nearest'});
    });
  }
  function open(id:string){
    stopHover();setSelected(id);queue.current?.enqueue(id,true);
    if(selected===id)drawer.current?.focus({preventScroll:true});
    sourceRefs.current.get(id)?.scrollIntoView({block:'nearest',inline:'nearest'});
    translationRefs.current.get(id)?.scrollIntoView({block:'nearest',inline:'nearest'});
  }
  function close(){const id=selected;setSelected(null);stopHover();if(id)sourceRefs.current.get(id)?.focus({preventScroll:true});}
  function changeSentence(direction:number){const next=Math.min(doc.roots.length-1,Math.max(0,index+direction));open(doc.roots[next].id);}
  function sentenceKey(e:KeyboardEvent<HTMLSpanElement>,id:string){
    if(e.key==='Enter'||e.key===' '){e.preventDefault();open(id);}
  }
  function drawerKey(e:KeyboardEvent<HTMLElement>){
    if(e.target instanceof HTMLInputElement||e.target instanceof HTMLSelectElement||e.target instanceof HTMLTextAreaElement||e.altKey||e.ctrlKey||e.metaKey)return;
    if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close();}
    if(['ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(e.key)){
      e.preventDefault();e.stopPropagation();changeSentence(['ArrowUp','ArrowLeft'].includes(e.key)?-1:1);
    }
  }
  function resize(e:ReactPointerEvent<HTMLDivElement>){
    if(!e.currentTarget.hasPointerCapture(e.pointerId)||!stage.current)return;
    const box=stage.current.getBoundingClientRect();setShare(Math.min(65,Math.max(minimumShare,(e.clientY-box.top)/box.height*100)));
  }
  const interactions=(id:string)=>({onPointerEnter:(e:ReactPointerEvent<HTMLSpanElement>)=>{if(e.pointerType==='mouse')hover(id,e.currentTarget);},onPointerLeave:stopHover,onFocus:(e:React.FocusEvent<HTMLSpanElement>)=>hover(id,e.currentTarget),onBlur:stopHover,onClick:()=>open(id),onKeyDown:(e:KeyboardEvent<HTMLSpanElement>)=>sentenceKey(e,id)});
  return <div className={`paragraph-analysis ${selected?'has-selection':''}`} onKeyDownCapture={e=>{if(e.target instanceof HTMLTextAreaElement)return;if(e.key==='Escape'&&selected){e.preventDefault();e.stopPropagation();close();}}}>
    <div className="paragraph-progress" aria-live="polite">{draftChanged?`原文已修改${Array.from(draftText||'').length>2000?'，超出 2,000 字符上限':'，点击重新分析更新译文与解析'}；下方仍为修改前结果。`:waitingCount?`译文加载中 · ${readyCount} / ${doc.roots.length}`:failedCount?`已加载 ${readyCount} / ${doc.roots.length} · ${failedCount} 句失败，可单独重试`:'译文已就绪'}</div>
    <div className="paragraph-stage" ref={stage} style={{'--reading-share':`${share}%`} as CSSProperties}>
      <section className="card paragraph-reading" aria-label="原文与逐句译文" onWheel={stopHover} onTouchMove={stopHover}>
        <div className="paragraph-original"><h2>原文</h2>{editing?<textarea id="paragraph-inline-editor" ref={inlineEditor} className="paragraph-text paragraph-inline-editor" aria-label="编辑原文" value={draftText??originalText} onChange={e=>onEditText?.(e.target.value)} spellCheck={false}/>:<div className="paragraph-text" aria-label="点击句子查看语法">{parts.map((part,i)=>{
          if(!part.id)return <span key={`gap-${i}`}>{part.text}</span>;
          const id=part.id,n=doc.roots.findIndex(r=>r.id===id),s=states[id]||{status:'idle'};
          return <span key={id} ref={element=>{if(element)sourceRefs.current.set(id,element);else sourceRefs.current.delete(id);}} role="button" tabIndex={0} aria-pressed={id===selected} aria-controls="sentence-grammar-drawer" aria-label={`句 ${n+1}，${statusText[s.status]}：${part.text}`} className={`paragraph-sentence ${id===selected?'selected':''} ${id===hovered?'hovered':''}`} data-sentence={n+1} {...interactions(id)}><sup>{n+1}</sup><span className={`sentence-status ${s.status}`} aria-hidden="true"/>{part.text}</span>;
        })}</div>}</div>
        <div className="paragraph-translations"><h2>译文<span>（与原句对应高亮）</span></h2><div className="paragraph-text translation-text">{doc.roots.map((root,n)=>{
          const s=states[root.id]||{status:'idle'},layer=layers[root.id];
          return <span key={root.id} ref={element=>{if(element)translationRefs.current.set(root.id,element);else translationRefs.current.delete(root.id);}} role="button" tabIndex={0} aria-controls="sentence-grammar-drawer" aria-pressed={root.id===selected} aria-label={`句 ${n+1}译文，${statusText[s.status]}`} className={`paragraph-sentence ${root.id===selected?'selected':''} ${root.id===hovered?'hovered':''}`} {...interactions(root.id)}><sup>{n+1}</sup>{layer?.translation|| (s.status==='error'?<span className="translation-error">译文加载失败，点击查看</span>:<span className="translation-skeleton" style={{width:`${Math.min(38,Math.max(12,(root.last-root.first)*1.2))}ch`}}>译文加载中</span>)}{' '}</span>;
        })}</div></div>
        <p className="paragraph-instruction"><Icon name="info" size={13}/>点击任一句查看语法<span>Tab 定位 · Enter 打开</span></p>
      </section>
      {selected&&<div role="separator" aria-label="调整原文与解析区域高度" aria-orientation="horizontal" aria-valuenow={Math.round(share)} aria-valuemin={Math.ceil(minimumShare)} aria-valuemax={65} tabIndex={0} className="paragraph-divider" title="拖动调整上下区域高度；方向键微调" onPointerDown={e=>{e.currentTarget.setPointerCapture(e.pointerId);resize(e);}} onPointerMove={resize} onPointerUp={e=>{if(e.currentTarget.hasPointerCapture(e.pointerId))e.currentTarget.releasePointerCapture(e.pointerId);}} onKeyDown={e=>{if(e.key==='ArrowUp'||e.key==='ArrowDown'){e.preventDefault();e.stopPropagation();setShare(v=>Math.min(65,Math.max(minimumShare,v+(e.key==='ArrowUp'?-5:5))));}}}><span/></div>}
      <section id="sentence-grammar-drawer" ref={drawer} tabIndex={-1} hidden={!selected} className="card paragraph-drawer" aria-label={selected?`句 ${index+1} 的语法解析`:'语法解析'} onKeyDownCapture={drawerKey}>
        <header className="paragraph-drawer-header"><h2>语法解析</h2><span className="sentence-counter">句 {index+1} / {doc.roots.length}</span><span className="drawer-skeleton" title={selectedLayer?.skeleton||''}>主干：{selectedLayer?.skeleton||(state?.status==='error'?'本句解析失败':'正在获取…')}</span><div className="drawer-actions"><button className="button secondary" disabled={index<=0} onClick={()=>changeSentence(-1)}>← 上一句</button><button className="button secondary" disabled={index<0||index>=doc.roots.length-1} onClick={()=>changeSentence(1)}>下一句 →</button><button className="button secondary drawer-close" aria-label="关闭语法解析" onClick={close}>×</button></div></header>
        <div ref={drawerBody} className="paragraph-drawer-body">
          {selected&&state?.status==='error'?<div className="drawer-state notice error" role="alert"><p>{state.error}</p><button className="button secondary" onClick={()=>{queue.current?.enqueue(selected,true,true);drawer.current?.focus({preventScroll:true});}}>重试本句</button></div>:selected&&!selectedLayer?<div className="drawer-state" role="status"><span className="spinner"/><p>{state?.status==='loading'?'正在分析本句…':'本句已优先排队，正在等待可用请求…'}</p></div>:null}
          <div hidden={!selected||!selectedLayer}><LayeredAnalysisView document={fullDocument} activeRootId={selected||doc.roots[0].id} inDrawer onMetrics={onMetrics} onLayerLoaded={commitLayer}/>{metrics&&<PerformanceDetails metrics={metrics}/>}</div>
        </div>
      </section>
      {!selected&&<div className="paragraph-idle"><Icon name="document" size={27}/><p>点击上方任一句，在这里查看语法解析</p><span>原文与译文始终保留，切换句子时只更新一个抽屉。</span></div>}
    </div>
    {tooltip&&<div role="tooltip" className="sentence-tooltip" style={{left:tooltip.left,top:tooltip.top}}><span>主干</span>{layers[tooltip.id]?.skeleton||(states[tooltip.id]?.status==='error'?'本句解析失败，点击重试':'正在获取本句主干…')}<small>点击查看语法</small></div>}
  </div>;
}
