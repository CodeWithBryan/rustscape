"""Prepare an offline visual corpus from the pinned server's Git LFS cache.

Only packaging runs in Python. Rust decodes the original model/frame/sequence,
terrain and synthesizer bytes at runtime. No JVM or game server is required.
"""
import argparse
import bz2
import concurrent.futures
import gzip
import hashlib
import json
import pathlib
import struct
import subprocess
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
LOCK = json.loads((ROOT / "upstream.lock.json").read_text())
REPO = ROOT / "reference/server"
OUT = ROOT / "assets/visual"


def git_file(name):
    return subprocess.check_output(["git", "show", LOCK["server"]["commit"] + ":" + name], cwd=REPO)


def download_cache():
    cache = OUT / "cache"
    cache.mkdir(parents=True, exist_ok=True)
    objects = []
    for suffix in ["dat2"] + ["idx" + str(i) for i in [0, 1, 2, 4, 5, 7, 8, 9, 16, 18, 20, 26, 255]]:
        name = "main_file_cache." + suffix
        pointer = git_file("Server/data/cache/" + name).decode().splitlines()
        oid = next(line.split(":")[1] for line in pointer if line.startswith("oid "))
        size = int(next(line.split()[1] for line in pointer if line.startswith("size ")))
        path = cache / name
        if path.exists() and path.stat().st_size == size and hashlib.sha256(path.read_bytes()).hexdigest() == oid:
            continue
        objects.append({"oid": oid, "size": size, "path": path})
    if objects:
        body = {"operation": "download", "transfers": ["basic"], "objects": [{"oid": o["oid"], "size": o["size"]} for o in objects]}
        req = urllib.request.Request(LOCK["server"]["url"] + "/info/lfs/objects/batch", data=json.dumps(body).encode(), headers={"Content-Type": "application/vnd.git-lfs+json", "Accept": "application/vnd.git-lfs+json"})
        with urllib.request.urlopen(req) as response:
            actions = {o["oid"]: o for o in json.load(response)["objects"]}
        def fetch(obj):
            action = actions[obj["oid"]]["actions"]["download"]
            print("Downloading", obj["path"].name, obj["size"], flush=True)
            with urllib.request.urlopen(urllib.request.Request(action["href"], headers=action.get("header", {})), timeout=120) as response:
                data = response.read()
            if len(data) != obj["size"] or hashlib.sha256(data).hexdigest() != obj["oid"]:
                raise ValueError("LFS checksum mismatch")
            obj["path"].write_bytes(data)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            list(pool.map(fetch, objects))
    return cache


def verify_cache(path):
    for suffix in ["dat2"] + ["idx" + str(i) for i in [0,1,2,4,5,7,8,9,16,18,20,26,255]]:
        name="main_file_cache."+suffix
        pointer=git_file("Server/data/cache/"+name).decode().splitlines()
        expected=next(line.split(":")[1] for line in pointer if line.startswith("oid "))
        if hashlib.sha256((path/name).read_bytes()).hexdigest()!=expected:
            raise ValueError(f"{name} differs from pinned server cache; review cache changes explicitly")


class Reader:
    def __init__(self, data):
        self.data, self.pos = data, 0
    def take(self, n):
        b = self.data[self.pos:self.pos+n]
        if len(b) != n:
            raise ValueError("truncated asset")
        self.pos += n
        return b
    def u1(self): return self.take(1)[0]
    def u2(self): return int.from_bytes(self.take(2), "big")
    def u4(self): return int.from_bytes(self.take(4), "big")
    def smart(self): return self.u1() if self.data[self.pos] < 128 else self.u2()-32768
    def text(self):
        end = self.data.index(0, self.pos)
        text = self.take(end-self.pos).decode("cp1252")
        self.take(1)
        return text


def container(data, key=None):
    if len(data)<5:
        raise ValueError("truncated JS5 header")
    length=int.from_bytes(data[1:5],"big")
    end=length+(5 if data[0]==0 else 9)
    if end>len(data):
        raise ValueError("truncated JS5 payload")
    # The two-byte disk revision trailer is outside the encrypted container.
    data=data[:end]
    if key and any(key):
        data = bytearray(data)
        for off in range(5, len(data)-7, 8):
            a, b = struct.unpack_from(">II", data, off)
            total = (0x9E3779B9*32) & 0xFFFFFFFF
            for _ in range(32):
                b = (b - ((((a << 4) ^ (a >> 5)) + a) ^ (total + key[(total >> 11) & 3]))) & 0xFFFFFFFF
                total = (total - 0x9E3779B9) & 0xFFFFFFFF
                a = (a - ((((b << 4) ^ (b >> 5)) + b) ^ (total + key[total & 3]))) & 0xFFFFFFFF
            struct.pack_into(">II", data, off, a, b)
    r = Reader(data)
    kind, length = r.u1(), r.u4()
    if kind == 0: return r.take(length)
    unpacked = r.u4()
    payload = r.take(length)
    if kind == 1:
        result = bz2.decompress(b"BZh1"+payload)
    elif kind == 2 and payload[:2] == b"\x1f\x8b":
        result = gzip.decompress(payload)
    else:
        raise ValueError("unsupported or incorrectly decrypted container")
    if len(result) != unpacked: raise ValueError("container length mismatch")
    return result


def name_hash(name):
    h = 0
    for char in name: h = (h*31+ord(char)) & 0xFFFFFFFF
    return h


class Cache:
    def __init__(self, path):
        self.disk = (path/"main_file_cache.dat2").read_bytes()
        self.idx = {int(p.suffix[4:]): p.read_bytes() for p in path.glob("*.idx*")}
        self.indices, self.groups = {}, {}
    def raw(self, archive, group):
        entry = self.idx[archive][group*6:group*6+6]
        if len(entry) != 6: raise KeyError((archive, group))
        length, sector = int.from_bytes(entry[:3], "big"), int.from_bytes(entry[3:], "big")
        if not length or not sector: raise KeyError((archive, group))
        result, chunk = bytearray(), 0
        while len(result) < length:
            off = sector*520
            if group > 65535:
                gid, part = struct.unpack_from(">IH", self.disk, off)
                nxt, idx, head = int.from_bytes(self.disk[off+6:off+9], "big"), self.disk[off+9], 10
            else:
                gid, part = struct.unpack_from(">HH", self.disk, off)
                nxt, idx, head = int.from_bytes(self.disk[off+4:off+7], "big"), self.disk[off+7], 8
            if (gid, part, idx) != (group, chunk, archive): raise ValueError("invalid cache sector")
            result.extend(self.disk[off+head:off+head+min(520-head, length-len(result))])
            sector, chunk = nxt, chunk+1
        return bytes(result)
    def index(self, archive):
        if archive in self.indices: return self.indices[archive]
        r = Reader(container(self.raw(255, archive)))
        protocol = r.u1()
        if protocol >= 6: r.u4()
        flags, count = r.u1(), r.u2()
        if protocol not in (5, 6) or flags not in (0, 1): raise ValueError("unsupported JS5 index")
        ids, last = [], 0
        for _ in range(count):
            last += r.u2()
            ids.append(last)
        names = {r.u4(): gid for gid in ids} if flags else {}
        r.take(count*8)  # CRCs and versions
        sizes = [r.u2() for _ in ids]
        files = {}
        for gid, size in zip(ids, sizes):
            last, fids = 0, []
            for _ in range(size):
                last += r.u2()
                fids.append(last)
            files[gid] = fids
        self.indices[archive] = (names, files)
        return names, files
    def group(self, archive, group, key=None):
        tag = (archive, group)
        if tag in self.groups: return self.groups[tag]
        data = container(self.raw(archive, group), key)
        fids = self.index(archive)[1][group]
        if len(fids) == 1: result = {fids[0]: data}
        else:
            chunks, count = data[-1], len(fids)
            table = len(data)-1-chunks*count*4
            r, arrays, offset = Reader(data[table:]), [bytearray() for _ in fids], 0
            for _ in range(chunks):
                length = 0
                for i in range(count):
                    length += struct.unpack(">i", r.take(4))[0]
                    arrays[i].extend(data[offset:offset+length])
                    offset += length
            if offset != table: raise ValueError("JS5 stripe mismatch")
            result = dict(zip(fids, map(bytes, arrays)))
        self.groups[tag] = result
        return result
    def file(self, a, g, f=0): return self.group(a,g)[f]


def loc_def(data):
    r = Reader(data)
    d = dict(models=[], shapes=None, name="", width=1, length=1, resize=[128]*3, offset=[0]*3, mirror=False, recolor=[], retexture=[], ambient=0, contrast=0, hill=False, animation=-1, randomized_animation=True, cast_shadow=True, blockwalk=2, blockrange=True, breakroute=False)
    while True:
        op = r.u1()
        if op == 0:
            if d["breakroute"]: d["blockwalk"],d["blockrange"] = 0,False
            return d
        if op == 1:
            entries = [(r.u2(),r.u1()) for _ in range(r.u1())]
            if not d["models"]: d["models"], d["shapes"] = [v[0] for v in entries], [v[1] for v in entries]
        elif op == 5:
            models = [r.u2() for _ in range(r.u1())]
            if not d["models"]: d["models"] = models
        elif op == 2: d["name"] = r.text()
        elif op in (14,15): d["width" if op == 14 else "length"] = r.u1()
        elif op in (65,66,67): d["resize"][op-65] = r.u2()
        elif op in (70,71,72): d["offset"][op-70] = struct.unpack(">h",r.take(2))[0]
        elif op == 62: d["mirror"] = True
        elif op == 21: d["hill"] = True
        elif op == 24: d["animation"] = r.u2()
        elif op == 89: d["randomized_animation"] = False
        elif op == 88: d["cast_shadow"] = False
        elif op == 17: d["blockwalk"],d["blockrange"] = 0,False
        elif op == 18: d["blockrange"] = False
        elif op == 27: d["blockwalk"] = 1
        elif op == 74: d["breakroute"] = True
        elif 30 <= op < 35: r.text()
        elif op in (40,41):
            pairs = [[r.u2(),r.u2()] for _ in range(r.u1())]
            d["recolor" if op==40 else "retexture"] = pairs
        elif op == 42: r.take(r.u1())
        elif op in (77,92):
            r.take(4)
            fallback=r.u2() if op==92 else 65535
            d['children']=[r.u2()for _ in range(r.u1()+1)]+[fallback]
        elif op == 78: r.take(3)
        elif op == 79: r.take(5); r.take(r.u1()*2)
        elif op in (99,100): r.take(3)
        elif op == 249:
            for _ in range(r.u1()):
                string = r.u1(); r.take(3)
                if string: r.text()
                else: r.take(4)
        elif op in (29,39): d["ambient" if op==29 else "contrast"] = struct.unpack("b",r.take(1))[0]
        elif op in (19,28,69,75,81,101): r.take(1)
        elif op in (60,93,102): r.take(2)
        elif op in (22,23,64,73,82,90,91,94,95,96,97,98): pass
        else: raise ValueError(f"unknown location opcode {op}")


def npc_dependencies(data):
    """Discover raw model/BAS/morph dependencies; Rust owns runtime selection."""
    r=Reader(data);models=set();children=set();bas=-1
    while (op:=r.u1()):
        if op==1:models.update(v for v in (r.u2()for _ in range(r.u1()))if v!=65535)
        elif op==127:bas=r.u2()
        elif op==2 or 30<=op<=34:r.text()
        elif op in (12,100,101,119,125,128):r.take(1)
        elif op in (40,41):r.take(r.u1()*4)
        elif op in (42,60):r.take(r.u1()*(2 if op==60 else 1))
        elif op in (95,97,98,102,103,114,115,122,123,126,137):r.take(2)
        elif op==113:r.take(4)
        elif op in (93,99,107,109,111):pass
        elif op in (106,118):
            r.take(4)
            if op==118:children.add(r.u2())
            children.update(r.u2()for _ in range(r.u1()+1))
        elif op==121:r.take(r.u1()*4)
        elif op==134:r.take(9)
        elif op in (135,136):r.take(3)
        elif op==249:
            for _ in range(r.u1()):
                text=r.u1();r.take(3)
                if text:r.text()
                else:r.take(4)
        # The pinned NpcType decoder ignores unknown opcode bytes, including138.
    return models,bas,children-{65535}

def prepare(cache, center=(3222,3218), label="Lumbridge", output=None, login_window=False, npc_types=None, region_bounds=None, region_npcs=False):
    entries = {}
    def add(name, data): entries[name] = data; return data
    locs, defs, models, seqs = [], {}, set(), {808,819,820,821,822,823,824,863,866,859}
    keys_raw = git_file("Server/data/configs/xteas.json")
    keys = json.loads(keys_raw)
    if isinstance(keys, dict) and "xteas" in keys:
        keys = {k["regionId"]: [int(v) for v in k["keys"].split(",")] for k in keys["xteas"]}
    # The pinned server stores region IDs as object keys.
    if isinstance(keys, list): keys = {str(k["region"]): k.get("keys",k.get("key")) for k in keys}
    first=[(n-32)//64 for n in center]
    regions = [(x,z) for x in range(first[0],first[0]+2) for z in range(first[1],first[1]+2)]
    if login_window:
        zone=[n>>3 for n in center]
        regions=[(x,z) for x in range((zone[0]-6)//8,(zone[0]+6)//8+1) for z in range((zone[1]-6)//8,(zone[1]+6)//8+1)]
    if region_bounds is not None:
        west,south,east,north=region_bounds
        regions=[(x,z) for x in range(west,east+1) for z in range(south,north+1)]
    names = cache.index(5)[0]
    for x,z in regions:
        add(f"terrain/{x}_{z}",cache.file(5,names[name_hash(f"m{x}_{z}")]))
        gid = names[name_hash(f"l{x}_{z}")]
        raw = cache.group(5,gid,keys.get(str((x<<8)|z)))[0]
        add(f"locations/{x}_{z}",raw)
        r, loc = Reader(raw), -1
        while True:
            delta = r.smart()
            if not delta: break
            while delta == 32767:
                loc += delta; delta = r.smart()
            loc += delta
            position = 0
            while True:
                delta = r.smart()
                if not delta: break
                position += delta-1
                info = r.u1()
                plane, lx, lz = position >> 12, position >> 6 & 63, position & 63
                locs.append([loc,x*64+lx,z*64+lz,plane,info>>2,info&3])
                if loc not in defs:
                    definition = cache.file(16,loc>>8,loc&255)
                    add(f"loc/{loc}",definition)
                    defs[loc] = loc_def(definition)
                    models.update(defs[loc]["models"])
                    if defs[loc]["animation"] not in (-1,65535): seqs.add(defs[loc]["animation"])
    # Replacement definitions/models are dependencies only. Door rules remain on
    # the authoritative server; the client accepts its original zone updates.
    door_source=git_file("Server/data/configs/door_configs.json")
    replacements={}
    for row in json.loads(door_source):
        a,b=int(row["id"]),int(row["replaceId"])
        replacements.setdefault(a,set()).add(b)
        replacements.setdefault(b,set()).add(a)
    pending=list(defs)
    while pending:
        for loc in sorted(replacements.get(pending.pop(),set())):
            if loc in defs:continue
            raw=cache.file(16,loc>>8,loc&255)
            add(f"loc/{loc}",raw);defs[loc]=loc_def(raw);pending.append(loc)
            models.update(defs[loc]["models"])
            if defs[loc]["animation"] not in (-1,65535):seqs.add(defs[loc]["animation"])
    # Minimap icon morphs need their actual raw child metadata, without expanding
    # the selected 3D model corpus to unrelated dynamic-body assets.
    seen=set(defs);pending=list(defs)
    while pending:
        id=pending.pop()
        raw=entries[f'loc/{id}']
        for child in loc_def(raw).get('children',[]):
            if child==65535 or child in seen:continue
            seen.add(child);add(f'loc/{child}',cache.file(16,child>>8,child&255));pending.append(child)
    # PlayerAppearance assembles equipment slots in ascending order.
    avatar = [18,26,36,0,33,42,10]
    avatar_models = []
    for kit in avatar:
        raw=cache.file(2,3,kit)
        add(f"kit/{kit}",raw)
        r = Reader(raw)
        while (op := r.u1()):
            if op == 1: r.take(1)
            elif op == 2: avatar_models.extend(r.u2() for _ in range(r.u1()))
            elif op == 3: pass
            elif op in (40,41): r.take(r.u1()*4)
            elif 60 <= op < 70: r.take(2)
            else: raise ValueError(f"unknown identity-kit opcode {op}")
    models.update(avatar_models)
    # Raw NPC definitions and BAS files are interpreted by Rust at runtime.
    # This reader discovers asset dependencies only, without baking a posed mesh.
    npc_roots = set([0,1,4,9,41,47,81,100]+(npc_types or []))
    spawn_source = git_file("Server/data/configs/npc_spawns.json")
    npc_spawns = []
    region_types = set()
    # Direction.values() ordinal from the pinned server, converted to RT4 yaw.
    directions = [768,1024,1280,512,1536,256,0,1792]
    for row in json.loads(spawn_source):
        npc = int(row["npc_id"])
        for data in filter(None, row["loc_data"].split("-")):
            x,z,plane,walk,direction = map(int, data.strip("{}").split(","))
            if (x//64,z//64) in regions:
                region_types.add(npc)
                if plane == 0 and npc in npc_roots:
                    npc_spawns.append([npc,x,z,walk,directions[direction]])
    npc_ids = set()
    pending = sorted(npc_roots, reverse=True)
    live_roots = sorted(region_types, reverse=True) if region_npcs else []
    preview_ids = None
    # Keep raw definitions for every live NPC index/type. Only selected body assets
    # are preloaded below; missing body/BAS assets remain explicit runtime diagnostics.
    for group,files in cache.index(18)[1].items():
        for file in files:add(f"npc/{group*128+file}",cache.file(18,group,file))
    add("npc-catalog-complete",b"")
    while pending or live_roots:
        if not pending:
            # Streamed actors are assembled on demand. Keep the offline preview
            # catalogue limited to the models it actually instantiates.
            preview_ids = sorted(npc_ids)
            pending, live_roots = live_roots, []
        npc=pending.pop()
        if npc in npc_ids:continue
        npc_ids.add(npc)
        raw=add(f"npc/{npc}",cache.file(18,npc>>7,npc&127))
        body_models,bas,children=npc_dependencies(raw);models.update(body_models)
        pending.extend(sorted(children-npc_ids,reverse=True))
        if bas==-1:continue
        r=Reader(add(f"bas/{bas}",cache.file(2,32,bas)))
        while (op:=r.u1()):
            if op==1: seqs.update(v for v in [r.u2(),r.u2()] if v!=65535)
            elif op in (*range(2,10),38,39,40,41,42): seqs.add(r.u2())
            elif op==26: r.take(2)
            elif op==27:r.take(13)
            elif op in (29,31,34,37):r.take(1)
            elif op in (30,32,33,35,36,43,44,45):r.take(2)
    npc_ids = sorted(npc_ids)
    missing = []
    for model in sorted(models):
        try: add(f"model/{model}",cache.file(7,model))
        except KeyError: missing.append(model)
    for group,name in [(1,"underlay"),(4,"overlay")]:
        for fid, data in cache.group(2,group).items(): add(f"{name}/{fid}",data)
    if output is not None and 31 in cache.index(2)[1]:
        for fid,data in cache.group(2,31).items():add(f"light/{fid}",data)
    frame_groups = set()
    add("bas/1426",cache.file(2,32,1426))
    for seq in sorted(seqs):
        raw = cache.file(20,seq>>7,seq&127)
        add(f"seq/{seq}",raw)
        # frame array is opcode 1, preceding all other sequence fields.
        r = Reader(raw)
        first = r.u1()
        while first != 1 and first != 0:
            if first in (2,6,7): r.take(2)
            elif first in (5,8,9,10,11): r.take(1)
            elif first in (4,14,15,16): pass
            elif first == 3: r.take(r.u1())
            else: raise ValueError(f"sequence {seq} unexpected opcode {first} before frames")
            first = r.u1()
        if first == 0:
            continue
        if first != 1: raise ValueError(f"sequence {seq} starts with opcode {first}")
        n = r.u2(); r.take(n*4)
        frame_groups.update(r.u2() for _ in range(n))
    bases = set()
    for group in sorted(frame_groups):
        for fid,data in cache.group(0,group).items():
            add(f"frame/{group}/{fid}",data)
            bases.add(int.from_bytes(data[:2],"big"))
    for base in sorted(bases): add(f"base/{base}",cache.file(1,base))
    add("materials",cache.file(26,0))
    for gid in cache.index(9)[1]:
        add(f"texture/{gid}",cache.file(9,gid))
    for gid in cache.index(8)[1]:
        add(f"sprite/{gid}",cache.file(8,gid))
    for id,data in sorted(cache.group(2,34).items()):
        add(f"mapscene/{id}",data)
    minimap_ids={}
    for name in ['mapfunction','mapdots','mapflag','compass','hint_mapmarkers','hint_mapedge']:
        h=0
        for byte in name.encode('latin1'):h=(h*31+byte)&0xffffffff
        minimap_ids[name]=cache.index(8)[0][h]
    add('minimap/assets',json.dumps(minimap_ids,separators=(',',':')).encode())
    # Real synthesized effects are decoded to PCM in Rust, on demand.
    for sound in [0,1,2,3,4,5,6,7,8,9,10,127,223,224,380,385,386,387]:
        try: add(f"sound/{sound}",cache.file(4,sound))
        except KeyError: pass
    scene = dict(regions=regions, locations=locs, definitions=defs, avatar_models=avatar_models, avatar_kits=avatar, missing_models=missing,npc_types=preview_ids or npc_ids,npc_spawns=npc_spawns)
    if output is not None:scene.update(name=label,start=center)
    add("scene.json",json.dumps(scene,separators=(",",":")).encode())
    spans, blob = {}, bytearray()
    for name,data in entries.items():
        spans[name] = [len(blob),len(data)]
        blob.extend(data)
    source = "Pinned 2009scape cache / " + LOCK["server"]["commit"]
    manifest = json.dumps(dict(source=source,files=spans),separators=(",",":")).encode()
    path=output or OUT/"demo.pack"
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_bytes(b"RS09VIS1"+struct.pack("<I",len(manifest))+manifest+blob)
    provenance = dict(server=LOCK["server"],client=LOCK["client"],pack_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),assets=len(entries),models=len(models)-len(missing),locations=len(locs),missing_models=missing,npc_types=npc_ids,npc_spawns=len(npc_spawns),npc_spawn_source_sha256=hashlib.sha256(spawn_source).hexdigest())
    provenance_path=path.with_suffix(".provenance.json") if output else OUT/"provenance.json"
    provenance_path.write_text(json.dumps(provenance,indent=2)+"\n")
    print(json.dumps(provenance,indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache",type=pathlib.Path,help="Use an existing pinned cache instead of downloading Git LFS files")
    parser.add_argument("--center",type=int,nargs=2,default=[3222,3218],metavar=("X","Z"))
    parser.add_argument("--label",default="Lumbridge")
    parser.add_argument("--output",type=pathlib.Path,help="Write a separate tour pack; preserve the default Lumbridge corpus")
    parser.add_argument("--login-window",action="store_true",help="Include the original normal rebuild's complete region enumeration")
    parser.add_argument("--npc-types",type=int,nargs='+',help="Additional body asset roots; include complete morph dependency closure")
    parser.add_argument("--region-npcs", action="store_true", help="Include body and movement assets for original live NPC spawns on all planes of the packed regions")
    parser.add_argument("--region-bounds",type=int,nargs=4,metavar=("WEST","SOUTH","EAST","NORTH"),help="Inclusive region coordinates for a standalone visual pack (up to 4 by 4)")
    args = parser.parse_args()
    cache_path=args.cache or download_cache()
    verify_cache(cache_path)
    if args.region_bounds is not None:
        west,south,east,north=args.region_bounds
        if (args.login_window or not all(n in range(256) for n in args.region_bounds)
                or not (0 <= east-west < 4 and 0 <= north-south < 4)
                or not (west <= args.center[0]//64 <= east and south <= args.center[1]//64 <= north)):
            parser.error("region bounds must contain the center and span up to 4 by 4, without --login-window")
    if (args.center!=[3222,3218] or args.label!="Lumbridge" or args.login_window or args.npc_types or args.region_bounds is not None or args.region_npcs) and args.output is None:
        parser.error("alternate map locations require --output to preserve the default regression corpus")
    prepare(Cache(cache_path),args.center,args.label,args.output,args.login_window,args.npc_types,args.region_bounds,args.region_npcs)
