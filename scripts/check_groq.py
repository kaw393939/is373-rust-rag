"""Live Groq adapter check against a separately configured server. Never reads or logs a key."""
import json
import os
from pathlib import Path
import urllib.request
import check_documents_video as media

media.RESULTS_PATH=media.ROOT/'data/groq-events.json'
media.BASE=os.environ.get('GROQ_TEST_BASE','http://127.0.0.1:3732/api')
status=media.api('/status')
assert status['chat_provider']=='groq' and status['chat_available'] and not status['local_only']
assert status['vision_provider']=='ollama' and status['capabilities']['audio_local']
rows=[]
conversation=media.api('/conversations',{},'POST')['id']
try:
    answer,end,c=media.ask(conversation,'How much is the annual membership fee? Cite the source.',mode='vector')
    assert end['outcome']=='complete' and '$15' in answer and '[membership:p2]' in answer
    assert c['messages'][-1]['model']==status['chat_model']
    rows.append({'question':'Membership fee','answer':answer,'outcome':end['outcome']})
    doc=media.upload(media.ROOT/'tmp/pdfs/workshop.pdf')
    answer,end,c=media.ask(conversation,'What color notebook does the attached PDF ask me to bring? Cite its page.',[doc['id']],'vector')
    assert end['outcome']=='complete' and 'orange' in answer.lower() and f'[attachment-{doc["id"]}:p2]' in answer
    rows.append({'question':'PDF notebook','answer':answer,'outcome':end['outcome']})
finally:media.api('/conversations/'+conversation,{'archived':True},'PATCH')
Path('data/groq-results.json').write_text(json.dumps({'provider':'groq','model':status['chat_model'],'results':rows},indent=2)+'\n')
print('PASS: real Groq streaming, library grounding, local PDF extraction, citations and persisted responses')
