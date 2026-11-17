"""Build a separate UI/player-body pack from pinned cache bytes.

The earlier UI/model and world packs remain immutable compatibility inputs.
"""
import argparse,hashlib,json,pathlib,struct,subprocess
from prepare_visual import Cache,Reader,ROOT,LOCK,git_file
def sha(b):return hashlib.sha256(b).hexdigest()
def prepare(cache_path,ui,output):
    if output.resolve()==ui.resolve():raise ValueError('Use a separate output pack')
    for name in ['main_file_cache.dat2',*[f'main_file_cache.idx{i}'for i in [0,1,2,7,20]]]:
        expected=next(line.split(':')[1]for line in git_file('Server/data/cache/'+name).decode().splitlines()if line.startswith('oid '))
        if sha((cache_path/name).read_bytes())!=expected:raise ValueError(name+' differs from pin')
    deps=json.loads(subprocess.check_output(['cargo','run','--quiet','-p','rs09-client','--example','player_model_assets','--',str(ui)],cwd=ROOT))
    raw=ui.read_bytes();n=struct.unpack_from('<I',raw,8)[0];manifest=json.loads(raw[12:12+n]);entries={k:raw[12+n+off:12+n+off+size]for k,(off,size)in manifest['files'].items()};cache=Cache(cache_path)
    missing=[];added=0
    for id in deps['models']:
        name=f'model/{id}'
        if name not in entries:
            try:entries[name]=cache.file(7,id);added+=1
            except KeyError:missing.append(id)
    sequences=set();bas=cache.group(2,32)
    for id,data in sorted(bas.items()):
        entries[f'bas/{id}']=data;r=Reader(data)
        while(op:=r.u1()):
            if op==1:sequences.update([r.u2(),r.u2()])
            elif 2<=op<=9 or 38<=op<=42:sequences.add(r.u2())
            elif op in [26,30,32,33,35,36,43,44,45]:r.take(2)
            elif op==27:r.take(13)
            elif op in [29,31,34,37]:r.take(1)
            else:raise ValueError('Unknown BAS opcode '+str(op))
    sequences.discard(65535);groups=set()
    for id in sorted(sequences):
        data=cache.file(20,id>>7,id&127);entries[f'seq/{id}']=data;r=Reader(data)
        while(op:=r.u1()):
            if op in [1,12]:
                count=r.u2()if op==1 else r.u1()
                if op==1:r.take(count*2)
                lows=[r.u2()for _ in range(count)];highs=[r.u2()for _ in range(count)]
                groups.update(high for low,high in zip(lows,highs)if (high<<16|low)!=65535)
            elif op in [2,6,7]:r.take(2)
            elif op in [4,14,15,16]:pass
            elif op in [5,8,9,10,11]:r.take(1)
            elif op==3:r.take(r.u1())
            elif op==13:
                for _ in range(r.u2()):
                    count=r.u1()
                    if count:r.take(3+(count-1)*2)
            else:raise ValueError('Unknown sequence opcode '+str(op))
    bases=set()
    for group in sorted(groups):
        for id,data in sorted(cache.group(0,group).items()):
            entries[f'frame/{group}/{id}']=data;bases.add(int.from_bytes(data[:2],'big'))
    for id in sorted(bases):entries[f'base/{id}']=cache.file(1,id)
    blob=bytearray();spans={}
    for name,data in entries.items():spans[name]=[len(blob),len(data)];blob.extend(data)
    header=json.dumps(dict(source=manifest['source'],files=spans),separators=(',',':')).encode();output.parent.mkdir(parents=True,exist_ok=True);output.write_bytes(b'RS09VIS1'+struct.pack('<I',len(header))+header+blob)
    report=dict(source=LOCK['server'],baseUiSha256=sha(raw),nativeDependencyDiscovery=deps,additionalModels=added,missingModels=missing,basDefinitions=len(bas),movementSequences=len(sequences),frameGroups=len(groups),bases=len(bases),bytes=output.stat().st_size,sha256=sha(output.read_bytes()))
    output.with_suffix('.provenance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k!='nativeDependencyDiscovery'},indent=2))
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cache',type=pathlib.Path,default=ROOT/'assets/cache');p.add_argument('--ui',type=pathlib.Path,default=ROOT/'assets/visual/ui-models.pack');p.add_argument('--output',type=pathlib.Path,default=ROOT/'assets/visual/ui-players.pack');a=p.parse_args();prepare(a.cache,a.ui,a.output)
