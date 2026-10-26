import type { AnalysisLayer, GrammarDetail, LayerNode, LayerRole } from './types/analysis';

const roles: Record<LayerRole,string> = {root:'整句',main_clause:'主句',subject:'主语',predicate:'谓语',object:'宾语',attribute:'定语',adverbial:'状语',complement:'补语',predicative:'表语',conjunction:'连接词',coordinate:'并列成分',head:'中心词',determiner:'限定词',marker:'结构标记',punctuation:'标点',other:'其他成分'};
export function componentRole(node:LayerNode, owner?:LayerNode, siblings:LayerNode[]=[], text=''):string {
  // Pure punctuation is filtered by source text. A retained punctuation-labelled
  // span contains meaningful content and must stay visible for review.
  if(node.role==='punctuation')return '待核对';
  if(node.role==='marker'&&node.grammar?.includes('discourse_marker'))return '独立成分';
  if(node.role==='head'&&node.grammar?.includes('noun_phrase'))return '核心名词短语';
  if(node.role==='attribute') {
    if(node.grammar?.includes('postmodifier'))return '后置定语';
    if(node.grammar?.includes('premodifier'))return '前置定语';
  }
  if(node.role==='determiner') {
    const subtype=determinerType(text);
    return subtype?`限定词 · ${subtype}`:'限定词';
  }
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
  if(node.grammar?.includes('noun_phrase'))return node.role==='head'?'NP':'名词短语（NP）';
  if(node.ctype==='participial') {
    if(node.grammar?.includes('past_participial'))return '过去分词短语';
    if(node.grammar?.includes('present_participial'))return '现在分词短语';
  }
  if(node.role==='main_clause')return '';
  if(coordinatedPredicate(node,layer))return '两个或多个并列动作';
  if(node.ctype==='coordinate')return node.kind==='clause'&&node.role!=='predicate'?'并列分句':'并列结构';
  const types={main:'主句',relative:'关系从句',noun:'名词性从句',adverbial:'状语从句',infinitive:'不定式结构',participial:'分词结构',gerund:'动名词结构',none:''};
  const text=types[node.ctype];
  if(text)return node.kind==='phrase'&&['relative','noun','adverbial'].includes(node.ctype)?'含'+text:text;
  const detailLabels:Partial<Record<GrammarDetail,string>>={
    verb_phrase:'动词短语',adjective_phrase:'形容词短语',adverb_phrase:'副词短语',prepositional_phrase:'介词短语',
    modal_verb:'情态动词',auxiliary_verb:'助动词',linking_verb:'系动词',lexical_verb:'实义动词',
    noun:'名词',proper_noun:'专有名词',pronoun:'代词',relative_pronoun:'关系代词',adjective:'形容词',adverb:'副词',
    preposition:'介词',numeral:'数词',coordinating_conjunction:'并列连词',subordinator:'从属连词',
    discourse_marker:'话语标记',infinitive_marker:'不定式标记',
  };
  // Most specific lexical type wins; never combine conflicting word classes into
  // a long label. Clause/non-finite types above keep their existing priority.
  const priority:GrammarDetail[]=['verb_phrase','adjective_phrase','adverb_phrase','prepositional_phrase',
    'modal_verb','auxiliary_verb','linking_verb','lexical_verb','proper_noun','relative_pronoun',
    'noun','pronoun','adjective','adverb','preposition','numeral','coordinating_conjunction','subordinator','discourse_marker','infinitive_marker'];
  const detail=priority.find(tag=>node.grammar?.includes(tag));
  return detail?detailLabels[detail]||'':'';
}

// Called only for model-classified determiners, never to relabel a pronoun or
// relative-clause marker. Match the whole lexical span, not just its first word.
export function determinerType(text:string):string {
  const word=text.trim().replace(/^[\p{P}\s]+|[\p{P}\s]+$/gu,'').toLowerCase();
  if(['a','an'].includes(word))return '不定冠词';
  if(word==='the')return '定冠词';
  if(['this','that','these','those'].includes(word))return '指示限定词';
  if(['my','your','his','her','its','our','their','whose'].includes(word))return '物主限定词';
  if(['some','any','no','much','many','few','a few','little','a little','several','enough','all','both'].includes(word))return '数量限定词';
  if(['each','every','either','neither'].includes(word))return '分配限定词';
  if(['what','which'].includes(word))return '疑问／关系限定词';
  return '';
}

export function componentHelp(node:LayerNode,text:string):string|undefined {
  if(node.role==='head'&&node.grammar?.includes('noun_phrase'))return '这是去掉后置修饰部分后保留的核心名词短语（NP）。它与后置定语一起组成完整名词短语；继续点击可查看限定词、前置定语和中心词。';
  if(node.role==='attribute'&&node.grammar?.includes('postmodifier'))return `${componentType(node)||'修饰成分'}放在被修饰的名词之后，作后置定语，进一步说明这个名词。`;
  if(node.role==='attribute'&&node.grammar?.includes('premodifier'))return '前置定语放在被修饰的名词之前，说明其性质、类别等；也可以由多个词共同构成。';
  if(node.role==='determiner') {
    const subtype=determinerType(text);
    if(subtype==='不定冠词')return '不定冠词 a/an，用于单数可数名词前，通常表示某一类中的一个；a/an 的选择取决于后面词语开头的发音。';
    if(subtype==='定冠词')return '定冠词 the，通常表示语境中可以确定或已经提到的人或事物。';
    return `${subtype||'限定词'}，限定名词的指称范围、所属或数量，具体含义结合当前短语理解。`;
  }
  if(node.grammar?.includes('modal_verb'))return '情态动词与主要动词共同构成谓语，表达可能性、能力、许可、义务等；具体含义由语境决定。';
  if(node.grammar?.includes('auxiliary_verb'))return '助动词与主要动词共同构成谓语，用于时态、语态、疑问、否定或强调。';
  if(node.grammar?.includes('linking_verb'))return '系动词连接主语与表语，后面的表语说明主语的身份、性质或状态。';
  if(node.grammar?.includes('lexical_verb'))return '实义动词承载动作、事件或状态的主要词义；与情态动词、助动词连用时，共同组成同一个谓语。';
  return undefined;
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
