import { useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import Icon from './Icon';

export const colorRoles = [
  ['main_clause','主干分句','#e6ecf6'], ['subject','主语','#dceafa'],
  ['predicate','谓语','#f8dedd'], ['object','宾语','#e1eddc'],
  ['attribute','定语','#ece0f6'], ['adverbial','状语','#faead6'],
  ['complement','补语','#dcefeb'], ['predicative','表语','#dcefeb'],
  ['conjunction','连接词','#e1e5f6'], ['coordinate','并列成分','#e9edf7'],
  ['head','中心词','#e5e8ef'], ['determiner','限定词','#f1eafa'],
  ['marker','结构标记','#e9eef0'], ['other','其他成分','#f1eee8'],
] as const;
export type Palette = Record<string, { foreground: string; background: string }>;
export const defaultPalette: Palette = Object.fromEntries(colorRoles.map(([role,,background])=>[role,{foreground:'#242424',background}]));
export const paletteKey = 'grammar-assistant.component-colors.v1';
export function loadPalette(): Palette {
  try {
    const saved=JSON.parse(localStorage.getItem(paletteKey)||'{}');
    return Object.fromEntries(colorRoles.map(([role])=>[role,{
      foreground:/^#[0-9a-f]{6}$/i.test(saved?.[role]?.foreground)?saved[role].foreground:defaultPalette[role].foreground,
      background:/^#[0-9a-f]{6}$/i.test(saved?.[role]?.background)?saved[role].background:defaultPalette[role].background,
    }]));
  } catch { return defaultPalette; }
}
export function paletteStyle(palette:Palette): CSSProperties {
  return Object.fromEntries(colorRoles.flatMap(([role])=>[
    [`--component-${role}-fg`,palette[role].foreground],
    [`--component-${role}-bg`,palette[role].background],
  ])) as CSSProperties;
}
const groups = [
  {title:'核心成分',roles:['subject','predicate','object','predicative','complement']},
  {title:'修饰成分',roles:['attribute','adverbial']},
  {title:'结构成分',roles:['main_clause','conjunction','coordinate','head','determiner','marker','other']},
];
const roleName=(role:string)=>colorRoles.find(([id])=>id===role)![1];
const presets:Record<string,Palette>={
  default:defaultPalette,
  soft:Object.fromEntries(colorRoles.map(([role,,background])=>[role,{foreground:'#364152',background:role==='subject'?'#e4edf7':role==='predicate'?'#f5e5e5':background}])),
  vivid:Object.fromEntries(colorRoles.map(([role],i)=>[role,{foreground:'#182230',background:['#a8d4ff','#ffb3b3','#bce5a8','#dbbafa','#ffda9e','#a3ddd2','#b4c6ef'][i%7]}])),
};
type SavedScheme = { id:string; name:string; palette:Palette };
const schemesKey='grammar-assistant.color-schemes.v1';
function loadSchemes():SavedScheme[] {
  try {
    const value:unknown=JSON.parse(localStorage.getItem(schemesKey)||'[]');
    if(!Array.isArray(value))return [];
    const ids=new Set<string>();
    return value.filter((s):s is SavedScheme=>{
      if(!s||typeof s.id!=='string'||!s.id.startsWith('user:')||ids.has(s.id)||typeof s.name!=='string'||!s.name.trim()||s.name.length>30)return false;
      if(!colorRoles.every(([role])=>['foreground','background'].every(field=>typeof s.palette?.[role]?.[field]==='string'&&/^#[0-9a-f]{6}$/i.test(s.palette[role][field]))))return false;
      ids.add(s.id);return true;
    });
  } catch { return []; }
}
export default function ColorSettings({palette,onChange,error}:{palette:Palette;onChange:(p:Palette)=>void;error:string}) {
  const [schemes,setSchemes]=useState(loadSchemes);
  const [naming,setNaming]=useState(false);
  const [name,setName]=useState('');
  const [schemeError,setSchemeError]=useState('');
  const [schemeNotice,setSchemeNotice]=useState('');
  const [preferred,setPreferred]=useState('');
  const [selected,setSelected]=useState('object');
  const [structureOpen,setStructureOpen]=useState(false);
  const rows=useRef<Record<string,HTMLDivElement|null>>({});
  const allSchemes=[...schemes,...Object.entries(presets).map(([id,palette])=>({id,name:id,palette}))];
  const matches=allSchemes.filter(s=>colorRoles.every(([role])=>s.palette[role].foreground.toLowerCase()===palette[role].foreground.toLowerCase()&&s.palette[role].background.toLowerCase()===palette[role].background.toLowerCase()));
  const preset=matches.find(s=>s.id===preferred)?.id||matches[0]?.id||'custom';
  const activeScheme=schemes.find(s=>s.id===preset);
  function persistSchemes(next:SavedScheme[]):boolean {
    try {localStorage.setItem(schemesKey,JSON.stringify(next));setSchemes(next);setSchemeError('');return true;}
    catch {setSchemeError('方案保存失败，请重试。');return false;}
  }
  function saveScheme() {
    const trimmed=name.trim();
    if(!trimmed||trimmed.length>30){setSchemeError('请输入 1–30 个字符的方案名称。');return;}
    if(['默认','柔和','鲜明',...schemes.map(s=>s.name)].some(n=>n.toLocaleLowerCase()===trimmed.toLocaleLowerCase())){setSchemeError('此名称已存在，请使用其他名称。');return;}
    const scheme={id:`user:${crypto.randomUUID()}`,name:trimmed,palette:structuredClone(palette)};
    if(persistSchemes([...schemes,scheme])){setPreferred(scheme.id);setNaming(false);setName('');setSchemeNotice(`已保存方案“${trimmed}”。`);}
  }
  function deleteScheme() {
    if(activeScheme&&persistSchemes(schemes.filter(s=>s.id!==activeScheme.id))){setPreferred('');setSchemeNotice(`已删除方案“${activeScheme.name}”，当前配色保留。`);}
  }
  function select(role:string) {
    setSelected(role);
    if(groups[2].roles.includes(role))setStructureOpen(true);
    requestAnimationFrame(()=>rows.current[role]?.scrollIntoView({block:'nearest',behavior:'smooth'}));
  }
  function keySelect(e:KeyboardEvent<HTMLSpanElement>,role:string) {if(e.key==='Enter'||e.key===' '){e.preventDefault();e.stopPropagation();select(role);}}
  function sample(role:string,text:string) {return <span className={`piece ${role} preview-part`} role="button" tabIndex={0} aria-label={`选择${roleName(role)}`} aria-pressed={selected===role} onClick={()=>select(role)} onKeyDown={e=>keySelect(e,role)}>{text}</span>;}
  function renderRows(roles:string[]) {return <><div className="color-table-head" aria-hidden="true"><span>成分</span><span>背景色</span><span>文字色</span><span/></div>{roles.map(role=><div className={`color-row ${selected===role?'selected':''}`} key={role} ref={el=>{rows.current[role]=el;}} onFocus={()=>setSelected(role)}>
    <button className={`piece color-name ${role}`} onClick={()=>select(role)} aria-pressed={selected===role}>{roleName(role)}</button>
    {(['background','foreground'] as const).map(field=><label className="color-cell" key={field}><input type="color" aria-label={`${roleName(role)}${field==='background'?'背景色':'文字色'}`} value={palette[role][field]} onChange={e=>onChange({...palette,[role]:{...palette[role],[field]:e.target.value}})}/><code>{palette[role][field].toUpperCase()}</code></label>)}
    <button className="button subtle color-reset" title={`重置${roleName(role)}`} aria-label={`重置${roleName(role)}`} onClick={()=>onChange({...palette,[role]:defaultPalette[role]})}><Icon name="reset" size={16}/></button>
  </div>)}</>;}
  return <div className="color-settings">
    <header className="page-header color-page-header"><div><h1>成分配色</h1><p>修改后立即应用并自动保存。点击预览中的成分，可直接定位到对应设置。</p></div><div className="color-actions"><label className="sr-only" htmlFor="color-preset">预设方案</label><select id="color-preset" value={preset} onChange={e=>{const scheme=allSchemes.find(s=>s.id===e.target.value);if(scheme){setPreferred(scheme.id);onChange(structuredClone(scheme.palette));setSchemeNotice("");}}}><option value="custom" disabled>当前配色：未保存为方案</option><option value="default">预设方案：默认</option><option value="soft">预设方案：柔和</option><option value="vivid">预设方案：鲜明</option>{schemes.length>0&&<optgroup label="我的方案">{schemes.map(s=><option key={s.id} value={s.id}>{s.name}</option>)}</optgroup>}</select><button className="button secondary" onClick={()=>{setNaming(v=>!v);setSchemeError("");}}>另存为方案</button>{activeScheme&&<button className="button subtle" onClick={deleteScheme} aria-label={`删除方案${activeScheme.name}`}><Icon name="trash" size={16}/>删除方案</button>}<button className="button secondary" onClick={()=>onChange(defaultPalette)}>恢复默认</button></div></header>
    {naming&&<form className="scheme-save-form" onSubmit={e=>{e.preventDefault();saveScheme();}}><label htmlFor="scheme-name">方案名称</label><input id="scheme-name" autoFocus type="text" maxLength={30} value={name} placeholder="例如：我的阅读配色" onChange={e=>setName(e.target.value)}/><button className="button primary" type="submit">保存方案</button><button className="button secondary" type="button" onClick={()=>{setNaming(false);setSchemeError('');}}>取消</button></form>}
    {schemeError&&<p className="scheme-message error-text" role="alert">{schemeError}</p>}
    {schemeNotice&&<p className="scheme-message muted" role="status">{schemeNotice}</p>}
    <section className="card color-preview"><div className="section-head"><h2>实时预览</h2><span className="status-pill" aria-live="polite">已选中：{roleName(selected)}</span></div><p className="sentence">{sample('subject','The interior ministry')}{' '}{sample('predicate','has said')}{' '}{sample('object','it will deploy 70,000 police officers')},{' '}<span className="piece adverbial preview-part" role="button" tabIndex={0} aria-label="选择状语" aria-pressed={selected==='adverbial'} onClick={()=>select('adverbial')} onKeyDown={e=>keySelect(e,'adverbial')}>ramping up the security presence in <span className="piece attribute preview-part" role="button" tabIndex={0} aria-label="选择定语 French" aria-pressed={selected==='attribute'} onClick={e=>{e.stopPropagation();select('attribute');}} onKeyDown={e=>keySelect(e,'attribute')}>French</span> cities</span>.</p></section>
    {error&&<div className="notice error" role="alert">{error}</div>}
    <div className="color-groups">{groups.map((group,i)=>i<2?<section className="card color-group" key={group.title}><h2>{group.title}</h2>{renderRows(group.roles)}</section>:<details className="card color-group structural-colors" key={group.title} open={structureOpen} onToggle={e=>setStructureOpen(e.currentTarget.open)}><summary><strong>{group.title}</strong><span>{group.roles.map(roleName).join('、')}</span><Icon name="chevron" size={13}/></summary>{structureOpen&&renderRows(group.roles)}</details>)}</div>
  </div>;
}
