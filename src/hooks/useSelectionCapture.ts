import {useEffect,useRef} from 'react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
export type CapturedText={requestId:string;text:string;elapsedMs:number};
type Snapshot={revision:number;busy:boolean;latest:CapturedText|null;error:{code:string;message:string}|null;hotkeyError:string|null};
type Callbacks={onStart:()=>void;onText:(value:CapturedText)=>void;onError:(message:string)=>void};
export default function useSelectionCapture(callbacks:Callbacks){
  const latestCallbacks=useRef(callbacks);latestCallbacks.current=callbacks;
  useEffect(()=>{
    if(!isTauri())return;
    let disposed=false;let unlisten:(()=>void)|undefined;
    let revision=-1;let delivered='';let lastError='';let lastHotkeyError='';let wasBusy=false;
    const refresh=()=>{void invoke<Snapshot>('get_selection_state').then(state=>{
      if(disposed||state.revision<=revision)return;revision=state.revision;
      if(state.busy&&!wasBusy)latestCallbacks.current.onStart();wasBusy=state.busy;
      if(state.latest&&state.latest.requestId!==delivered){delivered=state.latest.requestId;latestCallbacks.current.onText(state.latest);}
      const error=state.error?`${state.revision}:${state.error.code}`:'';
      if(error&&error!==lastError)latestCallbacks.current.onError(state.error!.message);lastError=error;
      if(state.hotkeyError&&state.hotkeyError!==lastHotkeyError)latestCallbacks.current.onError(`${state.hotkeyError} 请在“热键”页面重新设置。`);
      lastHotkeyError=state.hotkeyError??'';
    }).catch(()=>{if(!disposed&&lastError!=='state-unavailable'){lastError='state-unavailable';latestCallbacks.current.onError('无法读取取词状态，请手动粘贴或重新打开应用。');}});};
    void listen('selection-state-changed',refresh).then(stop=>{
      if(disposed){stop();return;}unlisten=stop;refresh();
    }).catch(()=>{if(!disposed){latestCallbacks.current.onError('取词通知连接失败，请重新打开应用。');refresh();}});
    window.addEventListener('focus',refresh);
    return()=>{disposed=true;unlisten?.();window.removeEventListener('focus',refresh);};
  },[]);
}
