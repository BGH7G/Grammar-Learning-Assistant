import type { Analysis, AnalysisNode, Role } from '../types/analysis';
export const labels:Record<Role,string>={root:'主句',subject:'主语',predicate:'谓语',object:'宾语',attribute:'定语',adverbial:'状语',complement:'补语',predicative:'表语'};
export default function AnalysisView({analysis:a}:{analysis:Analysis}) {
  const root=a.nodes.find(n=>n.parent_id===null)!;
  function line(parent:AnalysisNode) {
    const children=a.nodes.filter(n=>n.parent_id===parent.id);
    return <div className="sentence">{a.pieces.slice(parent.first,parent.last+1).map(p=>{
      const node=children.find(n=>n.first<=p.id&&p.id<=n.last);
      return <span key={p.id} className={node ? `piece ${node.role}`:''} title={node?labels[node.role]:undefined}>{p.text}</span>;
    })}</div>;
  }
  return <section className="card result"><div className="section-head"><h2>句子结构</h2><span className="badge">原文已对齐</span></div>
    {line(root)}<div className="legend">{Object.entries(labels).filter(([k])=>k!=='root').map(([k,v])=><span key={k} className={`piece ${k}`}>{v}</span>)}</div>
    <h3>中文翻译</h3><p>{a.translation}</p>
    {a.nodes.filter(n=>n.kind==='clause'&&n.parent_id!==null).map(n=><details key={n.id}><summary>从句 · {a.pieces.slice(n.first,n.last+1).map(p=>p.text).join('')}</summary>{line(n)}</details>)}
    {a.notes.length>0&&<><h3>分析说明</h3><ul>{a.notes.map((n,i)=><li key={i}>{n}</li>)}</ul></>}
  </section>;
}
