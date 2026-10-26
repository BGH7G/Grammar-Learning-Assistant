import type {AnalysisLayer,GrammarDetail,LayerDocument,LayerNode} from './types/analysis';

const sentences=[
  'The city launched a bike-sharing plan last spring.',
  'Many residents who work downtown now ride to the office instead of driving, hoping to save money.',
  'Critics argue that the stations are too far from the suburbs.',
  'If the council adds more stations, usage could double by next year.',
  'What worries the planners is that the bikes are often stolen.',
  'Surveys suggest the scheme has made the streets safer.',
  'Still, it remains to be seen whether the project will pay for itself.',
];
const translations=[
  '这座城市去年春天推出了一项共享单车计划。',
  '许多在市中心上班的居民现在骑车上班，而不开车，希望省钱。',
  '批评者认为，这些站点离郊区太远。',
  '如果市议会增设更多站点，使用量到明年可能会翻倍。',
  '让规划者担忧的是，这些自行车经常被盗。',
  '调查显示，该计划已经让街道更安全。',
  '不过，这个项目能否自负盈亏，仍有待观察。',
];
const skeletons=['The city launched a plan.','Residents ride to the office.','Critics argue the stations are far.','Usage could double.','What worries the planners is that bikes are stolen.','Surveys suggest the scheme has made streets safer.','It remains to be seen whether the project will pay for itself.'];
const split=(text:string)=>text.match(/[\p{L}\p{N}'’]+|\s+|[^\s]/gu)||[];
const pieces=split(sentences.join(' ')).map((text,id)=>({text,id}));
const tokens:LayerDocument['tokens']=pieces.filter(p=>!/^\s+$/u.test(p.text)).map((p,id)=>({id,text:p.text,pieceId:p.id}));
let cursor=0;
const roots:LayerNode[]=sentences.map((text,i)=>{
  const first=cursor;cursor+=split(text).filter(t=>!/^\s+$/u.test(t)).length;
  return {id:`offline:s${i}`,parentId:null,sentenceId:`offline:s${i}`,kind:'sentence',role:'root',ctype:'none',grammar:[],first,last:cursor-1,depth:0,expandable:true};
});
const layers:AnalysisLayer[]=[];const counts=new Map<string,number>();
function child(owner:LayerNode,role:LayerNode['role'],text:string,grammar:GrammarDetail[]=[],ctype:LayerNode['ctype']='none',kind?:LayerNode['kind']):LayerNode{
  const words=split(text).filter(t=>!/^\s+$/u.test(t));
  const offset=tokens.slice(owner.first,owner.last+1).findIndex((_,i,all)=>words.every((w,j)=>all[i+j]?.text===w));
  if(offset<0)throw new Error(`Missing offline fragment: ${text}`);
  const first=owner.first+offset,last=first+words.length-1;
  const index=counts.get(owner.id)||0;counts.set(owner.id,index+1);
  return {id:`${owner.id}:${index}`,parentId:owner.id,sentenceId:owner.sentenceId,role,ctype,grammar,first,last,kind:kind||(words.filter(w=>/[\p{L}\p{N}]/u.test(w)).length===1?'word':'phrase'),depth:owner.depth+1,expandable:first<last&&owner.depth+1<4};
}
function layer(owner:LayerNode,nodes:LayerNode[],translation:string,skeleton:string|null=null,relations:AnalysisLayer['relations']=[],notes:string[]=[]){layers.push({parentId:owner.id,depth:owner.depth+1,nodes,translation,skeleton,relations,notes});}

const r1=roots[0];
layer(r1,[child(r1,'subject','The city',['noun_phrase']),child(r1,'predicate','launched',['lexical_verb']),child(r1,'object','a bike-sharing plan',['noun_phrase']),child(r1,'adverbial','last spring.',['noun_phrase'])],translations[0],skeletons[0],[],['离线固定示例，用于体验交互，不调用模型。']);

const r2=roots[1],subject=child(r2,'subject','Many residents who work downtown',['noun_phrase']);
const toOffice=child(r2,'adverbial','to the office',['prepositional_phrase']);
layer(r2,[subject,child(r2,'adverbial','now',['adverb']),child(r2,'predicate','ride',['lexical_verb']),toOffice,child(r2,'adverbial','instead of driving,',['prepositional_phrase']),child(r2,'adverbial','hoping to save money.',['present_participial'],'participial','clause')].sort((a,b)=>a.first-b.first),translations[1],skeletons[1]);
const resident=child(subject,'head','residents',['noun']);
const relative=child(subject,'attribute','who work downtown',['postmodifier'],'relative','clause');
layer(subject,[child(subject,'determiner','Many'),resident,relative].sort((a,b)=>a.first-b.first),'许多在市中心上班的居民',null,[{sourceId:relative.id,headToken:resident.first}],['中心词 residents 后接定语从句。']);
layer(relative,[child(relative,'subject','who',['relative_pronoun']),child(relative,'predicate','work',['lexical_verb']),child(relative,'adverbial','downtown',['adverb'])],'在市中心上班的',null,[],['who 在从句中作主语。']);
const office=child(toOffice,'object','the office',['noun_phrase']);
layer(toOffice,[child(toOffice,'head','to',['preposition']),office].sort((a,b)=>a.first-b.first),'到办公室');
layer(office,[child(office,'determiner','the'),child(office,'head','office',['noun'])],'办公室');

const r3=roots[2],reported=child(r3,'object','that the stations are too far from the suburbs.',[],'noun','clause');
layer(r3,[child(r3,'subject','Critics',['noun']),child(r3,'predicate','argue',['lexical_verb']),reported].sort((a,b)=>a.first-b.first),translations[2],skeletons[2]);
layer(reported,[child(reported,'marker','that',['subordinator']),child(reported,'subject','the stations',['noun_phrase']),child(reported,'predicate','are',['linking_verb']),child(reported,'predicative','too far from the suburbs.',['adjective_phrase'])],'这些站点离郊区太远');

const r4=roots[3],condition=child(r4,'adverbial','If the council adds more stations,',[],'adverbial','clause'),verbs=child(r4,'predicate','could double',['verb_phrase']);
layer(r4,[condition,child(r4,'subject','usage',['noun']),verbs,child(r4,'adverbial','by next year.',['prepositional_phrase'])].sort((a,b)=>a.first-b.first),translations[3],skeletons[3]);
layer(condition,[child(condition,'marker','If',['subordinator']),child(condition,'subject','the council',['noun_phrase']),child(condition,'predicate','adds',['lexical_verb']),child(condition,'object','more stations,',['noun_phrase'])],'如果市议会增设更多站点');
layer(verbs,[child(verbs,'predicate','could',['modal_verb']),child(verbs,'predicate','double',['lexical_verb'])],'（使用量）可能翻倍',null,[],['情态动词与实义动词共同组成谓语。']);

const r5=roots[4],what=child(r5,'subject','What worries the planners',[],'noun','clause'),that=child(r5,'predicative','that the bikes are often stolen.',[],'noun','clause');
layer(r5,[what,child(r5,'predicate','is',['linking_verb']),that].sort((a,b)=>a.first-b.first),translations[4],skeletons[4]);
layer(what,[child(what,'subject','What',['relative_pronoun']),child(what,'predicate','worries',['lexical_verb']),child(what,'object','the planners',['noun_phrase'])],'让规划者担忧的事情');
layer(that,[child(that,'marker','that',['subordinator']),child(that,'subject','the bikes',['noun_phrase']),child(that,'predicate','are often stolen.',['verb_phrase'])],'这些自行车经常被盗');

const r6=roots[5],content=child(r6,'object','the scheme has made the streets safer.',[],'noun','clause');
layer(r6,[child(r6,'subject','Surveys',['noun']),child(r6,'predicate','suggest',['lexical_verb']),content].sort((a,b)=>a.first-b.first),translations[5],skeletons[5]);
layer(content,[child(content,'subject','the scheme',['noun_phrase']),child(content,'predicate','has made',['verb_phrase']),child(content,'object','the streets',['noun_phrase']),child(content,'complement','safer.',['adjective'])],'该计划已经让街道更安全');

const r7=roots[6];
layer(r7,[child(r7,'marker','Still,',['discourse_marker']),child(r7,'other','it'),child(r7,'predicate','remains',['linking_verb']),child(r7,'predicative','to be seen',[],'infinitive','clause'),child(r7,'subject','whether the project will pay for itself.',[],'noun','clause')],translations[6],skeletons[6]);

export const paragraphSample:LayerDocument={schemaVersion:'2',promptVersion:'offline-ui-v2',sessionId:'offline',pieces,tokens,roots,layers,maxDepth:4};
