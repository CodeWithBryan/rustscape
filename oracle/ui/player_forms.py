"""Original transformed-player position/policy and NPC raw/GL body observations."""
import argparse, hashlib, importlib.util, json, pathlib, random, struct, subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
RUN = ROOT / 'runs/player-form-oracle'
FIX = ROOT / 'fixtures/ui/player-forms.json'

def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value

v = module('player_form_visual', ROOT / 'oracle/visual/generate.py')
h = module('player_form_pack', ROOT / 'oracle/ui/model_geometry.py')
p = module('player_form_dependencies', ROOT / 'tools/prepare_visual.py')

def generate(check=False):
    v.RUN = RUN
    sources = v.generate(build_only=True)
    def source(name):
        raw = (v.SRC / (name + '.java')).read_bytes().replace(b'\r\n', b'\n')
        sources[name + '.java'] = h.sha(raw)
        return v.clean(raw.decode())
    npc = source('NpcType')
    body = v.method(npc, 'public final Model getBodyModel(')
    assembly = body[body.index('RawModel[] rawModels ='):body.index('model = rawModel.createModel')]
    assembly = assembly.replace('client.npcRecolorPalette', 'NPC_PALETTE')
    path = RUN / 'NpcDefinitionOracle.java'
    old = path.read_text()
    old = old[:old.index('RawModel assemble()')]
    path.write_text(old + 'RawModel assemble(){int i,frame,tweenFrame,translateZ,rotateX,rotateY,frameGroupId,frameDelay,groupId,slot,slotMask;'
                    + assembly + 'return rawModel;}}' + SUPPORT_TYPES
                    + 'class NpcType extends NpcDefinitionOracle {' + v.method(npc, 'public final NpcType getMultiNpc(') + '}')
    raw_path = RUN / 'RawModel.java'
    raw_path.write_text(raw_path.read_text().rstrip()[:-1] + v.method(source('RawModel'), 'public final void rotateXYZ(') + '}')
    player = source('Player')
    decode = v.method(player, 'public final void decodeAppearance(')
    first = decode[decode.index('this.setSize('):decode.index('this.skullHeadIcon =')]
    last = decode[decode.index('if (this.appearance == null)'):decode.index('if (this.particleSystem != null)')]
    shadow = player[player.index('if (Preferences.characterShadowsOn &&'):]
    shadow = shadow[:shadow.index(' {')].removeprefix('if (')[:-1]
    player_code = ('class PlayerForm extends FormEntity {PlayerAppearance appearance;int xFine,yFine,basTypeId,titleIndex;int[]movementQueueX,movementQueueY;'
                   + v.method(player, 'public final int getSize(') + v.method(player, 'public final int getBasId(')
                   + 'void decode(int flags,int npcId){int prevSize=super.getSize();int teamId,gender=0;int[]colors={},identikit={};'
                   + first + last + '}boolean shadow(){return ' + shadow + ';}}')
    (RUN / 'PlayerForm.java').write_text(player_code + SUPPORT_PLAYER)
    # The actor body uses the same unchanged terrain method as attached effects,
    # after animation/resize and after recording its original shadow bounds.
    alignment = v.method(source('PathingEntity'), 'protected final void alignToTerrain(')
    alignment = alignment.replace('Model model', 'GlModel model').replace('SceneGraph.getTileHeight', 'FormGround.height').replace('Player.plane', 'FormGround.plane')
    (RUN / 'FormTerrain.java').write_text(SUPPORT_TERRAIN + alignment + '}')
    gl_path = RUN / 'GlModel.java'
    gl = source('GlModel')
    gl_path.write_text(gl_path.read_text().rstrip()[:-1] + ''.join(v.method(gl, signature) for signature in
        ['public final void rotateX(', 'public final void rotateZ(', 'public final int getMinY()']) + '}')
    support = RUN / 'Support.java'
    support.write_text(support.read_text().replace('class Preferences {', 'class Preferences {static boolean characterShadowsOn=true;'))
    support = RUN / 'MovementSupport.java'
    text = support.read_text()
    text = text.replace(v.method(text, 'class NpcType {'), '')
    support.write_text(text)
    (RUN / 'FormOracle.java').write_text(DRIVER)
    subprocess.run(['javac', '-encoding', 'UTF-8', '-d', str(RUN), *map(str, RUN.glob('*.java'))], check=True)
    paths = [ROOT / 'runs/client-spot-validation/fixtures/lumbridge-spots.pack',
             ROOT / 'runs/client-pickup-assets-validation/fixtures/ui-pickup.pack']
    assets = {}
    packs = []
    for path in paths:
        assets.update(h.pack(path))
        packs.append(dict(path=path.relative_to(ROOT).as_posix(), sha256=h.sha(path.read_bytes())))
    data = RUN / 'data'
    data.mkdir(exist_ok=True)
    materials = (ROOT / 'fixtures/visual/materials.bin').read_bytes()
    (data / 'materials.bin').write_bytes(materials)
    types = {id: assets[f'npc/{id}'] for id in [4, 41, 81, 100]}
    # Controlled definition services exercise mixed parent/child size, shadow,
    # scale and both original variable selection branches without a content claim.
    u2 = lambda n: struct.pack('>H', n & 65535)
    types[60000] = bytes([12, 3, 111, 118]) + u2(-1) + u2(7) + u2(-1) + bytes([1]) + u2(41) + u2(81) + b'\0'
    types[60001] = bytes([12, 2, 118]) + u2(9) + u2(7) + u2(60000) + bytes([0]) + u2(100) + b'\0'
    types[60002] = bytes([12, 4, 97]) + u2(96) + bytes([98]) + u2(160) + b'\0'
    models = sorted(p.npc_dependencies(types[81])[0])
    if len(models) == 1: models *= 2
    for model in set(models): (data / f'model_{model}.bin').write_bytes(assets[f'model/{model}'])
    colour_probe = subprocess.run(['java','-cp',str(RUN),'FormOracle',str(data)], input=f'D\t81\t{types[81].hex()}\nQ\t81\n',text=True,capture_output=True,check=True)
    colours = json.loads(colour_probe.stdout)
    # Same cached model bytes, explicit original definition/BAS decoder inputs.
    synthetic = bytes([1, len(models)]) + b''.join(map(u2, models))
    synthetic += bytes([97]) + u2(72) + bytes([98]) + u2(176)
    synthetic += bytes([40, 2]) + u2(colours[0]) + u2(100) + u2(colours[1]) + u2(200) + bytes([42, 1, 17])
    synthetic += bytes([121, 2, 0, 127, 128, 64, 1, 255, 30, 10, 127]) + u2(60003) + b'\0'
    types[60003] = synthetic
    bas_raw = bytearray()
    for index in range(min(2, len(models))):
        bas_raw += bytes([27, index]) + b''.join(u2(n) for n in [13-index*30, -21, 48, 128, 512, 1792])
    bas_raw += b'\0'
    requests = ['D\t' + str(id) + '\t' + raw.hex() for id, raw in sorted(types.items())]
    requests.append('B\t60003\t' + bas_raw.hex())
    for raw in types.values():
        for model in p.npc_dependencies(raw)[0]:
            (data / f'model_{model}.bin').write_bytes(assets[f'model/{model}'])
    for raw in types.values():
        bas = p.npc_dependencies(raw)[1]
        if bas != -1 and bas != 60003:
            requests.append(f'B\t{bas}\t' + assets[f'bas/{bas}'].hex())
    rng = random.Random(20091020)
    cases = []
    for index in range(1024):
        previous = rng.choice([-2, -1, 41, 81, 60000, 60001, 60002])
        npc_id = rng.choice([-1, 41, 81, 60000, 60001, 60002])
        values = [rng.randrange(256), previous, rng.randrange(256), rng.randrange(-2147483648,2147483648),
                  rng.randrange(-2147483648,2147483648), rng.randrange(-20000000,20000000),
                  rng.randrange(-20000000,20000000), npc_id, rng.randrange(65536), rng.randrange(-1,4), rng.randrange(-1,4)]
        cases.append(dict(kind='form', input=values))
        requests.append('F\t' + ','.join(map(str, values)))
    body_ids = [4, 41, 81, 100, 60003]
    for id in body_ids:
        cases.append(dict(kind='body', id=id))
        requests.append(f'A\t{id}')
    # All direct frames plus multiple integer tween fractions on real cached
    # player movement, including scaled bodies and normal transformations.
    # 808 is the original standing player sequence; a transform keeps player BAS.
    seq_id = 808
    # Sequence opcode1 is first for this pinned original sequence.
    seq_raw = assets[f'seq/{seq_id}']
    assert seq_raw[0] == 1
    count = int.from_bytes(seq_raw[1:3], 'big')
    delays = list(struct.unpack_from('>' + 'H'*count, seq_raw, 3))
    lows = struct.unpack_from('>' + 'H'*count, seq_raw, 3 + count*2)
    highs = struct.unpack_from('>' + 'H'*count, seq_raw, 3 + count*4)
    frames = [(hi << 16) | lo for hi,lo in zip(highs,lows)]
    for id in body_ids:
        for index, (ref, length) in enumerate(zip(frames, delays)):
            next_ref = frames[(index+1) % len(frames)]
            first = assets[f'frame/{ref>>16}/{ref&65535}']
            next_frame = assets[f'frame/{next_ref>>16}/{next_ref&65535}']
            base_id = int.from_bytes(first[:2], 'big')
            for normals in [False, True]:
                for delta in sorted({0, 1, length//2, length-1}):
                    c = dict(kind='pose', id=id, sequence=seq_id, frame=index, next=(index+1)%len(frames), delta=delta, length=length, normals=normals,
                             first=f'frame/{ref>>16}/{ref&65535}', second=f'frame/{next_ref>>16}/{next_ref&65535}', base=f'base/{base_id}', base_id=base_id)
                    cases.append(c)
                    requests.append('\t'.join(['P', str(id), first.hex(), next_frame.hex(), assets[c['base']].hex(), str(delta), str(length), str(normals).lower()]))
    old_count = len(cases)
    terrain_cases = []
    terrain_bas = {66: assets['bas/66'], 1426: assets['bas/1426'],
                   60004: bytes([26, 0, 64, 0]), 60005: bytes([26, 24, 24, 0]),
                   60006: bytes([26, 255, 255, 0])}
    requests.extend('B\t' + str(id) + '\t' + raw.hex() for id, raw in terrain_bas.items())
    # Real Cow idle frames and existing transformed-player poses exercise both
    # a parent's Cow BAS and the player's own zero-dimension human BAS.
    idle = assets['seq/5852']
    assert idle[0] == 1
    n = int.from_bytes(idle[1:3], 'big')
    lengths, lows, highs = [struct.unpack_from('>' + 'H'*n, idle, 3 + n*2*i) for i in range(3)]
    poses = [c for c in cases if c['kind'] == 'pose']
    terrain_rng = random.Random(5301020)
    for index in range(256):
        if index < 128:
            frame = index % n
            nxt = (frame + 1) % n
            first_name, second_name = f'frame/{highs[frame]}/{lows[frame]}', f'frame/{highs[nxt]}/{lows[nxt]}'
            first, second = assets[first_name], assets[second_name]
            base_id = int.from_bytes(first[:2], 'big')
            c = dict(kind='terrain', id=81, sequence=5852, frame=frame, next=nxt,
                     delta=index % lengths[frame], length=lengths[frame], normals=bool(index & 1),
                     first=first_name, second=second_name, base=f'base/{base_id}', base_id=base_id)
        else:
            c = dict(poses[(index * 17) % len(poses)])
            c.pop('expected', None)
            c['kind'] = 'terrain'
        c.update(bas_id=[66, 1426, 60004, 60005, 60006][index % 5], yaw=terrain_rng.randrange(2048),
                 fine=[terrain_rng.randrange(-10000, 20000) for _ in range(2)],
                 tile_height=terrain_rng.randrange(-500, 501),
                 heights=[terrain_rng.randrange(-600, 601) for _ in range(4)])
        terrain_cases.append(c)
        requests.append('\t'.join(['P', str(c['id']), assets[c['first']].hex(), assets[c['second']].hex(),
            assets[c['base']].hex(), str(c['delta']), str(c['length']), str(c['normals']).lower(),
            ','.join(map(str, [c['bas_id'], c['yaw'], *c['fine'], c['tile_height'], *c['heights']]))]))
    (RUN / 'requests.tsv').write_text('\n'.join(requests)+'\n')
    result = subprocess.run(['java', '-cp', str(RUN), 'FormOracle', str(data)], input='\n'.join(requests)+'\n', text=True, capture_output=True)
    (RUN / 'stdout.log').write_text(result.stdout)
    (RUN / 'stderr.log').write_text(result.stderr)
    if result.returncode: raise SystemExit(result.stderr)
    lines = result.stdout.splitlines()
    assert len(lines) == old_count + len(terrain_cases), (len(lines), old_count, len(terrain_cases))
    for c,line in zip(cases, lines[:old_count]):
        c['expected'] = json.loads(line) if c['kind'] == 'form' else line
    for c,line in zip(terrain_cases, lines[old_count:]):
        c['expected'] = json.loads(line)
    out = dict(sources=sources, packs=packs, materialsSha256=h.sha(materials), types={str(id): raw.hex() for id,raw in sorted(types.items())},
               bas={'60003':bytes(bas_raw).hex(), **{str(id): raw.hex() for id, raw in terrain_bas.items()}}, cases=cases, terrainCases=terrain_cases,
               bindings='Unchanged Player getSize/getBasId and decodeAppearance position fragments; base-definition shadow expression. Unchanged NpcType getMultiNpc, body assembly fragment including offsets/BAS/palette, RawModel and GL model construction/animation/resize. Unchanged PathingEntity.alignToTerrain and GL rotateX/rotateZ/translate after poses, including original bounds and normals. Controlled definition/varp/varbit/height services, ready raw assets and initial zero palette. No slot-animation, full gameplay or GPU pixel equivalence claim.')
    if check:
        if json.loads(FIX.read_text()) != out: raise SystemExit('Player-form fixture differs')
    else:
        FIX.write_text(json.dumps(out, separators=(',', ':'))+'\n')
    print('Original player forms:', len(cases), 'preserved observations;', len(terrain_cases), 'actor terrain GL poses', flush=True)

SUPPORT_TYPES = '''
class NpcTypeList {static Js5 modelsArchive;static java.util.Map<Integer,NpcType>types=new java.util.HashMap<>();static NpcType get(int id){return types.get(id);}}
class BasTypeList {static java.util.Map<Integer,BasType>types=new java.util.HashMap<>();static BasType get(int id){return types.get(id);}}
class VarpDomain {static int bit;static int[]activeVarps=new int[65536];static int getVarbit(int id){return bit;}}
'''
SUPPORT_PLAYER = '''
class FormEntity {int size=1;public int getSize(){return size;}public int getBasId(){return -1;}void setSize(int size){this.size=size;}}
class PlayerAppearance {int npcId=-1;void set(int[]colors,int npcId,boolean gender,int[]kits,int bas){this.npcId=npcId;}}
'''
SUPPORT_TERRAIN = '''
class FormGround {static int plane,index;static int[]heights;static java.util.List<int[]>calls=new java.util.ArrayList<>();static int height(int p,int x,int y){calls.add(new int[]{p,x,y});return heights[index++];}}
class FormTerrain {static int terrainPitchAngle,terrainRollAngle,terrainYOffset;int xFine,yFine,tileHeight;BasType bas;BasType getBasType(){return bas;}
'''
DRIVER = r'''
import java.io.*;import java.nio.file.*;import java.util.*;
class FormOracle {
static int[]ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
static RawModel raw(int id){return NpcTypeList.get(id).assemble();}
public static void main(String[]args)throws Exception {
NpcTypeList.modelsArchive=new Js5(Path.of(args[0]));Rasteriser.textureProvider=new VisualMaterials(Files.readAllBytes(Path.of(args[0]).resolve("materials.bin")));BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));String line;
while((line=reader.readLine())!=null){String[]p=line.split("\t");switch(p[0]){
case"D":{NpcType n=new NpcType();n.id=Integer.parseInt(p[1]);n.decode(new Buffer(HexFormat.of().parseHex(p[2])));NpcTypeList.types.put(n.id,n);break;}
case"B":{BasType b=new BasType();b.decode(new Buffer(HexFormat.of().parseHex(p[2])));BasTypeList.types.put(Integer.parseInt(p[1]),b);break;}
case"F":{int[]a=ints(p[1]);PlayerForm f=new PlayerForm();f.size=a[1]==-2?1:(a[2]>>3&7)+1;if(a[1]!=-2){f.appearance=new PlayerAppearance();f.appearance.npcId=a[1];}f.xFine=a[3];f.yFine=a[4];f.movementQueueX=new int[]{a[5]};f.movementQueueY=new int[]{a[6]};f.basTypeId=a[8];VarpDomain.bit=a[9];Arrays.fill(VarpDomain.activeVarps,a[10]);f.decode(a[0],a[7]);NpcType n=a[7]==-1?null:NpcTypeList.get(a[7]);while(n!=null&&n.multiNpcs!=null)n=n.getMultiNpc();System.out.println("["+f.xFine+","+f.yFine+","+f.getSize()+","+f.getBasId()+","+f.shadow()+","+(n==null?"null":n.id)+","+(n==null?128:n.resizeX)+","+(n==null?128:n.resizeY)+"]");break;}
case"Q":{System.out.println(Arrays.toString(java.util.stream.IntStream.range(0,raw(Integer.parseInt(p[1])).triangleCount).map(i->raw(Integer.parseInt(p[1])).triangleColors[i]&65535).distinct().toArray()));break;}
case"A":{RawModel m=raw(Integer.parseInt(p[1]));ByteArrayOutputStream bytes=new ByteArrayOutputStream();VisualOracle.canonical(new DataOutputStream(bytes),m);System.out.println(VisualOracle.digest(bytes.toByteArray()));break;}
case"P":{NpcType n=NpcTypeList.get(Integer.parseInt(p[1]));RawModel m=n.assemble();m.triangleSources=new short[m.triangleCount];for(int i=0;i<m.triangleCount;i++)m.triangleSources[i]=(short)i;GlModel g=new GlModel(m,n.ambient+64,n.contrast+850,true);g.createBones();AnimBase base=new AnimBase(0,HexFormat.of().parseHex(p[4]));AnimFrame first=new AnimFrame(HexFormat.of().parseHex(p[2]),base),next=new AnimFrame(HexFormat.of().parseHex(p[3]),base);g.applyAnimation(new AnimFrameset(first),0,new AnimFrameset(next),0,Integer.parseInt(p[5]),Integer.parseInt(p[6]),Boolean.parseBoolean(p[7]));if(n.resizeX!=128||n.resizeY!=128)g.resize(n.resizeX,n.resizeY,n.resizeX);
int beforeY=g.getMinY();
if(p.length>8){int[]a=ints(p[8]);FormTerrain t=new FormTerrain();t.bas=BasTypeList.get(a[0]);t.xFine=a[2];t.yFine=a[3];t.tileHeight=a[4];FormGround.index=0;FormGround.calls.clear();FormGround.heights=Arrays.copyOfRange(a,5,9);t.alignToTerrain(g,a[1]);}
ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);for(int i=0;i<m.vertexCount;i++){out.writeInt(g.vertexX[i]);out.writeInt(g.vertexY[i]);out.writeInt(g.vertexZ[i]);}int[]lookup=new int[m.triangleCount];Arrays.fill(lookup,-1);for(int i=0;i<g.triangleCount;i++)lookup[g.triangleSources[i]&65535]=i;for(int i=0;i<m.triangleCount;i++){int index=lookup[i];if(index>=0)for(int v:new int[]{g.triangleVertexA[index]&65535,g.triangleVertexB[index]&65535,g.triangleVertexC[index]&65535}){out.writeInt(g.normalX[v]);out.writeInt(g.normalY[v]);out.writeInt(g.normalZ[v]);out.writeInt(g.normalMagnitude[v]);}}String digest=VisualOracle.digest(bytes.toByteArray());
if(p.length>8){System.out.print("{\"sha256\":\""+digest+"\",\"beforeY\":"+beforeY+",\"afterY\":"+g.getMinY()+",\"terrain\":["+FormTerrain.terrainPitchAngle+","+FormTerrain.terrainRollAngle+","+FormTerrain.terrainYOffset+"],\"calls\":[");for(int i=0;i<FormGround.calls.size();i++){if(i>0)System.out.print(",");System.out.print(Arrays.toString(FormGround.calls.get(i)));}System.out.println("]}");}
else System.out.println(digest);break;}
default:throw new AssertionError(p[0]);}}}}
'''

if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    generate(parser.parse_args().check)
