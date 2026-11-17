// Storage glue only: named byte strings in IndexedDB, standing in for the
// cache directory the original client keeps on disk, and the preferences
// file in local storage. What is stored, when and how it is checked on the
// way back is decided in Rust (web_cache.rs, preferences_file.rs).
(() => {
 const STORE='files';
 // 0 not opened, 1 reading, 2 ready, 3 unavailable.
 let status=0,db=null,entries=new Map(),staged=[],writing=0,failed=false,commits=0,written=0;
 const text=(ptr,len)=>new TextDecoder().decode(new Uint8Array(wasm_memory.buffer,ptr,len));
 const bytes=value=>value instanceof ArrayBuffer?new Uint8Array(value):
  ArrayBuffer.isView(value)?new Uint8Array(value.buffer,value.byteOffset,value.byteLength):new Uint8Array(0);
 function open(name) {
  status=1;
  let request;
  try{request=indexedDB.open(name,1);}catch(_){status=3;return;}
  request.onupgradeneeded=()=>{request.result.createObjectStore(STORE);};
  request.onerror=request.onblocked=()=>{status=3;};
  request.onsuccess=()=>{
   db=request.result;
   // Another tab upgrading or deleting the database takes it away.
   db.onversionchange=()=>{db.close();db=null;failed=true;};
   try {
    const tx=db.transaction(STORE,'readonly'),store=tx.objectStore(STORE);
    const keys=store.getAllKeys(),values=store.getAll();
    tx.oncomplete=()=>{
     keys.result.forEach((key,i)=>entries.set(String(key),bytes(values.result[i])));
     status=2;
    };
    tx.onerror=tx.onabort=()=>{status=3;};
   } catch(_){status=3;}
  };
 }
 // Review readback: counts only.
 window.rs09CacheStorage=()=>({status,stored_keys:entries.size,commits,written_bytes:written,writing,failed});
 const proof=document.createElement('output');proof.id='cache-proof';proof.hidden=true;
 proof.dataset.state=JSON.stringify({phase:'not requested'});document.body.append(proof);
 miniquad_add_plugin({name:'rs09_cache_storage',version:1,register_plugin(imports) {
  const env=imports.env;
  env.rs09_store_open=(ptr,len)=>{if(status===0)open(text(ptr,len));};
  env.rs09_store_status=()=>status;
  env.rs09_store_count=()=>entries.size;
  env.rs09_store_key=(index,ptr,capacity)=>{
   const key=[...entries.keys()][index];
   if(key===undefined)return -1;
   const encoded=new TextEncoder().encode(key);
   if(encoded.length>capacity)return -1;
   new Uint8Array(wasm_memory.buffer,ptr,encoded.length).set(encoded);return encoded.length;
  };
  env.rs09_store_length=(ptr,len)=>entries.get(text(ptr,len))?.length??-1;
  env.rs09_store_read=(keyPtr,keyLen,ptr,len)=>{
   const value=entries.get(text(keyPtr,keyLen));
   if(!value||value.length<len)return -1;
   new Uint8Array(wasm_memory.buffer,ptr,len).set(value.subarray(0,len));return 0;
  };
  env.rs09_store_release=()=>{entries=new Map();};
  env.rs09_store_put=(keyPtr,keyLen,ptr,len)=>{
   staged.push([text(keyPtr,keyLen),new Uint8Array(wasm_memory.buffer,ptr,len).slice().buffer]);
  };
  // One transaction per commit: the pieces of one flush land together or not at all.
  env.rs09_store_commit=()=>{
   const batch=staged;staged=[];
   if(!db||failed)return -1;
   let tx;
   try {
    tx=db.transaction(STORE,'readwrite');
    const store=tx.objectStore(STORE);
    for(const [key,value] of batch)store.put(value,key);
   } catch(_){failed=true;return -1;}
   writing++;
   let settled=false;
   const settle=ok=>{
    if(settled)return;
    settled=true;writing--;
    if(ok){commits++;for(const [,value] of batch)written+=value.byteLength;}else failed=true;
   };
   tx.oncomplete=()=>settle(true);tx.onerror=tx.onabort=()=>settle(false);
   return 0;
  };
  env.rs09_store_busy=()=>writing>0?1:0;
  env.rs09_store_failed=()=>failed?1:0;
  env.rs09_store_clear=()=>{
   staged=[];entries=new Map();
   if(!db||failed)return;
   try{db.transaction(STORE,'readwrite').objectStore(STORE).clear();}catch(_){failed=true;}
  };
  // The preferences file (preferences_file.rs): the original's bytes, kept
  // under the file's name in local storage, which is read synchronously at
  // start as the applet reads the file. Base64, since storage holds text.
  const PREFS='rs09:jagex_runescape_preferences.dat';
  const prefs=()=>{
   try{const text=localStorage.getItem(PREFS);if(text===null)return null;
    return Uint8Array.from(atob(text),c=>c.charCodeAt(0));}catch(_){return null;}
  };
  env.rs09_prefs_length=()=>prefs()?.length??-1;
  env.rs09_prefs_read=(ptr,len)=>{
   const value=prefs();
   if(!value||value.length!==len)return -1;
   new Uint8Array(wasm_memory.buffer,ptr,len).set(value);return 0;
  };
  env.rs09_prefs_write=(ptr,len)=>{
   try{
    localStorage.setItem(PREFS,btoa(String.fromCharCode(...new Uint8Array(wasm_memory.buffer,ptr,len))));
    return 0;
   }catch(_){return -1;}
  };
  // The cache's progress as JSON, for the page and for review.
  env.rs09_cache_state=(ptr,len)=>{
   try {
    const state=JSON.parse(text(ptr,len));
    window.rs09CacheState=state;proof.dataset.state=JSON.stringify(state);
   } catch(_){}
  };
 }});
})();
