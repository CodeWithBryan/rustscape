// Browser context lifecycle only; the renderer and game state remain in Rust.
(() => {
 const canvas=document.getElementById('glcanvas');
 // The drawing buffer is opaque. Blended draws (floor blends, interface
 // sprites, text, translucent panels) leave destination alpha below one, and
 // a canvas with an alpha channel is composited over the page with it, so
 // the page would show through those pixels. No pass reads destination
 // alpha. The loader's own getContext('webgl') afterwards returns this
 // context, whatever attributes it asks for.
 const gl=canvas.getContext('webgl',{alpha:false});
 // What the world shader needs beyond WebGL 1's guaranteed minimum: three
 // samplers in the vertex stage (the light grid, the light data and the
 // material tables; the minimum is none) and textures 4096 pixels wide (the
 // texture atlas and its mip atlas; the minimum is 64). The start page
 // refuses to launch without them and says so; the client itself stops
 // during start-up when the world shader cannot be made.
 const lacking=window.rs09GraphicsLacking=[];
 if(!gl)lacking.push('WebGL');
 else{
  if(gl.getParameter(gl.MAX_VERTEX_TEXTURE_IMAGE_UNITS)<3)lacking.push('three texture units in the vertex shader');
  if(gl.getParameter(gl.MAX_TEXTURE_SIZE)<4096)lacking.push('textures 4096 pixels wide');
 }
 // Context loss invalidates every renderer resource. Stop pending host frames
 // before the next Rust draw; a reload creates a new renderer and connection.
 //
 // A page that is not painted gets no animation frames: a hidden tab, and
 // also a tab or window the browser has put in the background while it still
 // reports itself visible. The client's 20 ms cycles, with the keep-alive
 // (93) after 50 silent cycles and each cycle's write, run inside its
 // frames. The original's GameShell.run is a thread of its own that goes on
 // whether the applet is seen or not (GameShell.java:617-649), so the
 // server, which drops a connection silent for 20 s, never notices. A frame
 // asked for is therefore also run by a timer when the browser has not run
 // it within 100 ms (at once while the page is hidden): a dedicated
 // worker's 20 ms timer, which browsers do not throttle as they throttle a
 // background page's own timers, or, where no worker can be made, the
 // page's own timer (at most once a second in the background; the client
 // then runs at most ten cycles a frame, so it still writes a keep-alive
 // every six seconds). Each such frame is a whole one: its cycles and its
 // redraw, into the canvas nobody sees. At the display's rate the browser's
 // frame always comes first and the timer runs nothing.
 const requestFrame=window.requestAnimationFrame.bind(window);
 const cancelFrame=window.cancelAnimationFrame.bind(window);
 // The frames asked for and not yet run: our id -> the callback, the
 // browser's own frame request and when it was asked for.
 const frames=new Map();let lost=false,counter=0,timer=null,timerFrames=0;
 const late=100;
 const tick=()=>{
  if(lost)return;
  const now=performance.now();
  // A frame asks for the next one as it runs; that one waits for a later tick.
  for(const [id,entry] of [...frames]){
   if(!document.hidden&&now-entry.asked<late)continue;
   if(lost||!frames.delete(id))continue;
   cancelFrame(entry.native);
   if(++timerFrames%250===1)publish();
   entry.callback(now);
  }
 };
 const wake=()=>{
  if(timer)return;
  try{
   const worker=new Worker(URL.createObjectURL(new Blob(['setInterval(()=>postMessage(0),20);'],{type:'text/javascript'})));
   worker.onmessage=tick;timer={stop:()=>worker.terminate()};
  }catch(_){const interval=setInterval(tick,20);timer={stop:()=>clearInterval(interval)};}
 };
 const sleep=()=>{if(timer){timer.stop();timer=null;}};
 window.requestAnimationFrame=callback=>{
  if(lost)return 0;
  const id=++counter,entry={callback,native:0,asked:performance.now()};frames.set(id,entry);
  entry.native=requestFrame(time=>{if(frames.delete(id)&&!lost)callback(time);});
  wake();
  return id;
 };
 window.cancelAnimationFrame=id=>{const entry=frames.get(id);if(!entry)return;frames.delete(id);cancelFrame(entry.native);};
 document.addEventListener('visibilitychange',()=>{
  if(lost)return;
  events.push({type:document.hidden?'hidden':'visible',time:performance.now(),timerFrames});publish();
 });
 const style=document.createElement('style');
 style.textContent='#graphics-interruption{position:fixed;inset:0;z-index:10000;display:grid;place-content:center;gap:18px;text-align:center;background:#151a1ef5;color:#e9ce96;font:16px system-ui}#graphics-interruption[hidden]{display:none}#graphics-interruption h2{margin:0;font-size:26px}#graphics-interruption p{margin:0;color:#aebcc8}';
 document.head.append(style);
 const panel=document.createElement('section');panel.id='graphics-interruption';panel.hidden=true;
 panel.setAttribute('role','alertdialog');panel.setAttribute('aria-labelledby','graphics-interruption-title');
 panel.setAttribute('aria-describedby','graphics-interruption-description');
 const title=document.createElement('h2');title.id='graphics-interruption-title';title.textContent='Graphics interrupted';
 const description=document.createElement('p');description.id='graphics-interruption-description';description.textContent='Reload the client to reconnect.';
 const reload=document.createElement('button');reload.type='button';reload.textContent='Reload client';reload.addEventListener('click',()=>location.reload());
 panel.append(title,description,reload);document.body.append(panel);
 const proof=document.createElement('output');proof.id='graphics-proof';proof.hidden=true;document.body.append(proof);
 const events=[];
 function publish(){proof.dataset.state=JSON.stringify({lost,events,timerFrames});}publish();
 const interrupted=event=>{
  lost=true;sleep();for(const entry of frames.values())cancelFrame(entry.native);frames.clear();
  events.push({type:event.type,time:performance.now(),message:event.statusMessage??''});publish();
  panel.hidden=false;canvas.inert=true;canvas.blur();reload.focus();
 };
 canvas.addEventListener('webglcontextlost',interrupted);
 // Existing GPU resources need a fresh Rust renderer even if a context returns.
 canvas.addEventListener('webglcontextrestored',interrupted);
 for(const name of ['keydown','keyup','keypress'])document.addEventListener(name,event=>{
  if(!lost)return;
  event.stopImmediatePropagation();
  if(!['Enter',' ','Tab'].includes(event.key))event.preventDefault();
 },true);
})();
