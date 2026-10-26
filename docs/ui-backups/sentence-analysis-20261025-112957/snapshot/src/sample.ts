import type { Analysis } from './types/analysis';
export const sample:Analysis={schema_version:'1',pieces:['The cat',' ','that',' ','I',' ','saw',' ','yesterday',' ','sat',' ','on the mat','.'].map((text,id)=>({id,text})),nodes:[
{id:'s',parent_id:null,kind:'clause',role:'root',first:0,last:13},
{id:'subject',parent_id:'s',kind:'phrase',role:'subject',first:0,last:8},
{id:'predicate',parent_id:'s',kind:'word',role:'predicate',first:10,last:10},
{id:'adverbial',parent_id:'s',kind:'phrase',role:'adverbial',first:12,last:12},
{id:'relative',parent_id:'subject',kind:'clause',role:'attribute',first:2,last:8},
{id:'object',parent_id:'relative',kind:'word',role:'object',first:2,last:2},
{id:'i',parent_id:'relative',kind:'word',role:'subject',first:4,last:4},
{id:'saw',parent_id:'relative',kind:'word',role:'predicate',first:6,last:6},
{id:'yesterday',parent_id:'relative',kind:'word',role:'adverbial',first:8,last:8}],translation:'我昨天看见的那只猫坐在垫子上。',notes:['that I saw yesterday 是修饰 The cat 的定语从句；that 在从句中作 saw 的宾语。','这是固定的离线示例，不调用模型。']};
