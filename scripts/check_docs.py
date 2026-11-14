"""Check repository-relative Markdown links without making network requests."""
from pathlib import Path
import re
import sys
root=Path(__file__).resolve().parents[1]
files=list(root.glob('*.md'))+list((root/'docs').rglob('*.md'))
failures=[]
for path in files:
    text=re.sub(r'```.*?```','',path.read_text(),flags=re.S)
    for raw in re.findall(r'\]\(([^)\n]+)\)',text):
        target=raw.strip().strip('<>').split('#')[0]
        if not target or re.match(r'[a-zA-Z][\w+.-]*:',target):continue
        if not (path.parent/target).resolve().exists():failures.append(f'{path.relative_to(root)} -> {target}')
if failures:
    print('\n'.join(failures));sys.exit(1)
print(f'PASS: relative links in {len(files)} Markdown documents')
