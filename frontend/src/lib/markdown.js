import { marked } from 'marked';
import DOMPurify from 'dompurify';
import hljs from 'highlight.js/lib/core';
import javascript from 'highlight.js/lib/languages/javascript';
import python from 'highlight.js/lib/languages/python';
import rust from 'highlight.js/lib/languages/rust';
import json from 'highlight.js/lib/languages/json';
import bash from 'highlight.js/lib/languages/bash';
import css from 'highlight.js/lib/languages/css';
import sql from 'highlight.js/lib/languages/sql';
import typescript from 'highlight.js/lib/languages/typescript';
import xml from 'highlight.js/lib/languages/xml';
for(const [name,language] of Object.entries({javascript,python,rust,json,bash,css,sql,typescript,xml})) hljs.registerLanguage(name,language);
DOMPurify.addHook('afterSanitizeAttributes',node=>{if(node.tagName==='A'){node.setAttribute('target','_blank');node.setAttribute('rel','noopener noreferrer');}});
const aliases={js:'javascript',ts:'typescript',py:'python',sh:'bash',html:'xml'};
export const tokens = text => marked.lexer(text || '', {gfm:true,breaks:false});
export function html(token) {
  return DOMPurify.sanitize(marked.parser([token]), {
    ALLOWED_TAGS:['p','br','strong','em','del','code','pre','blockquote','ul','ol','li','h1','h2','h3','h4','h5','h6','hr','table','thead','tbody','tr','th','td','a'],
    ALLOWED_ATTR:['href','title','start','align'], ALLOW_DATA_ATTR:false,
    ALLOWED_URI_REGEXP:/^(?:(?:https?|mailto):|#)/i
  });
}
export function highlighted(text,language) {
  const lang=aliases[language]||language;
  return hljs.getLanguage(lang) ? hljs.highlight(text,{language:lang,ignoreIllegals:true}).value : text.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}
