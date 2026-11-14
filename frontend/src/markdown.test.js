import {test} from 'node:test';
import assert from 'node:assert/strict';
import {JSDOM} from 'jsdom';
const dom=new JSDOM('');globalThis.window=dom.window;globalThis.document=dom.window.document;
const {tokens,html,highlighted}=await import('./lib/markdown.js');
test('rich messages strip executable HTML, unsafe links and remote images',()=>{
  const input='<script>alert(1)</script>\n\n<img src="https://tracker.invalid/pixel" onerror="alert(1)">\n\n[bad](javascript:alert(1))\n\n[good](https://example.com)';
  const output=tokens(input).map(html).join('');
  assert.doesNotMatch(output,/<script|<img|onerror=|href="javascript:/i);
  assert.match(output,/href="https:\/\/example.com"/);assert.match(output,/noopener noreferrer/);
});
test('tables survive sanitization and code escapes HTML',()=>{
  assert.match(tokens('| Name | Value |\n|---|---|\n| A | 1 |').map(html).join(''),/<table>/);
  assert.equal(highlighted('<script>x</script>','unknown'),'&lt;script&gt;x&lt;/script&gt;');
  assert.match(highlighted('fn main() {}','rust'),/hljs/);
});
