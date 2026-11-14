"""Real local PDF/video inference. Generate fixtures with make_document_video_fixtures.py first."""
import json
import time
import urllib.request
import urllib.error
from pathlib import Path
import uuid

ROOT=Path(__file__).resolve().parents[1]
BASE='http://127.0.0.1:3730/api'
RESULTS_PATH=ROOT/'data/document-video-results.json'
results=[]
quality_failures=[]
def request(path,data=None,method=None):
    return urllib.request.urlopen(urllib.request.Request(BASE+path,json.dumps(data).encode() if data is not None else None,{'Content-Type':'application/json'},method=method),timeout=360)
def api(path,data=None,method=None):
    with request(path,data,method) as r:return json.load(r)
def upload(path):
    boundary='test-'+uuid.uuid4().hex
    body=(f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{path.name}"\r\nContent-Type: application/octet-stream\r\n\r\n').encode()+path.read_bytes()+f'\r\n--{boundary}--\r\n'.encode()
    with urllib.request.urlopen(urllib.request.Request(BASE+'/attachments',body,{'Content-Type':f'multipart/form-data; boundary={boundary}'})) as r:return json.load(r)
def ask(cid,q,files=(),mode='chat'):
    start=time.monotonic(); events=[]
    with request('/chat',{'conversation_id':cid,'question':q,'attachments':list(files),'mode':mode}) as response:
        for line in response:
            line=line.decode().strip()
            if line.startswith('event:'): name=line[6:].strip()
            if line.startswith('data:'):events.append((name,json.loads(line[5:])))
    answer=''.join(d['text'] for n,d in events if n=='answer.delta'); end=events[-1][1]
    c=api('/conversations/'+cid)
    assert c['messages'][-1]['outcome']==end['outcome'] and c['messages'][-1]['content']==answer
    row={'question':q,'mode':mode,'answer':answer,'terminal':end,'seconds':round(time.monotonic()-start,2)}
    results.append(row);RESULTS_PATH.write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(row),flush=True)
    return answer,end,c
def main():
    s=api('/status'); assert s['capabilities']['pdf_input'] and s['capabilities']['video_input'],s
    doc=upload(ROOT/'tmp/pdfs/workshop.pdf')
    cid=api('/conversations',{},'POST')['id']
    try:
        answer,end,c=ask(cid,'What color notebook should I bring according to the PDF? Cite its page.',[doc['id']],'vector')
        assert end['outcome']=='complete' and 'orange' in answer.lower() and f'[attachment-{doc["id"]}:p2]' in answer,answer
        pages=c['messages'][0]['attachments'][0]['document']['pages']
        assert len(pages)==2 and 'Cedar 204' in pages[0]['text'] and 'orange notebook' in pages[1]['text']
        answer,end,c=ask(cid,'Which room is on the first page of that PDF? Cite the page.',mode='vector')
        assert end['outcome']=='complete' and 'Cedar 204' in answer and f'[attachment-{doc["id"]}:p1]' in answer,answer
    finally: api('/conversations/'+cid,{'archived':True},'PATCH')

    video=upload(ROOT/'data/qa/compound/narrated.mp4'); cid=api('/conversations',{},'POST')['id']
    try:
        answer,end,c=ask(cid,'Name the colors shown in the first and last sampled video frames, then give the access word spoken in the audio. Be concise.',[video['id']])
        assert end['outcome']=='complete' and all(w in answer.lower() for w in ['red','green']),answer
        a=c['messages'][0]['attachments'][0]; frames=a['video']['frames']
        assert len(frames)==3 and a['video']['audio_state']=='transcribed' and a['transcript']['text']
        results[-1]['transcript']=a['transcript']['text']
        exact_speech='cedar' in a['transcript']['text'].lower() and 'cedar' in answer.lower()
        results[-1]['speech_accuracy_pass']=exact_speech
        if not exact_speech: quality_failures.append('Video speech: expected cedar; inspect the saved transcript and answer.')
        assert [round(f['timestamp_seconds'],1) for f in frames]==[1.5,4.5,7.5]
        with request(f'/attachments/{video["id"]}/assets/{frames[1]["image_id"]}') as r:
            assert r.headers['Content-Type']=='image/jpeg' and r.read().startswith(b'\xff\xd8')
        try: request(f'/attachments/{doc["id"]}/assets/{frames[1]["image_id"]}')
        except urllib.error.HTTPError as e: assert e.code==404
        else: raise AssertionError('Asset association was not enforced')
        answer,end,c=ask(cid,'What color is visible in the middle sampled frame? Cite that frame.',mode='vector')
        assert end['outcome']=='complete' and 'blue' in answer.lower() and f'[attachment-{video["id"]}:f2]' in answer,answer
        assert c['messages'][0]['attachments'][0]['video']['frames']==frames
    finally: api('/conversations/'+cid,{'archived':True},'PATCH')

    scan=upload(ROOT/'tmp/pdfs/scanned.pdf');cid=api('/conversations',{},'POST')['id']
    try:
        answer,end,c=ask(cid,'What launch color is written on this scanned PDF page?',[scan['id']])
        assert end['outcome']=='complete' and 'purple' in answer.lower(),answer
        page=c['messages'][0]['attachments'][0]['document']['pages'][0]
        assert not page['text'] and page['image_id']
    finally: api('/conversations/'+cid,{'archived':True},'PATCH')

    for name in ['locked.pdf','corrupt.pdf','too-many-pages.pdf','too-much-text.pdf']:
        a=upload(ROOT/'tmp/pdfs'/name); cid=api('/conversations',{},'POST')['id']
        try:
            answer,end,c=ask(cid,'Summarize this PDF.',[a['id']])
            assert end['outcome']=='failed' and not answer and c['messages'][0]['attachments'][0]['input_state']=='failed'
        finally:api('/conversations/'+cid,{'archived':True},'PATCH')
    print('PASS: PDF pages/citations/follow-ups, video frame/audio plumbing/citations/cache, scanned PDF vision and four explicit PDF failures',flush=True)
    if quality_failures:
        print('MODEL QUALITY FAILURES: '+json.dumps(quality_failures),flush=True)
        raise SystemExit(1)
if __name__=='__main__':main()
