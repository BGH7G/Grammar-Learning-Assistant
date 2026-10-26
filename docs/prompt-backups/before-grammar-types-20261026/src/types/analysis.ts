export type Role = 'root'|'subject'|'predicate'|'object'|'attribute'|'adverbial'|'complement'|'predicative';
export interface AnalysisNode { id:string; parent_id:string|null; kind:'clause'|'phrase'|'word'; role:Role; first:number; last:number }
export interface Analysis { schema_version:string; pieces:{id:number;text:string}[]; nodes:AnalysisNode[]; translation:string; notes:string[] }
export type ThinkingProtocol = 'auto' | 'deepseek' | 'reasoning_effort' | 'omit';
export type ReasoningEffort = 'low' | 'high' | 'max';
export interface Settings {
  endpoint: string;
  model: string;
  jsonMode: boolean;
  hasKey: boolean;
  temperature: number;
  thinking: boolean;
  reasoningEffort: ReasoningEffort;
  thinkingProtocol: ThinkingProtocol;
}
export interface AttemptMetrics {
  requestMs: number;
  responseWaitMs: number;
  responseReadMs: number;
  validationMs: number;
  inputTokens: number | null;
  outputTokens: number | null;
  reasoningTokens: number | null;
  repairReason: string | null;
  stage?: string | null;
  cachedInputTokens?: number | null;
}
export interface AnalysisMetrics {
  totalMs: number;
  configurationMs: number;
  preparationMs: number;
  attempts: AttemptMetrics[];
}
export type LayerRole = Role | 'main_clause' | 'conjunction' | 'coordinate' | 'head' | 'determiner' | 'marker' | 'punctuation' | 'other';
export type ClauseType = 'main'|'coordinate'|'relative'|'noun'|'adverbial'|'infinitive'|'participial'|'gerund'|'none';
export type GrammarDetail = 'noun_phrase'|'premodifier'|'postmodifier'|'past_participial'|'present_participial';
export interface LayerToken { id: number; text: string; pieceId: number }
export interface LayerNode {
  id: string; parentId: string | null; kind: 'sentence'|'clause'|'phrase'|'word'; role: LayerRole;
  ctype: ClauseType; first: number; last: number; expandable: boolean; depth: number; sentenceId: string;
  grammar?: GrammarDetail[];
}
export interface AnalysisLayer {
  parentId: string; depth: number; skeleton: string | null; translation: string; nodes: LayerNode[];
  relations: { sourceId: string; headToken: number }[]; notes: string[]; warnings?: string[];
}
export interface LayerDocument {
  schemaVersion: '2'; promptVersion: string; sessionId: string; pieces: {id:number;text:string}[];
  tokens: LayerToken[]; roots: LayerNode[]; layers: AnalysisLayer[]; maxDepth: number;
}
export interface AnalysisResult { requestId: string; document: LayerDocument; metrics: AnalysisMetrics }
export interface ExpansionResult { requestId: string; sessionId: string; layer: AnalysisLayer | null; error: string | null; metrics: AnalysisMetrics; cached: boolean }

