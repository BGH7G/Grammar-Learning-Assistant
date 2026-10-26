import fs from 'node:fs';
import assert from 'node:assert/strict';
import ts from 'typescript';
const source = fs.readFileSync(new URL('../src/componentLabels.ts', import.meta.url), 'utf8');
const js = ts.transpileModule(source, {compilerOptions: {module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const m = await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
const make=(id,role,kind='phrase',parentId=null,ctype='none')=>({id,role,kind,parentId,ctype});
const root=make('r','root','sentence'), subject=make('s','subject','word','r'), predicate=make('p','predicate','phrase','r','coordinate');
const first=make('a','coordinate','phrase','p'), second=make('b','coordinate','phrase','p');
const layer={nodes:[first,second]}, layers={r:{nodes:[subject,predicate]},p:layer};
const nodes=new Map([root,subject,predicate,first,second].map(n=>[n.id,n]));
assert.equal(m.componentRole(first,predicate,layer.nodes),'谓语部分 1');
assert.equal(m.componentRole(second,predicate,layer.nodes),'谓语部分 2');
assert.equal(m.componentType(predicate),'并列结构');
assert.equal(m.coordinatedPredicate(predicate,layer),true);
assert.equal(m.predicateSubject(first,nodes,layers),subject);
const clause=make('c','coordinate','clause','p','coordinate');
assert.equal(m.coordinatedPredicate(predicate,{nodes:[first,clause]}),false);
assert.equal(m.componentRole(clause,predicate,[clause]),'分句 1');
const inner=make('i','object','clause','r','noun'), verb=make('v','predicate','phrase','i');
nodes.set('i',inner);nodes.set('v',verb);
assert.equal(m.predicateSubject(verb,nodes,{...layers,i:{nodes:[verb]}}),undefined);
for(const text of ['strike','including','as','240m','， strike','+']) assert.equal(m.isPunctuationOnly(text),false,text);
for(const text of [',','。','“ ”','-']) assert.equal(m.isPunctuationOnly(text),true,text);
assert.equal(m.componentRole(make('wrong','punctuation')),'待核对');
console.log('Component role, subject-boundary and lexical-visibility checks passed.');
const core={...make('np','head'),grammar:['noun_phrase']};
const post={...make('post','attribute','clause',null,'participial'),grammar:['postmodifier','past_participial']};
assert.equal(m.componentRole(core),'核心名词短语');
assert.equal(m.componentType(core),'NP');
assert.equal(m.componentRole(post),'后置定语');
assert.equal(m.componentType(post),'过去分词短语');
assert.equal(m.componentType({...post,grammar:['postmodifier','present_participial']}),'现在分词短语');
assert.equal(m.componentType({...post,ctype:'relative',grammar:['postmodifier']}),'关系从句');
assert.equal(m.componentRole({...post,grammar:[]}), '定语'); // older sessions remain usable
assert.match(m.componentHelp(core,''),/完整名词短语/);
for(const [text,label] of [['a','不定冠词'],['AN,','不定冠词'],['“The','定冠词'],['my','物主限定词'],['those','指示限定词'],['a few','数量限定词'],['each','分配限定词']]) {
  assert.equal(m.componentRole(make('d','determiner','word'),undefined,[],text),`限定词 · ${label}`);
}
for(const text of ['a British','someone else’s','unknown']) assert.equal(m.determinerType(text),'');
assert.equal(m.componentRole(make('rel','subject','word'),undefined,[],'that'),'主语');
assert.match(m.componentHelp(make('d','determiner'), 'an'),/发音/);
console.log('NP hierarchy, modifier and determiner teaching-label checks passed.');
for(const [role,tag,label] of [
  ['predicate','modal_verb','情态动词'],['predicate','auxiliary_verb','助动词'],
  ['predicate','lexical_verb','实义动词'],['predicate','linking_verb','系动词'],
  ['subject','pronoun','代词'],['head','noun','名词'],['head','proper_noun','专有名词'],
  ['attribute','adjective','形容词'],['adverbial','adverb','副词'],['head','preposition','介词'],
  ['conjunction','coordinating_conjunction','并列连词'],['marker','subordinator','从属连词'],
]) assert.equal(m.componentType({...make('typed',role,'word'),grammar:[tag]}),label);
assert.equal(m.componentType({...make('pp','adverbial'),grammar:['prepositional_phrase']}),'介词短语');
assert.equal(m.componentType({...make('vp','predicate'),grammar:['verb_phrase']}),'动词短语');
assert.equal(m.componentType({...make('inf','object','clause',null,'infinitive'),grammar:['verb_phrase']}),'不定式结构');
assert.equal(m.componentRole({...make('well','marker','word'),grammar:['discourse_marker']}),'独立成分');
assert.equal(m.componentType({...make('verb','predicate','word')}),''); // no spelling-based guess for have/do/be
assert.match(m.componentHelp({...make('may','predicate','word'),grammar:['modal_verb']},'may'),/共同构成谓语/);
console.log('Word-class labels, structure priority and verb-type explanations passed.');
