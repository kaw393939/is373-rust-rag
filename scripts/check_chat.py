"""Live local API contract checks; no cloud access. Server must be running."""
import json
import time
import urllib.request
import urllib.error
import uuid

BASE = 'http://127.0.0.1:3730/api'

def call(path, data=None, method=None, headers=None):
    body = json.dumps(data).encode() if data is not None else None
    return urllib.request.urlopen(urllib.request.Request(BASE+path,body,{'Content-Type':'application/json',**(headers or {})},method=method),timeout=180)

def value(path,data=None,method=None):
    with call(path,data,method) as r: return json.load(r)

def upload(name, data):
    boundary='test-'+uuid.uuid4().hex
    body=(f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{name}"\r\nContent-Type: application/octet-stream\r\n\r\n').encode()+data+f'\r\n--{boundary}--\r\n'.encode()
    request=urllib.request.Request(BASE+'/attachments',body,{'Content-Type':f'multipart/form-data; boundary={boundary}'})
    with urllib.request.urlopen(request,timeout=30) as r: return json.load(r)

def ask(cid, question, files=None):
    answer=''; outcome=None
    with call('/chat',{'conversation_id':cid,'question':question,'mode':'chat','attachments':files or []}) as response:
        name=None
        for raw in response:
            line=raw.decode().strip()
            if line.startswith('event:'): name=line[6:].strip()
            if line.startswith('data:'):
                data=json.loads(line[5:])
                if name=='answer.delta':answer+=data['text']
                if name=='run.ended':outcome=data['outcome']
    assert outcome=='complete',(outcome,answer)
    return answer

for headers in ({'Origin':'https://untrusted.example'},{'Host':'untrusted.example'}):
    try:
        call('/conversations',None,'GET',headers)
        raise AssertionError('Untrusted origin/host accepted')
    except urllib.error.HTTPError as e: assert e.code==403,e.code
print('Origin and host isolation: PASS',flush=True)

try:
    upload('active.svg',b'<svg onload="alert(1)"/>')
    raise AssertionError('SVG accepted')
except urllib.error.HTTPError as e: assert e.code==415,e.code
file=upload('reference.txt',b'The verification code is HARBOR-942. This is a synthetic test note.')
with call('/attachments/'+file['id'],headers={'Range':'bytes=0-9'}) as r:
    assert r.status==206 and len(r.read())==10
    assert r.headers['X-Content-Type-Options']=='nosniff'
print('Attachment type rejection, upload, download and byte ranges: PASS',flush=True)

c=value('/conversations',{},'POST')
cid=c['id']
try:
    value('/conversations/'+cid,{'title':'API verification · attachment context','pinned':True},'PATCH')
    answer=ask(cid,'What is the verification code in the attached text? Answer with just the code.',[file['id']])
    assert 'HARBOR-942' in answer,answer
    followup=ask(cid,'Repeat that same verification code, without any explanation.')
    assert 'HARBOR-942' in followup,followup
    saved=value('/conversations/'+cid)
    assert len(saved['messages'])==4 and saved['messages'][0]['attachments'][0]['id']==file['id']
    assert saved['messages'][-1]['content']==followup and saved['pinned']
    fork=value('/conversations/'+cid+'/fork',{'before':saved['messages'][2]['id']},'POST')
    assert len(fork['messages'])==2
    assert len(value('/conversations/'+cid)['messages'])==4
    value('/conversations/'+fork['id'],{'archived':True},'PATCH')
    print('Generation, attached text, follow-up context, persistence and non-destructive branching: PASS',flush=True)
    value('/conversations/'+cid,{'archived':True},'PATCH')
    try:
        ask(cid,'Hello')
        raise AssertionError('Archived chat accepted new messages')
    except urllib.error.HTTPError as e: assert e.code==409,e.code
    value('/conversations/'+cid,{'archived':False},'PATCH')
    assert not value('/conversations/'+cid)['archived']
    print('Rename, pin, archive and restore: PASS',flush=True)
finally:
    value('/conversations/'+cid,{'archived':True,'pinned':False},'PATCH')
