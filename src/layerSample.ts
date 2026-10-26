import type { AnalysisLayer, LayerDocument, LayerNode } from "./types/analysis";
const words = ['The','cat','that','I','saw','yesterday','sat','on','the','mat','.'];
const pieces: LayerDocument['pieces'] = [];
const tokens = words.map((text, id) => {
  if (id > 0 && text !== '.') pieces.push({ id: pieces.length, text: ' ' });
  const pieceId = pieces.length; pieces.push({ id: pieceId, text }); return { id, text, pieceId };
});
function node(id: string, parentId: string | null, role: LayerNode['role'], first: number, last: number, depth: number, ctype: LayerNode['ctype'] = 'none', kind: LayerNode['kind'] = first === last ? 'word' : 'phrase'): LayerNode {
  return { id, parentId, role, first, last, depth, ctype, kind, expandable: first < last && depth < 4, sentenceId: 'offline:s0' };
}
const root = node('offline:s0', null, 'root', 0, 10, 0, 'none', 'sentence');
const subject = node('offline:s0:0', root.id, 'subject', 0, 5, 1);
const relative = node(subject.id + ':2', subject.id, 'attribute', 2, 5, 2, 'relative', 'clause');
const adverbial = node('offline:s0:2', root.id, 'adverbial', 7, 9, 1);
const object = node(adverbial.id + ':1', adverbial.id, 'object', 8, 9, 2);
const layers: AnalysisLayer[] = [
  { parentId: root.id, depth: 1, skeleton: 'The cat sat.', translation: '我昨天看见的那只猫坐在垫子上。', nodes: [subject, node(root.id + ':1',root.id,'predicate',6,6,1), adverbial, node(root.id + ':3',root.id,'punctuation',10,10,1)], relations: [], notes: ['离线固定示例；点击成分逐层查看，无需调用模型。'] },
  { parentId: subject.id, depth: 2, skeleton: null, translation: '我昨天看见的那只猫', nodes: [node(subject.id+':0',subject.id,'determiner',0,0,2),node(subject.id+':1',subject.id,'head',1,1,2),relative], relations: [{ sourceId: relative.id, headToken: 1 }], notes: ['that 引导的定语从句修饰 cat。'] },
  { parentId: relative.id, depth: 3, skeleton: null, translation: '我昨天看见的', nodes: [node(relative.id+':0',relative.id,'object',2,2,3),node(relative.id+':1',relative.id,'subject',3,3,3),node(relative.id+':2',relative.id,'predicate',4,4,3),node(relative.id+':3',relative.id,'adverbial',5,5,3)], relations: [], notes: ['that 在从句中充当 saw 的宾语。'] },
  { parentId: adverbial.id, depth: 2, skeleton: null, translation: '在垫子上', nodes: [node(adverbial.id+':0',adverbial.id,'marker',7,7,2),object], relations: [], notes: [] },
  { parentId: object.id, depth: 3, skeleton: null, translation: '那块垫子', nodes: [node(object.id+':0',object.id,'determiner',8,8,3),node(object.id+':1',object.id,'head',9,9,3)], relations: [], notes: [] },
];
export const layerSample: LayerDocument = { schemaVersion: '2', promptVersion: 'layer-v2.5', sessionId: 'offline', pieces, tokens, roots: [root], layers, maxDepth: 4 };
