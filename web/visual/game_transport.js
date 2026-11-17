// Platform glue only. Login, packet framing and gameplay stay in Rust.
(() => {
 // Raw DOM units cross the platform boundary; normalization/accumulation is Rust.
 // Capture prevents the bundled raw-pixel handler from applying the event twice.
 document.getElementById('glcanvas').addEventListener('wheel',event=>{
  if(typeof wasm_exports==='undefined'||typeof wasm_exports.rs09_wheel_event!=='function')return;
  event.preventDefault();event.stopImmediatePropagation();
  wasm_exports.rs09_wheel_event(event.deltaY,event.deltaMode,Number.isFinite(event.wheelDeltaY)?event.wheelDeltaY:NaN);
 },{capture:true,passive:false});
 // The bundled handlers do not report the pointer leaving the canvas; Rust
 // then puts the pointer at (-1,-1) as the original's mouseExited does.
 document.getElementById('glcanvas').addEventListener('mouseleave',()=>{
  if(typeof wasm_exports!=='undefined'&&typeof wasm_exports.rs09_mouse_exit==='function')wasm_exports.rs09_mouse_exit();
 });
 // Three keys the bundled loader reports wrongly. Its key table sends 222
 // for the quote key and 189 for the slash key, numbers its Rust side does
 // not know (it reads 39 and 47), so both arrive as unknown keys and their
 // key codes are never held; and it sends 96 for the back-quote key, which
 // the Rust side reads as the quote key and has no other number for. They
 // are reported here instead: 39, 47, and for the back-quote key 336
 // (keypad equals, a key the original has no code for), which the client's
 // input translation takes as the back-quote key in a browser. A real
 // keypad-equals key is reported as unknown. As the loader does for the
 // keys it handles, the default action is cancelled (no quick find on / and
 // '), so the character is passed on here: no keypress event follows.
 const keyNumbers={Quote:39,Slash:47,Backquote:336,NumpadEqual:0};
 const keyModifiers=event=>(event.shiftKey?1:0)|(event.ctrlKey?2:0)|(event.altKey?4:0);
 const ownKey=event=>event.target===document.getElementById('glcanvas')&&Object.hasOwn(keyNumbers,event.code)
  &&typeof wasm_exports!=='undefined'&&typeof wasm_exports.key_down==='function';
 document.addEventListener('keydown',event=>{
  if(!ownKey(event))return;
  event.preventDefault();event.stopImmediatePropagation();
  wasm_exports.key_down(keyNumbers[event.code],keyModifiers(event),event.repeat);
  // The loader's rule for characters: none while Ctrl is held.
  if(!event.ctrlKey&&[...event.key].length===1)wasm_exports.key_press(event.key.codePointAt(0));
 },true);
 document.addEventListener('keyup',event=>{
  if(!ownKey(event))return;
  event.stopImmediatePropagation();
  wasm_exports.key_up(keyNumbers[event.code],keyModifiers(event));
 },true);
 let badgeTimer=0;
 const sockets = new Map();
 let next = 1;
 const limit = 1024 * 1024;
 // The update (JS5) connection keeps up to 40 requests in flight, each for a
 // group of up to a megabyte, and a page cannot slow the server down.
 const cacheLimit = 64 * 1024 * 1024;
 // Review readback for the update connection: counts only, no contents.
 const cacheWire = window.rs09CacheWire = {connections:0,open:0,messages:0,bytes:0,sent_messages:0,closed:0};
 // Diagnostic output only: do not retain URLs, credentials or packet contents.
 const proof=document.createElement('output');proof.id='transport-proof';proof.hidden=true;
 proof.dataset.state=JSON.stringify({phase:'uninitialized'});document.body.append(proof);
 function publishTransport(s,phase,event) {
  if(event)s.closed={code:event.code,reason:typeof event.reason==='string'?event.reason.slice(0,256):null,was_clean:event.wasClean};
  // The proof element describes the game connection only.
  if(s.cache)return;
  proof.dataset.state=JSON.stringify({handle:s.id,phase:s.closed?'closed':phase,status:s.status,
   queued_bytes:s.size,ingress_limit:limit,close_requested:s.closeRequested,
   code:s.closed?.code??null,reason:s.closed?.reason??null,was_clean:s.closed?.was_clean??null});
 }
 miniquad_add_plugin({name:'rs09_game_transport',version:1,register_plugin(imports) {
  const env=imports.env;
  env.rs09_launch_value = (index,ptr,capacity) => {
   const values=window.rs09Launch || ['', '', ''];
   const bytes=new TextEncoder().encode(values[index] || '');
   if(bytes.length>capacity) return -1;
   new Uint8Array(wasm_memory.buffer,ptr,bytes.length).set(bytes);
   values[index]=''; return bytes.length;
  };
  env.rs09_live_state = (ptr,len) => {
   try {
    const state=JSON.parse(new TextDecoder().decode(new Uint8Array(wasm_memory.buffer,ptr,len)));
    window.rs09ClientState=state;
    const badge=document.getElementById('connection');
    if(badge) {
     // The badge lies over the canvas corner where the game draws its hover
     // text, so it shows for a few seconds when the status changes and not
     // at all for an empty one (the title screen).
     const status=typeof state.status==='string'?state.status:'';
     if(badge.textContent!==status) {
      badge.textContent=status;badge.hidden=!status;
      clearTimeout(badgeTimer);badgeTimer=setTimeout(()=>{badge.hidden=true;},4000);
     }
     badge.dataset.state=JSON.stringify(state);
    }
   } catch (_) {}
  };
  env.rs09_ws_open_cache = (ptr,len) => env.rs09_ws_open(ptr,len,true);
  env.rs09_ws_open = (ptr,len,cache) => {
   let ws;
   try {
    const url = new TextDecoder().decode(new Uint8Array(wasm_memory.buffer,ptr,len));
    if (!/^wss?:\/\//.test(url)) return -1;
    ws = new WebSocket(url);
   } catch (_) { return -1; }
   ws.binaryType = 'arraybuffer';
   const id = next++, state = {id,ws,status:0,chunks:[],offset:0,size:0,closeRequested:null,closed:null,
    cache:cache===true,limit:cache===true?cacheLimit:limit};
   sockets.set(id,state);
   if(state.cache)cacheWire.connections++;
   publishTransport(state,'connecting');
   ws.onopen = () => { state.status = 1; if(state.cache)cacheWire.open++; publishTransport(state,'open'); };
   ws.onerror = () => { state.status = 3; publishTransport(state,'error'); };
   ws.onclose = event => { if (state.status !== 3) state.status = 2; if(state.cache)cacheWire.closed++; publishTransport(state,'closed',event); };
   ws.onmessage = event => {
    if (!(event.data instanceof ArrayBuffer)) {
     state.status=3;state.closeRequested={by:'binary-guard',code:1003};publishTransport(state,'closing');
     ws.close(1003,'binary packets required'); return;
    }
    if (state.size + event.data.byteLength > state.limit) {
     state.status=3;state.closeRequested={by:'ingress-guard',code:1009};publishTransport(state,'closing');
     ws.close(1009,'ingress limit'); return;
    }
    if (event.data.byteLength) {
     state.chunks.push(new Uint8Array(event.data)); state.size += event.data.byteLength;
     if(state.cache){cacheWire.messages++;cacheWire.bytes+=event.data.byteLength;}
    }
   };
   return id;
  };
  env.rs09_ws_status = id => sockets.get(id)?.status ?? 3;
  env.rs09_ws_read = (id,ptr,capacity) => {
   const s=sockets.get(id); if (!s || !s.size) return 0;
   const output=new Uint8Array(wasm_memory.buffer,ptr,capacity);
   let count=0;
   while (count < capacity && s.chunks.length) {
    const chunk=s.chunks[0], n=Math.min(chunk.length-s.offset,capacity-count);
    output.set(chunk.subarray(s.offset,s.offset+n),count);
    count+=n; s.offset+=n; s.size-=n;
    if (s.offset===chunk.length) { s.chunks.shift(); s.offset=0; }
   }
   return count;
  };
  env.rs09_ws_send = (id,ptr,len) => {
   const s=sockets.get(id);
   if (!s || s.status!==1 || len>65538 || s.ws.bufferedAmount+len>limit) return -1;
   try { s.ws.send(new Uint8Array(wasm_memory.buffer,ptr,len).slice()); if(s.cache)cacheWire.sent_messages++; return 0; }
   catch (_) { return -1; }
  };
  env.rs09_ws_close = id => {
   const s=sockets.get(id); if (!s) return;
   if(!s.closed)s.closeRequested??={by:s.status===3?'client-cleanup-after-error':'client'};
   publishTransport(s,'closing');
   s.ws.onmessage=null; s.chunks=[]; s.size=0; s.ws.close(); sockets.delete(id);
  };
 }});
})();
