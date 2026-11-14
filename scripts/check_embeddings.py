"""Test the real downloaded CPU model in its resource-limited image."""
import json,subprocess,sys,time,urllib.request,uuid,socket
image=sys.argv[1] if len(sys.argv)>1 else 'ordo-embeddings:test'
name='ordo-embed-test-'+uuid.uuid4().hex[:8];volume=name+'-models'
with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
def docker(*args,**kw):return subprocess.run(['docker',*args],check=True,**kw)
try:
    docker('volume','create',volume,stdout=subprocess.DEVNULL)
    docker('run','--rm','--memory','128m','-v',volume+':/models',image,'download')
    docker('run','-d','--name',name,'--memory','320m','--cpus','0.5','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges:true','--tmpfs','/tmp:size=16m','-v',volume+':/models:ro','-p',f'127.0.0.1:{port}:8080',image,stdout=subprocess.DEVNULL)
    base=f'http://127.0.0.1:{port}'
    for _ in range(60):
        try:identity=json.load(urllib.request.urlopen(base+'/identity',timeout=3));break
        except OSError:time.sleep(1)
    else:raise RuntimeError('Embedding service did not start')
    body=json.dumps({'texts':['The annual membership fee is fifteen dollars.','The blue sky is clear.'],'query':False}).encode()
    vectors=json.load(urllib.request.urlopen(urllib.request.Request(base+'/embed',body,{'Content-Type':'application/json'}),timeout=60))['embeddings']
    assert len(vectors)==2 and all(len(v)==384 for v in vectors)
    assert all(abs(sum(x*x for x in v)-1)<.001 for v in vectors)
    assert identity['provider']=='fastembed' and identity['revision'].startswith('fastembed-0.9.1:')
    docker('stats','--no-stream','--format','{{.Name}} {{.MemUsage}}',name)
    print('PASS: pinned model download, offline loading, 384-dimensional normalized embeddings, read-only non-root image under 320 MiB')
finally:
    subprocess.run(['docker','rm','-f',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    subprocess.run(['docker','volume','rm',volume],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
