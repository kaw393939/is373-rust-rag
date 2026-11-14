"""Smoke-test the packaged tools and real extraction, without model downloads or API keys."""
import json
import socket
import subprocess
import sys
import time
import urllib.request
import uuid

image=sys.argv[1] if len(sys.argv)>1 else 'is373-rust-rag:local'
name='is373-smoke-'+uuid.uuid4().hex[:10]
with socket.socket() as listener:
    listener.bind(('127.0.0.1',0));port=listener.getsockname()[1]
base=f'http://127.0.0.1:{port}/api'

def request(path,data=None,headers=None):
    h={'Origin':f'http://127.0.0.1:{port}'};h.update(headers or {})
    if isinstance(data,dict):data=json.dumps(data).encode();h['Content-Type']='application/json'
    return urllib.request.urlopen(urllib.request.Request(base+path,data,h),timeout=30)
def api(path,data=None):
    with request(path,data) as response:return json.load(response)
def upload(filename,content):
    boundary=uuid.uuid4().hex
    body=(f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{filename}"\r\nContent-Type: application/octet-stream\r\n\r\n').encode()+content+f'\r\n--{boundary}--\r\n'.encode()
    with request('/attachments',body,{'Content-Type':'multipart/form-data; boundary='+boundary}) as response:return json.load(response)
def pdf():
    stream=b'BT /F1 20 Tf 40 700 Td (SMOKE-PDF-42) Tj ET'
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',b'<< /Length '+str(len(stream)).encode()+b' >>\nstream\n'+stream+b'\nendstream']
    data=bytearray(b'%PDF-1.4\n');offsets=[]
    for i,obj in enumerate(objects,1):offsets.append(len(data));data.extend(f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n')
    xref=len(data);data.extend(f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'.encode())
    for offset in offsets:data.extend(f'{offset:010d} 00000 n \n'.encode())
    data.extend(f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    return bytes(data)
def prepare(file):
    cid=api('/conversations',{})['id']
    with request('/chat',{'conversation_id':cid,'question':'Read this attachment.','mode':'chat','attachments':[file['id']]}) as response:
        events=response.read().decode()
    assert 'run.ended' in events and '"outcome":"failed"' in events,events
    return api('/conversations/'+cid)['messages'][0]['attachments'][0]
try:
    subprocess.run(['docker','run','-d','--name',name,'-p',f'127.0.0.1:{port}:3730',image],check=True,stdout=subprocess.DEVNULL)
    for attempt in range(40):
        try:
            assert api('/health')['status']=='ok';break
        except Exception:
            if attempt==39:raise
            time.sleep(0.5)
    uid=subprocess.check_output(['docker','exec',name,'id','-u'],text=True).strip();assert uid=='10001',uid
    for command in [['ffmpeg','-version'],['ffprobe','-version'],['pdftotext','-v'],['pdfinfo','-v'],['pdftoppm','-v'],['whisper-cli','--help']]:
        subprocess.run(['docker','exec',name]+command,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    a=prepare(upload('smoke.pdf',pdf()))
    assert 'SMOKE-PDF-42' in a['document']['pages'][0]['text'] and a['input_state']=='ready'
    subprocess.run(['docker','exec',name,'ffmpeg','-v','error','-nostdin','-f','lavfi','-i','color=c=blue:s=160x90:d=1','-c:v','mpeg4','/tmp/smoke.mp4'],check=True)
    content=subprocess.check_output(['docker','exec',name,'cat','/tmp/smoke.mp4'])
    a=prepare(upload('smoke.mp4',content))
    assert len(a['video']['frames'])==3 and a['video']['audio_state']=='no_audio'
    asset=a['video']['frames'][0]['image_id']
    with request(f'/attachments/{a["id"]}/assets/{asset}') as response:assert response.read().startswith(b'\xff\xd8')
    print('PASS: non-root runtime, health, mapped-port Origin, native tools, PDF extraction and sampled video assets; model inference intentionally unavailable')
finally:
    subprocess.run(['docker','rm','-f',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
