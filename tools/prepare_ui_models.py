"""Extend a separate UI pack with raw model-widget/head dependencies.

Keeps the verified base UI/world packs unchanged. Native decoders discover
component/item/NPC dependencies; Python only packages pinned original bytes.
"""
import argparse,hashlib,json,pathlib,re,struct,subprocess
from prepare_visual import Cache,Reader,ROOT,LOCK,git_file

def sha(data):return hashlib.sha256(data).hexdigest()
def prepare(cache_path,ui,world,output):
    if output.resolve()in[ui.resolve(),world.resolve()]:raise ValueError('Use a separate output pack')
    for name in ['main_file_cache.dat2',*[f'main_file_cache.idx{i}'for i in[0,1,2,7,20]]]:
        expected=next(line.split(':')[1]for line in git_file('Server/data/cache/'+name).decode().splitlines()if line.startswith('oid '))
        if sha((cache_path/name).read_bytes())!=expected:raise ValueError(name+' differs from pin')
    cmd=['cargo','run','--quiet','-p','rs09-client','--example','ui_model_assets','--',str(ui),str(world)]
    deps=json.loads(subprocess.check_output(cmd,cwd=ROOT))
    raw=ui.read_bytes();n=struct.unpack_from('<I',raw,8)[0];manifest=json.loads(raw[12:12+n]);start=12+n
    entries={name:raw[start+off:start+off+size]for name,(off,size)in manifest['files'].items()}
    cache=Cache(cache_path);models=set(deps['models']);sequences=set(deps['sequences']);kit_count=0
    for id,data in sorted(cache.group(2,3).items()):
        entries[f'kit/{id}']=data;kit_count+=1;r=Reader(data)
        while(op:=r.u1()):
            if op==1:r.take(1)
            elif op==2:models.update(r.u2()for _ in range(r.u1()))
            elif op==3:pass
            elif op in[40,41]:r.take(r.u1()*4)
            elif 60<=op<70:models.add(r.u2())
            else:raise ValueError('Unknown identity kit dependency opcode '+str(op))
    facial_source=git_file('Server/src/main/core/game/dialogue/FacialExpression.java')
    sequences.update(map(int,re.findall(rb'\b[A-Z][A-Z_0-9]*\((\d+)\)',facial_source)))
    missing=[]
    for id in sorted(models):
        try:entries[f'model/{id}']=cache.file(7,id)
        except KeyError:missing.append(id)
    groups=set()
    for id in sorted(sequences):
        data=cache.file(20,id>>7,id&127);entries[f'seq/{id}']=data;r=Reader(data)
        while(op:=r.u1()):
            if op in[1,12]:
                count=r.u2()if op==1 else r.u1()
                if op==1:r.take(count*2)
                lows=[r.u2()for _ in range(count)];highs=[r.u2()for _ in range(count)]
                groups.update(high for low,high in zip(lows,highs)if (high<<16|low)!=65535)
            elif op in[2,6,7]:r.take(2)
            elif op in[4,14,15,16]:pass
            elif op in[5,8,9,10,11]:r.take(1)
            elif op==3:r.take(r.u1())
            elif op==13:
                for _ in range(r.u2()):
                    count=r.u1()
                    if count:r.take(3+(count-1)*2)
            else:raise ValueError('Unknown sequence dependency opcode '+str(op))
    bases=set()
    for group in sorted(groups):
        for id,data in sorted(cache.group(0,group).items()):
            entries[f'frame/{group}/{id}']=data;bases.add(int.from_bytes(data[:2],'big'))
    for id in sorted(bases):entries[f'base/{id}']=cache.file(1,id)
    blob=bytearray();spans={}
    for name,data in entries.items():spans[name]=[len(blob),len(data)];blob.extend(data)
    header=json.dumps(dict(source=manifest['source'],files=spans),separators=(',',':')).encode()
    output.parent.mkdir(parents=True,exist_ok=True);output.write_bytes(b'RS09VIS1'+struct.pack('<I',len(header))+header+blob)
    provenance=dict(source=LOCK['server'],baseUiSha256=sha(ui.read_bytes()),worldSha256=sha(world.read_bytes()),facialExpressionSource=sha(facial_source),nativeDependencyDiscovery=deps,identityKits=kit_count,widgetSequences=len(sequences),frameGroups=len(groups),bases=len(bases),modelsRequested=len(models),missingModels=missing,bytes=output.stat().st_size,sha256=sha(output.read_bytes()))
    output.with_suffix('.provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({k:v for k,v in provenance.items()if k!='nativeDependencyDiscovery'},indent=2))

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cache',type=pathlib.Path,default=ROOT/'assets/cache');p.add_argument('--ui',type=pathlib.Path,default=ROOT/'assets/visual/ui.pack');p.add_argument('--world',type=pathlib.Path,default=ROOT/'assets/visual/live/lumbridge-minimap.pack');p.add_argument('--output',type=pathlib.Path,default=ROOT/'assets/visual/ui-models.pack');a=p.parse_args();prepare(a.cache,a.ui,a.world,a.output)
