export type SentenceStatus = 'idle'|'queued'|'loading'|'ready'|'error';
export interface SentenceState<T> { status:SentenceStatus; value?:T; error?:string }

export class HoverIntent {
  private pending:(()=>void)|null=null;
  constructor(private wait:(run:()=>void,delay:number)=>()=>void=(run,delay)=>{const id=setTimeout(run,delay);return()=>clearTimeout(id);}){}
  run(callback:()=>void){this.cancel();this.pending=this.wait(()=>{this.pending=null;callback();},300);}
  cancel(){this.pending?.();this.pending=null;}
}

/** One queue per analysis session. Click/settled hover only reprioritizes work. */
export class SentenceRequestQueue<T> {
  private states=new Map<string,SentenceState<T>>();
  private pending:string[]=[];
  private active=0;
  private stopped=false;
  constructor(ids:string[],private load:(id:string)=>Promise<T>,private changed:(id:string,state:SentenceState<T>)=>void,cache=new Map<string,T>(),private limit=2) {
    if(limit<1)throw new Error('Queue concurrency must be positive');
    for(const id of ids)this.states.set(id,cache.has(id)?{status:'ready',value:cache.get(id)}:{status:'idle'});
  }
  get(id:string){return this.states.get(id);}
  enqueue(id:string,priority=false,retry=false) {
    const state=this.states.get(id);
    if(this.stopped||!state||state.status==='ready'||state.status==='loading'||state.status==='error'&&!retry)return;
    this.pending=this.pending.filter(item=>item!==id);
    priority?this.pending.unshift(id):this.pending.push(id);
    this.update(id,{status:'queued'});
    this.pump();
  }
  enqueueAll(){for(const id of this.states.keys())this.enqueue(id);}
  dispose(){this.stopped=true;this.pending=[];}
  private update(id:string,state:SentenceState<T>){this.states.set(id,state);if(!this.stopped)this.changed(id,state);}
  private pump(){
    while(!this.stopped&&this.active<this.limit&&this.pending.length){
      const id=this.pending.shift()!;this.active++;this.update(id,{status:'loading'});
      Promise.resolve().then(()=>{if(this.stopped)throw new Error('Queue disposed');return this.load(id);}).then(value=>{
        if(!this.stopped)this.update(id,{status:'ready',value});
      },error=>{
        if(!this.stopped)this.update(id,{status:'error',error:typeof error==='string'?error:error instanceof Error?error.message:'本句分析失败，请重试。'});
      }).finally(()=>{this.active--;this.pump();});
    }
  }
}
