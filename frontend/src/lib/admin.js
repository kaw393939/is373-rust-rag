// Only requests, clarifying questions and typed previews enter planner context.
// Results can contain one-time credentials; never serialize them into a prompt.
export function adminHistory(entries) {
  const bytes=text=>new TextEncoder().encode(text).length;
  const history=entries.flatMap(entry=>{
    if(entry.kind==='request')return [{role:'user',content:entry.text}];
    if(entry.kind==='question')return [{role:'assistant',content:entry.text}];
    if(entry.kind==='proposal')return [{role:'assistant',content:'Previously proposed; execution is not asserted: '+JSON.stringify(entry.action)}];
    return [];
  }).filter(turn=>typeof turn.content==='string'&&bytes(turn.content)<=2000).slice(-6);
  while(history.reduce((n,t)=>n+bytes(t.content),0)>6000)history.shift();
  return history;
}
