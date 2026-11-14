"""Real local inference checks. Supply a screenshot containing 'Ordo' and spoken-note.wav.

Fixtures live under ignored data/qa. See docs/multimedia.md for creation commands.
No mocked model output; nonzero exit means an assertion failed. Archives QA conversations.
"""
import json
import pathlib
import time
import urllib.request
import uuid

BASE = 'http://127.0.0.1:3730/api'
ROOT = pathlib.Path(__file__).resolve().parents[1]
results = []

def request(path, data=None, method=None):
    raw = json.dumps(data).encode() if data is not None else None
    return urllib.request.urlopen(urllib.request.Request(BASE+path, raw, {'Content-Type':'application/json'}, method=method), timeout=240)

def api(path, data=None, method=None):
    with request(path, data, method) as response:
        return json.load(response)

def upload(name, data, mime='application/octet-stream'):
    boundary = 'ordo-'+uuid.uuid4().hex
    body = (f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{name}"\r\nContent-Type: {mime}\r\n\r\n').encode()+data+f'\r\n--{boundary}--\r\n'.encode()
    req = urllib.request.Request(BASE+'/attachments', body, {'Content-Type':f'multipart/form-data; boundary={boundary}'})
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)

def ask(cid, text, attachments=(), mode='chat'):
    started = time.monotonic()
    events = []
    with request('/chat', {'conversation_id':cid,'question':text,'attachments':list(attachments),'mode':mode}) as response:
        for raw in response:
            line = raw.decode().strip()
            if line.startswith('event:'): name = line[6:].strip()
            if line.startswith('data:'): events.append((name,json.loads(line[5:])))
    answer = ''.join(data['text'] for name,data in events if name == 'answer.delta')
    terminal = events[-1][1]
    saved = api('/conversations/'+cid)
    assert saved['messages'][-1]['content'] == answer
    assert saved['messages'][-1]['outcome'] == terminal['outcome']
    row = {'question':text,'mode':mode,'seconds':round(time.monotonic()-started,2),'answer':answer,'terminal':terminal,'model':saved['messages'][-1]['model']}
    results.append(row)
    (ROOT/'data/multimedia-results.json').write_text(json.dumps(results,indent=2))
    print(json.dumps(row), flush=True)
    return answer,terminal,events,saved

def main():
    status = api('/status')
    assert status['capabilities']['image_input'] and status['capabilities']['audio_input'], status
    cid = api('/conversations',{},'POST')['id']
    try:
        wav = upload('spoken-note.wav',(ROOT/'data/qa/spoken-note.wav').read_bytes())
        answer,end,events,saved = ask(cid,'What day and time is the workshop, and what should I bring?',[wav['id']])
        assert end['outcome'] == 'complete', end
        assert 'friday' in answer.lower() and 'blue' in answer.lower() and ('3' in answer or 'three' in answer.lower()), answer
        transcript = saved['messages'][0]['attachments'][0]['transcript']
        assert 'Friday' in transcript['text'] and transcript['provider'] == 'whisper_cpp'
        assert any(name=='input.progress' and data['stage']=='transcribing' for name,data in events)
        answer,end,_,_ = ask(cid,'What color notebook did that recording mention?')
        assert end['outcome']=='complete' and 'blue' in answer.lower(), answer
        answer,end,_,saved = ask(cid,'According to the attached recording, what day is the workshop? Cite the recording.',[wav['id']],'vector')
        assert end['outcome']=='complete' and 'friday' in answer.lower() and f'[attachment-{wav["id"]}:p1]' in answer, answer
        assert saved['messages'][-2]['attachments'][0]['transcript']==transcript
    finally:
        api('/conversations/'+cid,{'archived':True},'PATCH')

    cid = api('/conversations',{},'POST')['id']
    try:
        img = upload('workspace.jpg',(ROOT/'data/qa/interface.jpg').read_bytes())
        answer,end,_,saved = ask(cid,'Read the brand name in the top left corner of this screenshot. Answer briefly.',[img['id']])
        assert end['outcome']=='complete' and 'ordo' in answer.lower(), answer
        assert saved['messages'][-1]['model']==status['vision_model']
        answer,end,_,saved = ask(cid,'Read the two large headline lines in that screenshot.')
        assert end['outcome']=='complete' and 'curiosity' in answer.lower() and 'possibility' in answer.lower(), answer
        assert saved['messages'][-1]['model']==status['vision_model']
        answer,end,_,_ = ask(cid,'What brand name appears in the screenshot? Cite the image source.',[img['id']],'vector')
        assert end['outcome']=='complete' and 'ordo' in answer.lower() and f'[attachment-{img["id"]}:p1]' in answer, answer
    finally:
        api('/conversations/'+cid,{'archived':True},'PATCH')

    cid = api('/conversations',{},'POST')['id']
    try:
        broken = upload('broken.wav',b'RIFF'+bytes(4)+b'WAVE'+b'not audio')
        _,end,_,saved = ask(cid,'Transcribe this.',[broken['id']])
        assert end['outcome']=='failed' and saved['messages'][-2]['attachments'][0]['input_state']=='failed', end
    finally:
        api('/conversations/'+cid,{'archived':True},'PATCH')

    formats = [('spoken-note.weba','audio/webm'),('spoken-note.m4a','audio/mp4')]
    if all((ROOT/'data/qa'/name).exists() for name,_ in formats):
        cid = api('/conversations',{},'POST')['id']
        try:
            files = [upload(name,(ROOT/'data/qa'/name).read_bytes(),mime) for name,mime in formats]
            assert [f['mime'] for f in files] == [mime for _,mime in formats]
            answer,end,_,saved = ask(cid,'What should I bring according to these voice notes?',[f['id'] for f in files])
            assert end['outcome']=='complete' and 'blue notebook' in answer.lower(),answer
            assert all('blue notebook' in a['transcript']['text'].lower() for a in saved['messages'][0]['attachments'])
        finally:
            api('/conversations/'+cid,{'archived':True},'PATCH')
    print('PASS: live audio, vision, follow-ups, source citations, cache, persistence and corrupt-media failure',flush=True)

if __name__ == '__main__': main()
