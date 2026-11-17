"""Pinned nearby NPC assembly, BAS and representative original GL poses."""
import hashlib,importlib.util,json,pathlib,subprocess
ROOT_TYPES=[0,1,2,3,4,5,9,41,42,47,59,81,86,100,278,519,520,521,708,2281,2342,3021,3384,3777,4899,4942,7969]

def generate(root,visual,check=False,*,roots=None,folder='npc-assets',include_movement=False):
    sources=visual.generate(build_only=True)
    spec=importlib.util.spec_from_file_location('npc_asset_prepare',root/'tools/prepare_visual.py');p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)
    root_types=ROOT_TYPES if roots is None else roots
    cache=p.Cache(root/'assets/cache');run=root/'runs/players-oracle'/folder;run.mkdir(parents=True,exist_ok=True);inputs={};cases=[];requests=[];types={};pending=list(root_types)
    def save(name,data):
        file=name.replace('/','_')+'.bin';inputs[file]=data;(run/file).write_bytes(data);return file
    save('materials',cache.file(26,0))
    # VisualOracle initializes its original dynamic-light noise service even for
    # body-only requests. Retain that explicit, pinned service input too.
    inputs['light-noise.bin']=(root/'fixtures/visual/light-noise.bin').read_bytes()
    (run/'light-noise.bin').write_bytes(inputs['light-noise.bin'])
    while pending:
        id=pending.pop()
        if id in types:continue
        raw=cache.file(18,id>>7,id&127);models,bas,children=p.npc_dependencies(raw);types[id]=(models,bas,children);pending.extend(sorted(children))
        save(f'npc/{id}',raw)
        for model in sorted(models):save(f'model/{model}',cache.file(7,model))
        if bas!=-1:save(f'bas/{bas}',cache.file(2,32,bas))
    leafs=[]
    for id,(models,bas,children) in sorted(types.items()):
        if children:continue
        leafs.append(id);file=f'npc_{id}.bin';basfile=f'bas_{bas}.bin'
        for kind in ['npcdecode','npcmodel']:
            cases.append(dict(kind=kind,file=file,bas=basfile,id=f'{kind}_{id}'));requests.append(f'{kind}\t{file}\t{basfile}')
        r=p.Reader(inputs[basfile]);seq_ids=[]
        while (op:=r.u1()):
            if op==1:seq_ids.extend([r.u2(),r.u2()])
            elif op==26:r.take(2)
            elif op in (*range(2,10),*range(38,46)):
                value=r.u2()
                if include_movement and op<=42:seq_ids.append(value)
            else:raise ValueError(f'NPC{id} BAS{bas} needs a wider pose oracle: opcode{op}')
        for seq in sorted(set(seq_ids)-{65535}):
            seqfile=save(f'seq/{seq}',cache.file(20,seq>>7,seq&127));r=p.Reader(inputs[seqfile])
            while (op:=r.u1())!=1:
                if op in (2,6,7):r.take(2)
                elif op in (5,8,9,10,11):r.take(1)
                elif op in (4,14,15,16):pass
                elif op==3:r.take(r.u1())
                else:raise ValueError(f'NPC{id} sequence{seq} lacks frames: opcode{op}')
            n=r.u2();delays=[r.u2()for _ in range(n)];lows=[r.u2()for _ in range(n)];highs=[r.u2()for _ in range(n)]
            for i in sorted({0,n//2,n-1}):
                j=(i+1)%n;a=save(f'frame/{highs[i]}/{lows[i]}',cache.file(0,highs[i],lows[i]));b=save(f'frame/{highs[j]}/{lows[j]}',cache.file(0,highs[j],lows[j]));bid=int.from_bytes(inputs[a][:2],'big')
                base=save(f'base/{bid}',cache.file(1,bid))
                if int.from_bytes(inputs[b][:2],'big')!=bid:continue
                for delta in sorted({0,delays[i]//2,delays[i]-1}):
                    for tween in [False,True]:
                        angle=(i*128)&2047;case=dict(kind='npcpose',id=f'npc_{id}_{seq}_{i}_{delta}_{tween}',file=file,bas=basfile,seq=seqfile,base_id=bid,base=base,first=a,next=b,delta=delta,length=delays[i],tween=tween,angle=angle)
                        cases.append(case);requests.append('\t'.join(map(str,['npcpose',file,basfile,seqfile,bid,base,a,b,delta,delays[i],str(tween).lower(),angle])))
    (run/'requests.tsv').write_text('\n'.join(requests)+'\n')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(visual.RUN),*[str(f)for f in visual.RUN.glob('*.java')]],check=True)
    output=subprocess.check_output(['java','-cp',str(visual.RUN),'VisualOracle',str(run)],text=True).splitlines();assert len(output)==len(cases)
    for c,line in zip(cases,output):c['expected']=line.split('\t')[2]
    bas_ids=sorted({b for _,b,_ in types.values()}-{ -1 })
    probe='''import java.nio.file.*;import java.util.*;public class NpcBasProbe {public static void main(String[]args)throws Exception {for(int i=1;i<args.length;i++){BasType b=new BasType();b.decode(new Buffer(Files.readAllBytes(Path.of(args[0],"bas_"+args[i]+".bin"))));System.out.println(Arrays.toString(new int[]{b.idleAnimationId,b.walkAnimation,b.runAnimationId,b.standingCCWTurn,b.standingCWTurn,b.walkFullTurnAnimationId,b.walkCCWTurnAnimationId,b.walkCWTurnAnimationId,b.terrainAlignWidth,b.terrainAlignDepth}));}}}'''
    (run/'NpcBasProbe.java').write_text(probe);subprocess.run(['javac','-cp',str(visual.RUN),'-d',str(run),str(run/'NpcBasProbe.java')],check=True)
    import os
    lines=subprocess.check_output(['java','-cp',str(run)+os.pathsep+str(visual.RUN),'NpcBasProbe',str(run),*map(str,bas_ids)],text=True).splitlines()
    bas_cases=[dict(id=id,file=f'bas_{id}.bin',expected=json.loads(line))for id,line in zip(bas_ids,lines)]
    bindings='Real pinned nearby NPC definitions/models/BAS, complete leaf bind geometry and representative first/middle/last idle/walk GL frame samples with fractional tweening and original shadows. Unchanged original methods from visual oracle; flat terrain services. Morph selection is a separate corpus.'
    if include_movement:bindings+=' This isolated corpus also includes every configured movement/run/turn sequence.'
    fixture=dict(sources=sources,bindings=bindings,root_types=root_types,asset_types=sorted(types),body_types=leafs,bas=bas_cases,inputs={n:hashlib.sha256(b).hexdigest()for n,b in inputs.items()},cases=cases)
    content=json.dumps(fixture,separators=(',',':')).encode()+b'\n';dest=root/'fixtures/players'/folder
    if check:
        if (dest/'oracle.json').read_bytes()!=content:raise SystemExit('NPC asset corpus differs')
        for n,b in inputs.items():
            if (dest/n).read_bytes()!=b:raise SystemExit('NPC asset input differs: '+n)
    else:
        dest.mkdir(parents=True,exist_ok=True)
        for n,b in inputs.items():(dest/n).write_bytes(b)
        (dest/'oracle.json').write_bytes(content)
    print(f'Original nearby NPC assets: {len(types)} types/{len(leafs)} leaf bodies/{len(bas_cases)} BAS/{len(cases)} geometry and pose cases')
