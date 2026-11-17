"""Package pinned interface/script/font bytes. No layout or gameplay is baked."""
import argparse,hashlib,json,pathlib,struct
from prepare_visual import Cache,Reader,ROOT,LOCK,git_file
def item_models(data):
    r=Reader(data);models=set()
    while (op:=r.u1()):
        if op==1:models.add(r.u2())
        elif op==2 or 30<=op<=39:r.text()
        elif op in [4,5,6,7,8,23,24,25,26,78,79,90,91,92,93,95,97,98,110,111,112,121,122]:r.take(2)
        elif op==12:r.take(4)
        elif op in [40,41]:r.take(r.u1()*4)
        elif op==42:r.take(r.u1())
        elif op in [96,113,114,115]:r.take(1)
        elif 100<=op<=109:r.take(4)
        elif op in [125,126,127,128,129,130]:r.take(3)
        elif op==249:
            for _ in range(r.u1()):
                text=r.u1()==1;r.take(3)
                if text:r.text()
                else:r.take(4)
    return models
def prepare(path,output):
    for archive in [2,3,4,7,8,10,12,13,17,19,22]:
        name=f'main_file_cache.idx{archive}'
        oid=next(line.split(':')[1] for line in git_file('Server/data/cache/'+name).decode().splitlines() if line.startswith('oid '))
        if hashlib.sha256((path/name).read_bytes()).hexdigest()!=oid:raise ValueError(name+' differs from pinned cache')
    name='main_file_cache.dat2'
    oid=next(line.split(':')[1] for line in git_file('Server/data/cache/'+name).decode().splitlines() if line.startswith('oid '))
    if hashlib.sha256((path/name).read_bytes()).hexdigest()!=oid:raise ValueError(name+' differs from pinned cache')
    cache=Cache(path);spans={};blob=bytearray();counts={};models={0}
    for archive,prefix in [(3,'interface'),(12,'script'),(13,'font'),(17,'enum'),(19,'item'),(22,'varbit')]:
        count=0
        for group in sorted(cache.index(archive)[1]):
            for fid,data in sorted(cache.group(archive,group).items()):
                name=f'{prefix}/{group}/{fid}' if archive==3 else f'{prefix}/{group*256+fid}' if archive in [17,19] else f'{prefix}/{group*1024+fid}' if archive==22 else f'{prefix}/{group}'
                spans[name]=[len(blob),len(data)];blob.extend(data);count+=1
                if archive==19:models.update(item_models(data))
        counts[prefix]=count
    for fid,data in sorted(cache.group(2,5).items()):
        spans[f'invtype/{fid}']=[len(blob),len(data)];blob.extend(data)
    for fid,data in sorted(cache.group(2,16).items()):
        spans[f'varptype/{fid}']=[len(blob),len(data)];blob.extend(data)
    params=cache.group(2,11)
    for fid,data in sorted(params.items()):
        spans[f'param/{fid}']=[len(blob),len(data)];blob.extend(data)
    spans['param-catalog-complete']=[len(blob),0];counts['params']=len(params)
    for id in sorted(models):
        data=cache.file(7,id);spans[f'model/{id}']=[len(blob),len(data)];blob.extend(data)
    counts['item_models']=len(models)
    for group in sorted(cache.index(4)[1]):
        data=cache.file(4,group);spans[f'sound/{group}']=[len(blob),len(data)];blob.extend(data)
    counts['sounds']=len(cache.index(4)[1])
    def named(archive,name):
        h=0
        for b in name.encode('latin1'):h=(h*31+b)&0xffffffff
        return cache.file(archive,cache.index(archive)[0][h])
    overhead_names=['p11_full','p12_full','b12_full','hitmarks','hitbar_default','headicons_pk','headicons_prayer','hint_headicons']
    overhead_ids={}
    for name in overhead_names:
        h=0
        for b in name.encode('latin1'):h=(h*31+b)&0xffffffff
        overhead_ids[name]=cache.index(8)[0][h]
    data=json.dumps(overhead_ids,separators=(',',':')).encode()
    spans['overhead/assets']=[len(blob),len(data)];blob.extend(data)
    for name,data in [('huffman',named(10,'huffman')),('sprite/name_icons',named(8,'name_icons')),('sprite/scrollbar',named(8,'scrollbar'))]:spans[name]=[len(blob),len(data)];blob.extend(data)
    source='Pinned 2009scape cache / '+LOCK['server']['commit']
    manifest=json.dumps(dict(source=source,files=spans),separators=(',',':')).encode()
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_bytes(b'RS09VIS1'+struct.pack('<I',len(manifest))+manifest+blob)
    provenance=dict(server=LOCK['server'],client=LOCK['client'],counts=counts,bytes=output.stat().st_size,sha256=hashlib.sha256(output.read_bytes()).hexdigest())
    output.with_suffix('.provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
    print(json.dumps(provenance,indent=2))
if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('--cache',type=pathlib.Path,default=ROOT/'assets/cache');ap.add_argument('--output',type=pathlib.Path,default=ROOT/'assets/visual/ui.pack');a=ap.parse_args();prepare(a.cache,a.output)
