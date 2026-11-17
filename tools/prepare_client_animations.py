"""Package the complete pinned sequence/frame/base archives separately from closed UI packs."""
import argparse,hashlib,json,pathlib,struct
from prepare_visual import Cache,ROOT,LOCK,git_file

def prepare(cache_path,output):
    if output.exists():raise ValueError('Use a fresh output path; existing animation packs are immutable')
    inputs={}
    for name in ['main_file_cache.dat2',*[f'main_file_cache.idx{i}'for i in [0,1,20,255]]]:
        expected=next(line.split(':')[1]for line in git_file('Server/data/cache/'+name).decode().splitlines()if line.startswith('oid '))
        actual=hashlib.sha256((cache_path/name).read_bytes()).hexdigest()
        if actual!=expected:raise ValueError(name+' differs from the pin')
        inputs[name]=actual
    cache=Cache(cache_path);entries={};counts={}
    for archive,prefix in [(20,'seq'),(0,'frame'),(1,'base')]:
        count=0
        for group in sorted(cache.index(archive)[1]):
            for file,data in sorted(cache.group(archive,group).items()):
                if prefix=='seq':name=f'seq/{group*128+file}'
                elif prefix=='base':
                    if file!=0:raise ValueError(('Unexpected base file',group,file))
                    name=f'base/{group}'
                else:name=f'frame/{group}/{file}'
                entries[name]=data;count+=1
        counts[prefix]=count
    entries['animation-catalog-complete']=json.dumps(counts,separators=(',',':')).encode()
    blob=bytearray();spans={}
    for name,data in entries.items():spans[name]=[len(blob),len(data)];blob.extend(data)
    header=json.dumps(dict(source="Pinned 2009scape cache / "+LOCK['server']['commit'],files=spans),separators=(',',':')).encode()
    output.parent.mkdir(parents=True,exist_ok=True);output.write_bytes(b'RS09VIS1'+struct.pack('<I',len(header))+header+blob)
    report=dict(scope='Complete pinned JS5 archives20/0/1 with unchanged raw sequence/frame/base bytes. Runtime decodes metadata before requests and frame dependencies on demand. Separate from existing immutable UI/world packs; no animation/content/gameplay parity claim.',source=LOCK['server'],cacheInputs=inputs,counts=counts,bytes=output.stat().st_size,sha256=hashlib.sha256(output.read_bytes()).hexdigest())
    output.with_suffix('.provenance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cache',type=pathlib.Path,default=ROOT/'assets/cache');p.add_argument('--output',type=pathlib.Path,default=ROOT/'assets/visual/animations.pack');a=p.parse_args();prepare(a.cache,a.output)
