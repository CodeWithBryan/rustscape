// Device bridge only. Voice lifetime, looping, gain and clipping run in Rust.
class Rs09PcmProcessor extends AudioWorkletProcessor {
 constructor(options) {
  super();
  if(sampleRate!==22050)throw new Error('Original PCM requires a22050Hz audio context');
  this.wasm=new WebAssembly.Instance(options.processorOptions.module,{}).exports;
  this.block=new Int16Array(512);this.readback=new Int16Array(512);this.cursor=256;
  this.review=!!options.processorOptions.review;this.captured=false;this.captureFrames=0;
  this.targetFrames=Math.ceil(22050*8/256)*256;this.blocks=0;this.nonzero=0;
  this.expected=null;this.gaps=[];this.quanta=0;this.failed=false;
  // Output readback for review: sum of squares and peak of what was played.
  this.music=false;this.energy=0;this.peak=0;
  this.port.onmessage=event=>this.command(event.data);
  this.port.postMessage({type:'ready'});
 }
 fail(error) {this.failed=true;this.port.postMessage({type:'error',error:String(error)});}
 status() {
  return {sample_rate:sampleRate,buffer_rate:22050,channels:2,source_block:256,
   rendered_frames:this.wasm.rs09_audio_frames(),active_voices:this.wasm.rs09_audio_active(),
   buffers:this.blocks,nonzero_samples:this.nonzero,quanta:this.quanta,
   quantum_gaps:this.gaps.slice(),context_frame:currentFrame,
   output_energy:this.energy,output_peak:this.peak,music:this.musicStatus()};
 }
 musicStatus() {
  return {played:this.wasm.rs09_audio_music_played(),queued:this.wasm.rs09_audio_music_queued(),
   gaps:this.wasm.rs09_audio_music_gaps(),nonzero_samples:this.wasm.rs09_audio_music_nonzero()};
 }
 command(c) {
  try {
   if(this.failed)return;
   const low=c.low>>>0,high=c.high>>>0;
   if(c.op==='play') {
    const pcm=new Uint8Array(c.pcm);
    if(!pcm.length||pcm.length>16*1024*1024)throw new Error('PCM source transfer bound');
    const ptr=this.wasm.rs09_audio_input(pcm.length);
    if(!ptr)throw new Error('PCM source allocation rejected');
    new Uint8Array(this.wasm.memory.buffer,ptr,pcm.length).set(pcm);
    if(!this.wasm.rs09_audio_play(low,high,pcm.length,c.start,c.end,c.loops,c.volume))throw new Error('PCM voice rejected');
   } else if(c.op==='volume')this.wasm.rs09_audio_volume(low,high,c.volume);
   else if(c.op==='stop')this.wasm.rs09_audio_stop(low,high);
   else if(c.op==='music') {
    // Blocks of the music line's mix, read by the client between cycles.
    const pcm=new Int32Array(c.pcm),blocks=pcm.length/512;
    if(!Number.isInteger(blocks)||!blocks||blocks>64)throw new Error('Music block transfer bound');
    const ptr=this.wasm.rs09_audio_music_input(blocks);
    if(!ptr)throw new Error('Music block allocation rejected');
    new Int32Array(this.wasm.memory.buffer,ptr,pcm.length).set(pcm);
    this.wasm.rs09_audio_music_push(blocks);this.music=true;return;
   }
   else throw new Error('Unknown PCM command');
   if(this.review&&!this.captured)this.port.postMessage({type:'applied',op:c.op,low,high,
    frame:this.wasm.rs09_audio_frames(),start:c.start,end:c.end,loops:c.loops,volume:c.volume});
  } catch(e) {this.fail(e);}
 }
 process(_inputs,outputs) {
  try {
   if(this.failed)return false;
   const channels=outputs[0];
   if(channels.length!==2||channels[0].length!==channels[1].length)throw new Error('Stereo output required');
   const [left,right]=channels,quantum=left.length;
   if(this.expected!==null&&currentFrame!==this.expected&&this.gaps.length<16)
    this.gaps.push({expected:this.expected,actual:currentFrame});
   this.expected=currentFrame+quantum;this.quanta++;
   if(!this.wasm.rs09_audio_started()){left.fill(0);right.fill(0);return true;}
   for(let i=0;i<quantum;i++) {
    if(this.cursor===256) {
     const ptr=this.wasm.rs09_audio_read();
     this.block.set(new Int16Array(this.wasm.memory.buffer,ptr,512));this.cursor=0;this.blocks++;
     const count=this.wasm.rs09_audio_completed_count();
     if(count) {
      const ptr=this.wasm.rs09_audio_completed();
      this.port.postMessage({type:'ended',ids:Array.from(new Uint32Array(this.wasm.memory.buffer,ptr,count*2))});
     }
     // The client reads ahead by what has been played.
     if(this.music)this.port.postMessage({type:'music',...this.musicStatus()});
    }
    const p=this.cursor*2;
    left[i]=this.block[p]/32768;right[i]=this.block[p+1]/32768;
    this.nonzero+=Number(left[i]!==0)+Number(right[i]!==0);
    this.energy+=left[i]*left[i]+right[i]*right[i];
    this.peak=Math.max(this.peak,Math.abs(left[i]),Math.abs(right[i]));
    if(this.review&&!this.captured) {
     // Read back the actual output channel arrays, after Float32 assignment.
     this.readback[p]=Math.round(left[i]*32768);this.readback[p+1]=Math.round(right[i]*32768);
    }
    this.cursor++;
    if(this.cursor===256&&this.review&&!this.captured) {
     const pcm=this.block.slice(),readback=this.readback.slice();this.captureFrames+=256;
     this.port.postMessage({type:'pcm',frames:this.captureFrames,pcm:pcm.buffer,readback:readback.buffer},[pcm.buffer,readback.buffer]);
     if(this.captureFrames===this.targetFrames) {
      this.captured=true;this.port.postMessage({type:'capture_done',frames:this.captureFrames,device:this.status()});
     }
    }
   }
   if(this.blocks%64===0)this.port.postMessage({type:'status',device:this.status()});
   return true;
  } catch(e) {this.fail(e);return false;}
 }
}
registerProcessor('rs09-pcm',Rs09PcmProcessor);
