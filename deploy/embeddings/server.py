"""Private, single-worker CPU embedding service; pinned artifacts live outside the image."""
import hashlib
import json
import os
from pathlib import Path
import sys
import urllib.request
from http.server import BaseHTTPRequestHandler, HTTPServer

MODEL='BAAI/bge-small-en-v1.5'
REVISION='aa8f8b060edb00e03bfdd08813a2949946c8ba55'
REPO='Qdrant/bge-small-en-v1.5-onnx-Q'
ROOT=Path(os.environ.get('MODEL_DIR','/models/bge-small'))
LOCK=json.loads(Path(__file__).with_name('model-lock.json').read_text())
def digest(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
def validate():
    for name,expected in LOCK['files'].items():
        path=ROOT/name
        if not path.is_file() or digest(path)!=expected:raise RuntimeError('Missing or changed model artifact: '+name+'. Run download first.')
if len(sys.argv)>1 and sys.argv[1]=='download':
    ROOT.mkdir(parents=True,exist_ok=True)
    for name,expected in LOCK['files'].items():
        path=ROOT/name
        if path.exists() and digest(path)==expected:continue
        temp=path.with_suffix(path.suffix+'.download')
        with urllib.request.urlopen(f'https://huggingface.co/{REPO}/resolve/{REVISION}/{name}',timeout=120) as response,temp.open('wb') as output:
            while block:=response.read(1024*1024):output.write(block)
        if digest(temp)!=expected:temp.unlink();raise RuntimeError('Model checksum mismatch')
        temp.replace(path)
    validate();print('Verified pinned BGE model and tokenizer artifacts');sys.exit(0)
validate()
from fastembed import TextEmbedding
model=TextEmbedding(model_name=MODEL,specific_model_path=str(ROOT),local_files_only=True,threads=1,providers=['CPUExecutionProvider'],enable_cpu_mem_arena=False)
identity={'provider':'fastembed','model':MODEL,'revision':'fastembed-0.9.1:'+hashlib.sha256(json.dumps(LOCK,sort_keys=True).encode()).hexdigest()}
class Handler(BaseHTTPRequestHandler):
    server_version='OrdoEmbedding/1'
    def setup(self):
        super().setup();self.connection.settimeout(10)
    def log_message(self,*args):pass
    def respond(self,status,value):
        body=json.dumps(value,allow_nan=False).encode();self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def do_GET(self):
        if self.path not in ('/identity','/health'):self.respond(404,{'error':'Not found'});return
        self.respond(200,identity)
    def do_POST(self):
        if self.path!='/embed':self.respond(404,{'error':'Not found'});return
        try:
            length=int(self.headers.get('Content-Length','0'))
            if not 0<length<=140000:raise ValueError()
            value=json.loads(self.rfile.read(length));texts=value['texts'];query=value['query']
            if type(query) is not bool or not isinstance(texts,list) or not 1<=len(texts)<=16 or any(not isinstance(t,str) or not t or len(t.encode())>12000 for t in texts):raise ValueError()
        except (KeyError,ValueError,TypeError):self.respond(400,{'error':'Invalid embedding request'});return
        try:
            method=model.query_embed if query else model.passage_embed
            vectors=[vector.tolist() for vector in method(texts,batch_size=1)]
            self.respond(200,{'embeddings':vectors})
        except Exception:self.respond(503,{'error':'Embedding inference failed'})
if __name__=='__main__':
    # Single request at a time bounds CPU and memory on the small shared host.
    HTTPServer((os.environ.get('BIND_ADDRESS','0.0.0.0'),int(os.environ.get('PORT','8080'))),Handler).serve_forever()
