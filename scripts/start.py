#!/usr/bin/env python3
"""Load private setup configuration and start native Rust or Docker Compose."""
import os
import subprocess
import sys
from config import ROOT, CONFIG, load

def main():
    cfg=load()
    if not cfg: raise SystemExit('Run python3 scripts/setup.py first, or use cargo directly with exported environment variables.')
    env=os.environ.copy(); env.update(cfg)
    if cfg.get('ORDO_RUNTIME')=='docker':
        command=['docker','compose','--env-file',str(CONFIG),'up','-d']
    else: command=['cargo','run','--locked','--','serve']
    print('Open http://127.0.0.1:'+cfg.get('PORT','3730'),flush=True)
    raise SystemExit(subprocess.call(command,cwd=ROOT,env=env))
if __name__=='__main__':main()
