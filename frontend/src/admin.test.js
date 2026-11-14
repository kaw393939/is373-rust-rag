import test from 'node:test';
import assert from 'node:assert/strict';
import {adminHistory} from './lib/admin.js';
test('admin follow-up context retains intent and never sends result credentials',()=>{
  const history=adminHistory([
    {kind:'request',text:'Invite a student'},
    {kind:'question',text:'What username?'},
    {kind:'request',text:'alex373'},
    {kind:'proposal',action:{tool:'users.invite',arguments:{username:'alex373'}}},
    {kind:'result',result:{invitation_url:'https://private.invalid/#invite=SECRET'},receipt:'SECRET-RECEIPT'}
  ]);
  assert.equal(history.length,4);
  assert.equal(history[1].role,'assistant');
  assert.match(history[3].content,/execution is not asserted/);
  assert.doesNotMatch(JSON.stringify(history),/SECRET|invitation_url/);
  const long=adminHistory(Array.from({length:10},()=>({kind:'request',text:'é'.repeat(900)})));
  assert.ok(long.length<=6&&new TextEncoder().encode(long.map(t=>t.content).join('')).length<=6000);
});
