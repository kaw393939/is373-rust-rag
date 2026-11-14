#!/usr/bin/env python3
"""Root-owned, narrowly scoped Ordo release/backup operator. No arbitrary commands or paths.
The SSH key can invoke only this program through a fixed sudo wrapper.
"""
import contextlib,fcntl,json,os,re,shutil,sqlite3,subprocess,sys,tempfile,time
from pathlib import Path
ROOT=Path('/opt/ordo');BACKUPS=Path('/var/backups/ordo')
IMAGE='kaw393939/is373-rust-rag'
ENV={**os.environ,'DOCKER_CONFIG':str(ROOT/'docker-public'),'DOCKER_HOST':'unix:///var/run/docker.sock'}
def run(*args,**kwargs):return subprocess.run(args,check=True,env=ENV,**kwargs)
def compose(*args,**kwargs):return run('docker','compose','--project-directory',str(ROOT),'-f',str(ROOT/'compose.yaml'),*args,**kwargs)
def running():return subprocess.check_output(['docker','ps','-q','--filter','label=com.docker.compose.project=ordo','--filter','label=com.docker.compose.service=app'],env=ENV,text=True).strip()
def backup():
    volume=subprocess.run(['docker','volume','inspect','ordo_app-data','--format','{{.Mountpoint}}'],env=ENV,capture_output=True,text=True)
    if volume.returncode:return None
    source=Path(volume.stdout.strip())
    if not source.is_dir() or not str(source).startswith('/var/lib/docker/volumes/ordo_app-data/'):raise RuntimeError('Unexpected Ordo data volume')
    BACKUPS.mkdir(mode=0o700,parents=True,exist_ok=True)
    was_running=bool(running())
    with tempfile.TemporaryDirectory(prefix='ordo-backup-',dir=ROOT) as temp:
        stage=Path(temp)
        try:
            if was_running:compose('stop','app')
            shutil.copytree(source,stage/'data')
            for path in (stage/'data').rglob('*.sqlite'):
                with sqlite3.connect('file:'+str(path)+'?mode=ro',uri=True) as db:
                    if db.execute('PRAGMA quick_check').fetchone()[0]!='ok':raise RuntimeError('Database integrity check failed')
            for name in ['.env','compose.yaml']:
                if (ROOT/name).exists():shutil.copy2(ROOT/name,stage/name)
            shutil.copytree(ROOT/'secrets',stage/'secrets')
        finally:
            if was_running:compose('up','-d','--no-deps','app')
        destination=BACKUPS/(time.strftime('%Y%m%dT%H%M%SZ',time.gmtime())+'.tar.gz.age')
        pending=destination.with_suffix('.age.partial')
        tar=subprocess.Popen(['tar','czf','-','-C',str(stage),'.'],stdout=subprocess.PIPE)
        try:
            run('age','-R','/etc/ai-server-infra/recipients.txt','-o',str(pending),stdin=tar.stdout)
            tar.stdout.close()
            if tar.wait()!=0:raise RuntimeError('Backup archive failed')
            pending.replace(destination)
        finally:
            if tar.poll() is None:tar.terminate();tar.wait()
            pending.unlink(missing_ok=True)
    # Only this service's completed encrypted backups. Never touch other projects.
    for old in sorted(BACKUPS.glob('*.tar.gz.age'))[:-7]:old.unlink()
    print('Encrypted Ordo backup:',destination.name)
    return str(destination)
def release():
    raw=sys.stdin.buffer.read(4097)
    if len(raw)>4096:raise RuntimeError('Release request too large')
    request=json.loads(raw)
    if set(request)!={'revision','app_digest','embedding_digest'}:raise RuntimeError('Invalid release request')
    revision=request['revision']
    if not re.fullmatch('[0-9a-f]{40}',revision):raise RuntimeError('Invalid revision')
    for name in ['app_digest','embedding_digest']:
        if not re.fullmatch('sha256:[0-9a-f]{64}',request[name]):raise RuntimeError('Immutable digest required')
    current=(ROOT/'.env').read_text() if (ROOT/'.env').exists() else ''
    if (ROOT/'release.json').exists() and json.loads((ROOT/'release.json').read_text())==request:
        print('This release is already deployed');return
    for key in ['app_digest','embedding_digest']:
        image=IMAGE+'@'+request[key];run('docker','pull',image)
        labels=json.loads(subprocess.check_output(['docker','image','inspect',image,'--format','{{json .Config.Labels}}'],env=ENV))
        if labels.get('org.opencontainers.image.revision')!=revision or labels.get('org.opencontainers.image.source')!='https://github.com/kaw393939/is373-rust-rag':raise RuntimeError('Image provenance labels do not match the release')
    backup_path=backup()
    domain=(ROOT/'domain').read_text().strip()
    if not re.fullmatch(r'[a-z0-9.-]+',domain):raise RuntimeError('Invalid configured domain')
    new=f'ORDO_DOMAIN={domain}\nORDO_IMAGE={IMAGE}@{request["app_digest"]}\nORDO_EMBEDDING_IMAGE={IMAGE}@{request["embedding_digest"]}\n'
    if current:(ROOT/'.env.previous').write_text(current)
    (ROOT/'.env').write_text(new)
    try:
        compose('config','--quiet')
        compose('--profile','setup','run','--rm','--no-deps','model-download')
        compose('up','-d','--wait','--wait-timeout','90','embeddings')
        compose('run','--rm','--no-deps','app','import')
        compose('up','-d','--wait','--wait-timeout','90','app')
        # Public deployment gate: trusted TLS plus enforced authentication.
        run('curl','--fail','--silent','--show-error','--retry','8','--retry-delay','5','--retry-all-errors',f'https://{domain}/api/health',stdout=subprocess.DEVNULL)
        code=subprocess.check_output(['curl','--silent','--output','/dev/null','--write-out','%{http_code}',f'https://{domain}/api/conversations'],text=True)
        if code!='401':raise RuntimeError('Public authentication gate failed')
        (ROOT/'release.json').write_text(json.dumps(request,indent=2)+'\n')
        (ROOT/'last-backup.json').write_text(json.dumps({'backup':backup_path,'revision':revision})+'\n')
        print('Ordo deployment healthy:',revision)
    except Exception:
        if current:
            (ROOT/'.env').write_text(current);compose('up','-d','--wait','--wait-timeout','90','embeddings','app')
            print('Restored previous image configuration; inspect migration compatibility before data rollback',file=sys.stderr)
        else:compose('stop','app')
        raise
if __name__=='__main__':
    if os.geteuid()!=0 or len(sys.argv)!=1:raise SystemExit('Requires the dedicated root operator entry point without arguments')
    os.umask(0o077)
    with open('/run/lock/ordo-deploy.lock','w') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        if Path(sys.argv[0]).name=='ordo-backup':backup()
        else:release()
