import type { AnalysisLayer, LayerNode, LayerRole } from './types/analysis';

const roles: Record<LayerRole,string> = {root:'整句',main_clause:'主句',subject:'主语',predicate:'谓语',object:'宾语',attribute:'定语',adverbial:'状语',complement:'补语',predicative:'表语',conjunction:'连接词',coordinate:'并列成分',head:'中心词',determiner:'限定词',marker:'结构标记',punctuation:'标点',other:'其他成分'};
export function componentRole(node:LayerNode, owner?:LayerNode, siblings:LayerNode[]=[]):string {
  // Pure punctuation is filtered by source text. A retained punctuation-labelled
  // span contains meaningful content and must stay visible for review.
  if(node.role==='punctuation')return '待核对';
  if(node.role==='coordinate') {
    const items=siblings.filter(n=>n.role==='coordinate');
    const index=items.findIndex(n=>n.id===node.id);
    if(owner?.role==='predicate'&&node.kind!=='clause')return `谓语部分${index>=0?' '+(index+1):''}`;
    if(node.kind==='clause')return `分句${index>=0?' '+(index+1):''}`;
  }
  return roles[node.role];
}
export function isPunctuationOnly(text:string):boolean {
  return /^[\p{P}\s]+$/u.test(text);
}
export function coordinatedPredicate(node:LayerNode, layer?:AnalysisLayer):boolean {
  if(node.role!=='predicate')return false;
  const items=layer?.nodes.filter(n=>n.role==='coordinate')||[];
  return items.length>=2&&items.every(n=>n.kind!=='clause');
}
export function componentType(node:LayerNode, layer?:AnalysisLayer):string {
  if(node.role==='main_clause')return '';
  if(coordinatedPredicate(node,layer))return '两个或多个并列动作';
  if(node.ctype==='coordinate')return node.kind==='clause'&&node.role!=='predicate'?'并列分句':'并列结构';
  const types={main:'主句',relative:'关系从句',noun:'名词性从句',adverbial:'状语从句',infinitive:'不定式结构',participial:'分词结构',gerund:'动名词结构',none:''};
  const text=types[node.ctype];
  return node.kind==='phrase'&&['relative','noun','adverbial'].includes(node.ctype)?'含'+text:text;
}
// Only use an explicit subject belonging to the predicate's own containing clause.
// Never cross a clause boundary and accidentally borrow an outer subject.
export function predicateSubject(node:LayerNode,nodes:Map<string,LayerNode>,layers:Record<string,AnalysisLayer>):LayerNode|undefined {
  let cursor:LayerNode|undefined=node;
  while(cursor) {
    if(cursor.role==='predicate'&&cursor.parentId) {
      const subjects=layers[cursor.parentId]?.nodes.filter(n=>n.role==='subject')||[];
      return subjects.length===1?subjects[0]:undefined;
    }
    if(cursor.kind==='clause'||cursor.kind==='sentence')return undefined;
    cursor=cursor.parentId?nodes.get(cursor.parentId):undefined;
  }
  return undefined;
}
