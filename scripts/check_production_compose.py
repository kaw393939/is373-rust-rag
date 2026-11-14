"""Boot the actual production Compose topology with synthetic credentials and isolated names.
No real Groq calls or existing proxy/network changes. TLS itself is verified after deployment.
"""
import json,os,secrets,shutil,socket,subprocess,sys,tempfile,urllib.request,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
app_image=sys.argv[1];embedding_image=sys.argv[2]
name='ordo-compose-test-'+uuid.uuid4().hex[:8]
with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
def docker(*args,**kw):return subprocess.run(['docker',*args],check=True,**kw)
with tempfile.TemporaryDirectory(prefix=name) as directory:
    root=Path(directory);(root/'secrets').mkdir();(root/'secrets/groq-api-key').write_text('synthetic-not-a-provider-key')
    (root/'secrets/groq-api-key').chmod(0o444) # Synthetic fixture only; production key is 0400 UID 10001.
    shutil.copy(ROOT/'deploy/compose.production.yaml',root/'compose.yaml')
    env={**os.environ,'ORDO_IMAGE':app_image,'ORDO_EMBEDDING_IMAGE':embedding_image,'ORDO_DOMAIN':'ordo.test.invalid'}
    def compose(*args,**kw):return docker('compose','--project-directory',str(root),'-p',name,*args,env=env,**kw)
    config=json.loads(compose('--profile','setup','config','--format','json',capture_output=True,text=True).stdout)
    for service in ['app','embeddings']:
        assert all(m.startswith('/') for m in config['services'][service]['tmpfs']),config['services'][service]['tmpfs']
        assert not config['services'][service].get('ports')
        assert config['services'][service]['read_only'] is True
    assert config['networks']['private']['internal'] is True
    assert 'web' not in config['services']['embeddings']['networks']
    config['name']=name;config['networks']['web']['name']=name+'-web'
    config['services']['app']['ports']=[{'target':3730,'published':str(port),'host_ip':'127.0.0.1','protocol':'tcp'}]
    config['services']['app']['labels']={} # This test must never register a route with another task's proxy.
    (root/'compose.yaml').write_text(json.dumps(config))
    docker('network','create',name+'-web',stdout=subprocess.DEVNULL)
    try:
        compose('--profile','setup','run','--rm','--no-deps','model-download')
        compose('up','-d','--wait','--wait-timeout','90','embeddings')
        compose('run','--rm','--no-deps','app','import',stdout=subprocess.DEVNULL)
        compose('up','-d','--wait','--wait-timeout','90','app')
        from check_auth import Client
        client=Client(f'http://127.0.0.1:{port}')
        headers={'Host':'ordo.test.invalid','Origin':'https://ordo.test.invalid'}
        client.api('/conversations',headers=headers,expected=401)
        link=compose('exec','-T','app','is373-rag','auth','invite','compose-owner','--admin',capture_output=True,text=True).stdout.strip()
        _,h=client.call('/auth/enroll',{'token':link.split('#invite=')[1],'password':secrets.token_urlsafe(24)},headers=headers)
        assert '__Host-ordo_session=' in h['set-cookie'] and '; Secure' in h['set-cookie']
        assert len(client.api('/sources',headers=headers))==5
        assert len(client.api('/control/catalog',headers=headers)['tools'])==8
        client.api('/auth/logout',{},headers=headers)
        print('PASS: production Compose mounts, limits, isolated networks, verified models, corpus import, startup health and secure-cookie authentication')
    finally:
        compose('--profile','setup','down','--volumes','--remove-orphans',stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        docker('network','rm',name+'-web',stdout=subprocess.DEVNULL)
