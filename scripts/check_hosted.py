"""Live hosted-provider acceptance check. Reads a private test-account file, never model keys.
Usage: python3 scripts/check_hosted.py ORIGIN PRIVATE_ACCOUNT_JSON
Synthetic fixtures only. This makes real, metered provider calls.
"""
import json,sys,uuid,time
from pathlib import Path
from check_auth import Client
ROOT=Path(__file__).resolve().parents[1]
base=sys.argv[1];credentials=json.loads(Path(sys.argv[2]).read_text());client=Client(base)
client.api('/auth/login',credentials)
status=client.api('/status');assert status['ready'] and status['chat_available'] and status['vision_available'] and status['transcription']['ready'],status
rows=[]
def upload(path):
    boundary=uuid.uuid4().hex
    body=(f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{path.name}"\r\nContent-Type: application/octet-stream\r\n\r\n').encode()+path.read_bytes()+f'\r\n--{boundary}--\r\n'.encode()
    return client.api('/attachments',body,headers={'Content-Type':'multipart/form-data; boundary='+boundary})
def ask(question,files=(),mode='chat'):
    cid=client.api('/conversations',{})['id']
    try:
        raw=client.api('/chat',{'question':question,'mode':mode,'conversation_id':cid,'attachments':list(files)})
        events=[]
        for line in raw.decode().splitlines():
            if line.startswith('event:'):name=line[6:].strip()
            if line.startswith('data:'):events.append((name,json.loads(line[5:])))
        answer=''.join(d['text'] for n,d in events if n=='answer.delta');end=events[-1][1]
        saved=client.api('/conversations/'+cid)
        assert end['outcome']=='complete',(end,answer)
        assert saved['messages'][-1]['content']==answer
        rows.append({'question':question,'answer':answer,'outcome':end['outcome']})
        return answer,saved
    finally:client.api('/conversations/'+cid,{'archived':True},'PATCH')
answer,_=ask('What is the annual membership fee at Harbor Club? Cite its source.',mode='vector')
assert '$15' in answer and '[membership:p2]' in answer
if (ROOT/'tmp/pdfs/workshop.pdf').exists():
    pdf=upload(ROOT/'tmp/pdfs/workshop.pdf');answer,_=ask('What color notebook should I bring according to this PDF? Cite its page.',[pdf['id']],'vector')
    assert 'orange' in answer.lower() and f'[attachment-{pdf["id"]}:p2]' in answer
    scan=upload(ROOT/'tmp/pdfs/scanned.pdf');answer,_=ask('What launch color is written on this scanned page?',[scan['id']])
    assert 'purple' in answer.lower()
    # The free hosted vision allowance is 7,000 input tokens/minute on the test account.
    # Pace separate live vision tests; runtime does not silently retry partial generations.
    time.sleep(55)
    video=upload(ROOT/'data/qa/compound/narrated.mp4');answer,saved=ask('Name the colors in the first and last sampled video frames and the access word spoken in the audio.',[video['id']])
    assert all(w in answer.lower() for w in ['red','green','cedar']),answer
    attached=saved['messages'][0]['attachments'][0]
    assert attached['transcript']['provider']=='groq' and len(attached['video']['frames'])==2
    rows[-1]['transcript']=attached['transcript']['text']
plan=client.api('/control/propose',{'request':'Show the current feature flags without changing anything.'})
assert plan['action']=={'tool':'flags.list','arguments':{}},plan
result=client.api('/control/execute',{'id':plan['id']})
assert result['result']['chat'] is True
Path('data/hosted-acceptance.json').write_text(json.dumps({'origin':base,'rows':rows,'admin_plan':plan['action'],'flags_read':True},indent=2)+'\n')
print('PASS: authenticated Groq streaming, local semantic retrieval, PDF extraction, scanned-page vision, sampled video + speech, persisted messages and conversational admin planning')
client.api('/auth/logout',{})
