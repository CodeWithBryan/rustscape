// Platform transport only. The dedicated audio thread executes the Rust mixer.
(() => {
 const rate=22050,review=new URLSearchParams(location.search).get('audio-review')==='1';
 const script=new URL(document.currentScript.src),version=script.search;
 const asset=name=>new URL(name+version,script).href;
 const voices=new Set(),rawSources=new Map();
 let context=null,node=null,ready=false,loading=false,error=null,pending=[],pendingBytes=0;
 let firstFlush=true,flushScheduled=false,last={},captured=false,sourceBytes=0,complete=true;
 // Music blocks handed to the audio thread, and what it reports back.
 let musicSent=0,music={played:0,queued:0,gaps:0,nonzero_samples:0};
 const capture={commands:{frames:0,rate,channels:2,commands:[]},pcm:[],submitted_pcm:[]};
 const key=(low,high)=>`${high>>>0}:${low>>>0}`;
 function fail(e){error=String(e);voices.clear();}
 function publicStatus() {
  return {state:context?.state??'uninitialized',ready,backend:'AudioWorklet/Rust PCM',
   sample_rate:context?.sampleRate??null,buffer_rate:rate,channels:2,
   main_live_handles:voices.size,queued_commands:pending.length,error,review,...last,
   music:{sent:musicSent,...music}};
 }
 // Review hook: the device's own counters, readable from the page.
 window.rs09AudioStatus=publicStatus;
 function publishProof(device) {
  if(!review||captured)return;
  captured=true;capture.complete=complete;capture.readback_complete=complete;
  capture.device={...device,state:context.state,error,sample_rate:context.sampleRate,
   buffer_rate:rate,channels:2,readback_frames:capture.submitted_pcm.length/2};
  const out=document.createElement('output');out.id='audio-proof';out.hidden=true;
  out.dataset.capture=JSON.stringify(capture);document.body.append(out);rawSources.clear();
 }
 function receive(m) {
  if(m.type==='ready'){ready=true;flush();}
  else if(m.type==='error')fail(m.error);
  else if(m.type==='ended')for(let i=0;i<m.ids.length;i+=2)voices.delete(key(m.ids[i],m.ids[i+1]));
  else if(m.type==='status')last=m.device;
  else if(m.type==='music')music={played:m.played,queued:m.queued,gaps:m.gaps,nonzero_samples:m.nonzero_samples};
  else if(m.type==='applied'&&review&&!captured) {
   if(capture.commands.commands.length>=8192){complete=false;return;}
   const id=m.low+m.high*4294967296,c={op:m.op,id,frame:m.frame};
   if(m.op==='play') {
    const samples=rawSources.get(key(m.low,m.high));
    if(!samples){complete=false;return;}
    Object.assign(c,{samples,start:m.start,end:m.end,loops:m.loops,volume:m.volume});
   } else if(m.op==='volume')c.volume=m.volume;
   capture.commands.commands.push(c);
  } else if(m.type==='pcm'&&review&&!captured) {
   const pcm=new Int16Array(m.pcm),readback=new Int16Array(m.readback);
   if(m.frames!==capture.commands.frames+256||pcm.length!==512||readback.length!==512||m.frames>rate*30){complete=false;return;}
   capture.pcm.push(...pcm);capture.submitted_pcm.push(...readback);capture.commands.frames=m.frames;
  } else if(m.type==='capture_done'){last=m.device;publishProof(m.device);}
 }
 function flush() {
  if(!ready||!node||error||!pending.length)return;
  flushScheduled=false;firstFlush=false;
  for(const c of pending)node.port.postMessage(c,c.pcm?[c.pcm]:[]);
  pending=[];pendingBytes=0;
 }
 function send(c,size=0) {
  if(error||pending.length>=4096||pendingBytes+size>32*1024*1024)return -1;
  pending.push(c);pendingBytes+=size;
  if(firstFlush&&!flushScheduled) {
   // Queue the initial commands after the first game/render callback returns.
   flushScheduled=true;queueMicrotask(flush);
  } else if(!firstFlush)flush();
  return 0;
 }
 function open() {
  if(error)return -1;
  try {
   if(!context)context=new AudioContext({sampleRate:rate,latencyHint:'interactive'});
   if(context.sampleRate!==rate)throw new Error('Audio device context rejected22050Hz');
   if(!loading) {
    loading=true;
    (async()=>{
     const response=await fetch(asset('audio_worklet.wasm'));
     if(!response.ok)throw new Error(`Audio WASM request failed: ${response.status}`);
     const module=await WebAssembly.compile(await response.arrayBuffer());
     await context.audioWorklet.addModule(asset('pcm_worklet.js'));
     node=new AudioWorkletNode(context,'rs09-pcm',{numberOfInputs:0,numberOfOutputs:1,
      outputChannelCount:[2],processorOptions:{module,review}});
     node.port.onmessage=e=>receive(e.data);node.onprocessorerror=()=>fail('Rust audio processor failed');
     node.connect(context.destination);
    })().catch(fail);
   }
   return 0;
  } catch(e){fail(e);return -1;}
 }
 function unlock(){if(open()===0&&context.state==='suspended')context.resume().catch(fail);}
 document.getElementById('launch')?.addEventListener('click',unlock);
 document.getElementById('glcanvas')?.addEventListener('pointerdown',unlock);
 document.getElementById('glcanvas')?.addEventListener('keydown',unlock);
 miniquad_add_plugin({name:'rs09_pcm_audio',version:2,register_plugin(imports) {
  const env=imports.env;
  env.rs09_pcm_open=open;
  env.rs09_pcm_record=()=>review?1:0;
  env.rs09_pcm_play=(low,high,ptr,len,start,end,loops,volume)=>{
   if(!len||len>16*1024*1024||error)return -1;
   const pcm=new Int8Array(wasm_memory.buffer,ptr,len).slice(),id=key(low,high);
   if(review&&!captured) {
    if(sourceBytes+len>4*1024*1024)complete=false;
    else{rawSources.set(id,Array.from(pcm));sourceBytes+=len;}
   }
   if(send({op:'play',low,high,pcm:pcm.buffer,start,end,loops,volume},len)<0)return -1;
   voices.add(id);return 0;
  };
  env.rs09_pcm_volume=(low,high,volume)=>send({op:'volume',low,high,volume});
  env.rs09_pcm_stop=(low,high)=>{voices.delete(key(low,high));return send({op:'stop',low,high});};
  env.rs09_pcm_linked=(low,high)=>Number(voices.has(key(low,high))&&!error);
  env.rs09_pcm_status=(ptr,capacity)=>{
   const text=JSON.stringify(publicStatus()),bytes=new TextEncoder().encode(text);
   if(bytes.length>capacity)return -1;
   new Uint8Array(wasm_memory.buffer,ptr,bytes.length).set(bytes);return bytes.length;
  };
  // Music: the client reads the sequencer and passes whole 256-frame blocks
  // (signed 24-bit mix in 32-bit words); the audio thread clips and adds them.
  env.rs09_pcm_music=(ptr,blocks)=>{
   if(!blocks||blocks>64||error)return -1;
   const pcm=new Int32Array(wasm_memory.buffer,ptr,blocks*512).slice();
   if(send({op:'music',pcm:pcm.buffer},pcm.byteLength)<0)return -1;
   musicSent+=blocks;return 0;
  };
  env.rs09_pcm_music_sent=()=>error?-1:musicSent;
  env.rs09_pcm_music_played=()=>error?-1:music.played;
  env.rs09_pcm_close=()=>{
   voices.clear();pending=[];pendingBytes=0;node?.disconnect();node?.port.close();
   context?.close().catch(()=>{});node=null;ready=false;context=null;
   musicSent=0;music={played:0,queued:0,gaps:0,nonzero_samples:0};
  };
 }});
})();
