#!/usr/bin/env python3
"""Student onboarding: choose local/Groq, install models, build and import."""
import argparse
import getpass
import json
import os
import platform
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
from config import ROOT, CONFIG, load, save

def run(command, env):
    print('+ ' + ' '.join(command), flush=True)
    subprocess.run(command, cwd=ROOT, env=env, check=True)

def require(names):
    missing = [name for name in names if not shutil.which(name)]
    if missing: raise RuntimeError('Install these prerequisites first: ' + ', '.join(missing) + '. See docs/course/setup.md')

def choose(prompt, options, default):
    answer = input(prompt + ' [' + '/'.join(options) + '] (' + default + '): ').strip().lower() or default
    if answer not in options: raise RuntimeError('Choose one of: ' + ', '.join(options))
    return answer

def verify_groq(key, model):
    req = urllib.request.Request('https://api.groq.com/openai/v1/models',headers={'Authorization':'Bearer '+key,'User-Agent':'is373-rust-rag/0.1','Accept':'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=15) as response: available = json.load(response)
    except urllib.error.HTTPError as exc:
        raise RuntimeError(f'Groq rejected the model-access check (HTTP {exc.code}); verify your key and account.') from None
    if not any(item['id']==model for item in available.get('data', [])):
        raise RuntimeError('The selected model is not available to this Groq account.')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--provider', choices=['local','groq'])
    parser.add_argument('--runtime', choices=['native','docker'])
    parser.add_argument('--skip-media', action='store_true', help='Skip Whisper and optional local vision downloads; PDF text can still work')
    parser.add_argument('--container-ollama', action='store_true', help='Use Compose Ollama instead of native host Ollama')
    parser.add_argument('--configure-only', action='store_true', help='Save configuration without downloading, building or calling providers')
    parser.add_argument('--build-image', action='store_true', help='Build the Docker image from source instead of pulling the published image')
    parser.add_argument('--yes', action='store_true', help='Accept downloads/build; provider/runtime must be supplied')
    args=parser.parse_args()
    if args.yes and (not args.provider or not args.runtime): parser.error('--yes requires --provider and --runtime')
    provider=args.provider or choose('Answer provider: local models or Groq text API', ['local','groq'], 'local')
    runtime=args.runtime or choose('Run Rust natively or use Docker', ['native','docker'], 'native' if platform.system()=='Darwin' else 'docker')
    old=load()
    cfg={'CHAT_PROVIDER':'groq' if provider=='groq' else 'ollama',
         'CHAT_MODEL':'openai/gpt-oss-20b' if provider=='groq' else 'gemma3:4b',
         'EMBEDDING_PROVIDER':'ollama','EMBEDDING_MODEL':'nomic-embed-text:latest',
         'VISION_PROVIDER':'ollama','VISION_MODEL':'gemma3:4b',
         'WHISPER_VARIANT':'small','WHISPER_MODEL':'data/models/ggml-small.bin',
         'WHISPER_LANGUAGE':'auto','VIDEO_SAMPLE_FRAMES':'3','PORT':'3730',
         'ORDO_RUNTIME':runtime,'ORDO_IMAGE':'kaw393939/is373-rust-rag:latest'}
    if provider=='groq':
        key=os.environ.get('GROQ_API_KEY') or old.get('GROQ_API_KEY')
        key_file=os.environ.get('GROQ_API_KEY_FILE') or old.get('GROQ_API_KEY_FILE')
        if not key and key_file:
            from pathlib import Path
            key=Path(key_file).read_text().strip()
        if not key and not args.yes: key=getpass.getpass('Groq API key (hidden; stored only in ignored .env.local): ').strip()
        if not key or not key.startswith('gsk_') or not all(c.isalnum() or c in '_-' for c in key):
            raise RuntimeError('Provide a valid Groq key through the hidden prompt or GROQ_API_KEY environment variable.')
        cfg['GROQ_API_KEY']=key
        if not args.configure_only: verify_groq(key,cfg['CHAT_MODEL'])
    container_ollama=runtime=='docker' and (args.container_ollama or platform.system()!='Darwin' or not shutil.which('ollama'))
    cfg['OLLAMA_URL']='http://ollama:11434' if container_ollama else 'http://host.docker.internal:11434' if runtime=='docker' else 'http://127.0.0.1:11434'
    cfg['COMPOSE_PROFILES']='local-models' if container_ollama else ''
    save(cfg)
    print(f'Configuration saved privately to {CONFIG.name}. Provider: {provider}; runtime: {runtime}.')
    if args.configure_only: return
    if not args.yes and choose('Download models and build/import now? Full media uses about 4.2 GB of model files', ['y','n'],'y')=='n': return
    env=os.environ.copy(); env.update(cfg)
    compose=['docker','compose','--env-file',str(CONFIG)]
    if runtime=='docker':
        require(['docker'])
        run(compose+(['build','app'] if args.build_image else ['pull','app']),env)
        if container_ollama: run(compose+['up','-d','--wait','ollama'],env)
    else:
        require(['cargo','npm','ollama'])
        if not args.skip_media: require(['whisper-cli','ffmpeg','ffprobe','pdfinfo','pdftotext','pdftoppm','curl'])
    ollama=compose+['exec','-T','ollama','ollama'] if container_ollama else ['ollama']
    if not container_ollama: require(['ollama'])
    run(ollama+['list'],env) # Fail clearly if the local model service is not running.
    run(ollama+['pull','nomic-embed-text:latest'],env)
    if provider=='local' or not args.skip_media: run(ollama+['pull','gemma3:4b'],env)
    if runtime=='docker':
        if not args.skip_media: run(compose+['run','--rm','app','setup-models'],env)
        run(compose+['run','--rm','app','import'],env)
    else:
        if not args.skip_media: run(['bash','scripts/download_whisper.sh'],env)
        run(['npm','ci','--prefix','frontend'],env)
        run(['npm','--prefix','frontend','run','build'],env)
        run(['cargo','run','--locked','--','import'],env)
    print('Ready. Start with: python3 scripts/start.py')

if __name__=='__main__':
    try: main()
    except (RuntimeError,ValueError,subprocess.CalledProcessError,urllib.error.URLError) as exc:
        print('Setup stopped: '+str(exc),file=sys.stderr); sys.exit(1)
