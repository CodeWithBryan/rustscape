// Display glue only: the screen's size and the browser's Fullscreen API,
// standing in for the original's full-screen frame (DisplayMode,
// SignLink.enterFullScreen), and the browser's text for the world map's
// labels (AWT's Font there). Which mode, when, what happens when full screen
// ends and what is done with the glyphs are decided in Rust (display.rs,
// display_host.rs, world_map/font.rs).
(() => {
 const canvas=document.getElementById('glcanvas');
 // wanted: the client asked for full screen and has not left it.
 // left: the browser ended it, or refused it, without the client asking.
 let wanted=false,left=false,refusals=0,entries=0;
 const proof=document.createElement('output');proof.id='display-proof';proof.hidden=true;document.body.append(proof);
 const publish=()=>{proof.dataset.state=JSON.stringify({wanted,left,refusals,entries,
  full:document.fullscreenElement===canvas,screen:[screen.width,screen.height]});};
 publish();
 document.addEventListener('fullscreenchange',()=>{
  if(document.fullscreenElement===canvas)entries++;
  else if(wanted){wanted=false;left=true;}
  publish();
 });
 miniquad_add_plugin({name:'rs09_display',version:1,register_plugin(imports) {
  const env=imports.env;
  // The screen in CSS pixels, the unit of the page and of the canvas.
  env.rs09_screen_width=()=>screen.width|0;
  env.rs09_screen_height=()=>screen.height|0;
  env.rs09_full_screen=on=>{
   if(on){
    wanted=true;left=false;
    // Browsers grant full screen only shortly after a click or key press
    // (the options' button, the ::wm3 line); otherwise it is refused.
    const refuse=()=>{if(wanted){wanted=false;left=true;refusals++;publish();}};
    try{
     const request=canvas.requestFullscreen?canvas.requestFullscreen({navigationUI:'hide'}):null;
     if(!request)refuse();else request.catch(refuse);
    }catch(_){refuse();}
   } else {
    wanted=false;left=false;
    if(document.fullscreenElement)document.exitFullscreen().catch(()=>{});
   }
   publish();
  };
  // Once per change: full screen ended without the client asking.
  env.rs09_full_screen_left=()=>{const was=left;left=false;if(was)publish();return was?1:0;};
  // The world map's label face (world_map_render.rs): the original asks AWT
  // for "Helvetica" and draws each glyph on an off-screen image; here the
  // browser draws it on an off-screen 2D canvas, in the same family with
  // its own sans-serif behind it. Coverage is read from the red channel of
  // white text on black.
  const glyphs=document.createElement('canvas'),pen=glyphs.getContext('2d',{willReadFrequently:true});
  const face=(bold,size)=>`${bold?'bold ':''}${size}px Helvetica, Arial, sans-serif`;
  env.rs09_font_metric=(bold,size,what,ch)=>{
   pen.font=face(bold,size);
   const m=pen.measureText(what===0?String.fromCharCode(ch):'A');
   return what===0?m.width:what===1?m.fontBoundingBoxAscent:m.fontBoundingBoxDescent;
  };
  env.rs09_font_draw=(bold,size,ch,x,baseline,width,height,ptr)=>{
   if(width<=0||height<=0||width*height>1<<20)return -1;
   glyphs.width=width;glyphs.height=height;
   pen.fillStyle='#000';pen.fillRect(0,0,width,height);
   pen.font=face(bold,size);pen.textBaseline='alphabetic';pen.textAlign='left';
   pen.fillStyle='#fff';pen.fillText(String.fromCharCode(ch),x,baseline);
   const rgba=pen.getImageData(0,0,width,height).data,out=new Uint8Array(wasm_memory.buffer,ptr,width*height);
   for(let i=0;i<out.length;i++)out[i]=rgba[i*4];
   return 0;
  };
 }});
})();
