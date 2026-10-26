import fs from 'node:fs';
import assert from 'node:assert/strict';
import ts from 'typescript';
async function moduleAt(file){
  const text=fs.readFileSync(new URL(file,import.meta.url),'utf8');
  const js=ts.transpileModule(text,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
  return import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
}
const {SentenceRequestQueue,HoverIntent}=await moduleAt('../src/analysisQueue.ts');
const tick=()=>new Promise(resolve=>setImmediate(resolve));
const calls=[],deferred=new Map(),changes=[];
const load=id=>{calls.push(id);return new Promise((resolve,reject)=>deferred.set(id,{resolve,reject}));};
const q=new SentenceRequestQueue(['a','b','c','d','e'],load,(id,state)=>changes.push([id,state.status]),new Map([['a','cached']]));
q.enqueueAll();await tick();assert.deepEqual(calls,['b','c']);
q.enqueue('b',true);q.enqueue('b',true);q.enqueue('a',true);
q.enqueue('e',true);q.enqueue('e',true);await tick();assert.deepEqual(calls,['b','c']);
deferred.get('b').resolve('B');await tick();assert.deepEqual(calls,['b','c','e']);
deferred.get('c').reject('temporary failure');await tick();assert.equal(q.get('c').status,'error');assert.deepEqual(calls,['b','c','e','d']);
q.enqueue('c',true);await tick();assert.equal(calls.filter(id=>id==='c').length,1);
q.enqueue('c',true,true);q.enqueue('c',true,true);await tick();
deferred.get('e').resolve('E');await tick();assert.equal(calls.filter(id=>id==='c').length,2);
deferred.get('c').resolve('C');deferred.get('d').resolve('D');await tick();
assert.equal(q.get('c').status,'ready');q.enqueueAll();await tick();assert.equal(calls.length,5);
q.dispose();

let disposedCalls=0;
const disposed=new SentenceRequestQueue(['x','y','z'],async()=>{disposedCalls++;return 'value';},()=>{});
disposed.enqueueAll();disposed.dispose();await tick();assert.equal(disposedCalls,0,'effect cleanup must stop not-yet-started calls');
let release,lateChanges=0,lateCalls=0;
const late=new SentenceRequestQueue(['x','y'],()=>{lateCalls++;return new Promise(resolve=>release=resolve);},()=>lateChanges++,new Map(),1);
late.enqueueAll();await tick();const before=lateChanges;late.dispose();release('late');await tick();
assert.equal(lateChanges,before);assert.equal(lateCalls,1,'dispose must not start the queued sentence');

let now=0,hoverCalls=0;const jobs=[];
const intent=new HoverIntent((run,delay)=>{const job={run,at:now+delay,cancelled:false};jobs.push(job);return()=>job.cancelled=true;});
const advance=ms=>{now+=ms;for(const job of jobs){if(!job.cancelled&&job.at<=now){job.cancelled=true;job.run();}}};
intent.run(()=>hoverCalls++);advance(200);intent.run(()=>hoverCalls++);advance(200);intent.cancel();advance(500);assert.equal(hoverCalls,0);
intent.run(()=>hoverCalls++);advance(299);assert.equal(hoverCalls,0);advance(1);assert.equal(hoverCalls,1);

const {paragraphSample:doc}=await moduleAt('../src/paragraphSample.ts');
assert.equal(doc.roots.length,7);
const nodes=new Map([...doc.roots,...doc.layers.flatMap(l=>l.nodes)].map(n=>[n.id,n]));
assert.equal(nodes.size,doc.roots.length+doc.layers.reduce((sum,l)=>sum+l.nodes.length,0));
for(const layer of doc.layers){
  const owner=nodes.get(layer.parentId);assert.ok(owner);
  let next=owner.first;
  for(const node of layer.nodes){assert.equal(node.first,next,`${layer.parentId}: source gap/overlap`);assert.equal(node.parentId,owner.id);assert.ok(node.last<=owner.last);next=node.last+1;}
  assert.equal(next,owner.last+1);
  for(const r of layer.relations){const child=nodes.get(r.sourceId);assert.ok(child);assert.ok(r.headToken<child.first||r.headToken>child.last);}
}
for(const token of doc.tokens)assert.equal(token.text,doc.pieces[token.pieceId].text);
console.log('Paragraph queue: concurrency, cache, priority, retry, disposal and 300ms hover intent passed.');
console.log('Seven-sentence offline fixture: lossless mapping, node IDs, hierarchy and coverage passed.');
