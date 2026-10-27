import {useEffect, useState, type KeyboardEvent} from 'react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import Icon from './Icon';
const defaultShortcut='Ctrl+Alt+E';
type Settings={selectionTranslate:string;registered:boolean;registrationError:string|null};
export default function HotkeySettings(){
  const [saved,setSaved]=useState(defaultShortcut);
  const [draft,setDraft]=useState(defaultShortcut);
  const [ready,setReady]=useState(false);
  const [saving,setSaving]=useState(false);
  const [recording,setRecording]=useState(false);
  const [error,setError]=useState('');
  const [notice,setNotice]=useState('');
  const [status,setStatus]=useState<Settings|null>(null);
  useEffect(()=>{
    if(!isTauri()){setReady(true);return;}
    let active=true;
    void invoke<Settings>('get_hotkey_settings').then(value=>{
      if(active){setSaved(value.selectionTranslate);setDraft(value.selectionTranslate);setStatus(value);if(value.registrationError)setError(value.registrationError);}
    }).catch(e=>{if(active)setError(typeof e==='string'?e:'读取热键设置失败，请重试。');})
      .finally(()=>{if(active)setReady(true);});
    return()=>{active=false;};
  },[]);
  function record(event:KeyboardEvent<HTMLInputElement>){
    if(event.key==='Tab'&&!event.ctrlKey&&!event.altKey&&!event.metaKey)return;
    event.preventDefault();event.stopPropagation();
    if(event.repeat||event.nativeEvent.isComposing)return;
    if(event.key==='Escape'){setDraft(saved);setRecording(false);setError('');setNotice('');return;}
    const virtualKey=event.nativeEvent.keyCode;
    if(['Control','Alt','Shift','Meta','AltGraph'].includes(event.key)||[16,17,18,91,92].includes(virtualKey))return;
    if(!event.ctrlKey&&!event.altKey&&!event.metaKey){setError('请使用包含 Ctrl、Alt 或 Win 的组合键。');return;}
    let key='';
    if(/^Key[A-Z]$/.test(event.code))key=event.code.slice(3);
    else if(/^Digit[0-9]$/.test(event.code))key=event.code.slice(5);
    else if(/^F([1-9]|1[0-9]|2[0-4])$/.test(event.code))key=event.code;
    else if(event.code==='Space')key='Space';
    // Some virtual keyboards omit KeyboardEvent.code; keep their key input usable.
    if(!key&&/^[a-z0-9]$/i.test(event.key))key=event.key.toUpperCase();
    if(!key&&/^F([1-9]|1[0-9]|2[0-4])$/.test(event.key))key=event.key;
    if(!key&&((virtualKey>=65&&virtualKey<=90)||(virtualKey>=48&&virtualKey<=57)))key=String.fromCharCode(virtualKey);
    if(!key&&virtualKey>=112&&virtualKey<=135)key=`F${virtualKey-111}`;
    if(!key&&(event.key===' '||virtualKey===32))key='Space';
    if(!key){setError('请搭配字母、数字、空格或 F1–F24 功能键。');return;}
    setDraft([event.ctrlKey?'Ctrl':'',event.altKey?'Alt':'',event.shiftKey?'Shift':'',event.metaKey?'Win':'',key].filter(Boolean).join('+'));
    setRecording(false);setError('');setNotice('');
  }
  async function save(){
    if(!isTauri()){setError('请在桌面应用中保存快捷键。');return;}
    setSaving(true);setError('');setNotice('');
    try{const value=await invoke<Settings>('save_hotkey_settings',{shortcut:draft});setSaved(value.selectionTranslate);setDraft(value.selectionTranslate);setStatus(value);setNotice('快捷键已保存并生效。');}
    catch(e){setError(typeof e==='string'?e:'保存失败，请重试。');}
    finally{setSaving(false);}
  }
  return <div className="hotkey-settings">
    <section className="card hotkey-card" aria-label="热键设置">
      <div className="hotkey-row">
        <div className="hotkey-action"><span className="hotkey-icon"><Icon name="keyboard" size={22}/></span><div><h2>划词翻译</h2><p>设置选中文字后使用的快捷键。</p></div></div>
        <div className="hotkey-recorder"><label className="sr-only" htmlFor="selection-translate-hotkey">划词翻译快捷键</label><input id="selection-translate-hotkey" type="text" readOnly value={draft} disabled={!ready||saving} onFocus={()=>{setRecording(true);setNotice('');}} onBlur={()=>setRecording(false)} onKeyDown={record} aria-describedby="hotkey-input-hint" aria-invalid={!!error} spellCheck={false}/></div>
      </div>
      <div className="hotkey-footer"><p id="hotkey-input-hint" className="hint">{recording?'请按下新的组合键，Esc 取消。':status?.registered?'快捷键已启用。点击输入框可修改。':'点击快捷键输入框后，按下要使用的组合键。'}</p><div className="button-row"><button className="button secondary" disabled={!ready||saving||draft===defaultShortcut} onClick={()=>{setDraft(defaultShortcut);setError('');setNotice('');}}>恢复默认</button><button className="button primary" disabled={!ready||saving||(draft===saved&&!!status?.registered)||!isTauri()} onClick={()=>void save()}><Icon name="save" size={15}/>{saving?'保存中…':'保存'}</button></div></div>
    </section>
    {error&&<div className="notice error" role="alert"><Icon name="info" size={17}/><span>{error}</span></div>}
    {notice&&<div className="notice" role="status"><Icon name="check" size={17}/><span>{notice}</span></div>}
    <p className="workspace-note"><Icon name="info" size={14}/>在其他软件中选中文字后按下热键，文本将填入分析页，确认后再分析。取词会更新系统剪贴板，不恢复之前的内容。部分编辑器未选中文字时会复制整行，请核对原文。</p>
  </div>;
}
