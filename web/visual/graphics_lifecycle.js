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
 const requestFrame=window.requestAnimationFrame.bind(window);
 const cancelFrame=window.cancelAnimationFrame.bind(window);
 const frames=new Set();let lost=false;
 window.requestAnimationFrame=callback=>{
  if(lost)return 0;
  const id=requestFrame(time=>{frames.delete(id);if(!lost)callback(time);});
  frames.add(id);return id;
 };
 window.cancelAnimationFrame=id=>{frames.delete(id);cancelFrame(id);};
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
 const publish=()=>{proof.dataset.state=JSON.stringify({lost,events});};publish();
 const interrupted=event=>{
  lost=true;for(const id of frames)cancelFrame(id);frames.clear();
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
