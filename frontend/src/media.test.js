import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mediaLabel,mediaHint} from './lib/media.js';
test('media labels never promise unsupported model capabilities',()=>{
  assert.equal(mediaLabel({mime:'image/png'},{}),'Vision unavailable');
  assert.equal(mediaLabel({mime:'audio/webm'},{audio_input:true}),'Transcribed on send');
  assert.equal(mediaLabel({mime:'application/pdf'},{image_input:true}),'PDF processing unavailable');
  assert.equal(mediaLabel({mime:'audio/wav',transcript:{text:'Hello'}}),'Transcript included');
  assert.match(mediaHint([{mime:'video/mp4'}],{}),/needs setup/);
});
test('processed document and video summaries describe the actual inputs',()=>{
  assert.equal(mediaLabel({mime:'application/pdf',document:{pages:[{},{}]}}),'2 pages included');
  assert.equal(mediaLabel({mime:'video/mp4',video:{frames:[{},{},{}]},transcript:{text:'speech'}}),'3 frames + transcript');
  assert.match(mediaHint([{mime:'video/mp4'}],{video_input:true}),/between frames may be missed/);
});
