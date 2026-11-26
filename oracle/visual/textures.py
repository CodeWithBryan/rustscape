"""Pinned RT4 texture graphs, executed only as a differential test oracle."""
import hashlib
import json
import pathlib
import argparse
import shutil
import struct
import subprocess
from generate import clean, method, ROOT, SRC
def _recorded(root,name):
    # The packs the JVM fixtures were recorded from: kept under recorded/ once
    # the default packs are rebuilt natively (crates/client/tests/support/packs.rs).
    kept=root/'assets/visual/recorded'/name
    return kept if kept.is_file() else root/'assets/visual'/name

RUN = ROOT / "runs/texture-oracle"

def main(check=False):
    RUN.mkdir(parents=True, exist_ok=True)
    sources = {}
    def source(name):
        raw = (SRC / (name + ".java")).read_bytes().replace(b"\r\n", b"\n")
        sources[name + ".java"] = hashlib.sha256(raw).hexdigest()
        return clean(raw.decode())
    names = {p.stem for p in SRC.glob("TextureOp*.java")} | {
        "Texture", "TextureProvider", "Node", "LinkedList", "ArrayUtils", "IntUtils",
        "RandomUtils", "ColorImageCache", "MonochromeImageCache",
        "ColorImageCacheEntry", "MonochromeImageCacheEntry", "LruHashTable",
        "HashTable", "SecondaryLinkedList", "SecondaryNode", "ByteArrayNode", "ByteArrayNodeSecondary",
    }
    for name in sorted(names):
        (RUN / (name + ".java")).write_text(source(name))
    light=source("Light")
    (RUN/"LightNoiseOracle.java").write_text("class LightNoiseOracle {"+method(light,"public static int[] craeteNoise(")+"public static void main(String[] args)throws Exception{int[] noise=craeteNoise(0.4F);java.nio.ByteBuffer b=java.nio.ByteBuffer.allocate(noise.length*4);for(int v:noise)b.putInt(v);java.nio.file.Files.write(java.nio.file.Path.of(args[0]),b.array());}}")
    sprite = source("SpriteLoader")
    (RUN / "SpriteLoader.java").write_text("class SpriteLoader { static int frames,width,height;static int[] innerHeights,innerWidths,xOffsets,yOffsets,palette;static boolean[] hasAlpha;static byte[][] alpha,pixels;" + method(sprite, "public static void decode(byte[] data)") + """
    static SoftwareSprite loadSoftwareSpriteAutoDetect(Js5 archive,int id){return loadSoftwareSprite(id,archive,0);}
    static SoftwareSprite loadSoftwareSprite(int group,Js5 archive,int file){decode(archive.fetchFile(group,file));SoftwareSprite s=new SoftwareSprite();s.width=innerWidths[0];s.height=innerHeights[0];s.innerWidth=width;s.innerHeight=height;s.xOffset=xOffsets[0];s.yOffset=yOffsets[0];s.pixels=new int[s.width*s.height];for(int i=0;i<s.pixels.length;i++)s.pixels[i]=palette[pixels[0][i]&255];return s;}
    }
    """)
    software=source("SoftwareSprite")
    (RUN/"SoftwareSprite.java").write_text("class SoftwareSprite {int width,height,innerWidth,innerHeight,xOffset,yOffset;int[] pixels;"+method(software,"public final void trim()")+"}")
    shutil.copyfile(pathlib.Path(__file__).with_name("TextureOracle.java"), RUN / "TextureOracle.java")
    blob = _recorded(ROOT,'demo.pack').read_bytes()
    length = struct.unpack_from("<I", blob, 8)[0]
    manifest = json.loads(blob[12:12+length]); payload = blob[12+length:]
    for name, (off, size) in manifest["files"].items():
        if name.startswith(("texture/", "sprite/")) or name == "materials":
            (RUN / (name.replace("/", "_") + ".bin")).write_bytes(payload[off:off+size])
    (RUN / "sources.json").write_text(json.dumps(sources, indent=2) + "\n")
    subprocess.run(["javac", "-encoding", "UTF-8", "-d", str(RUN), *map(str,RUN.glob("*.java"))], check=True)
    result=subprocess.check_output(["java", "-cp", str(RUN), "TextureOracle", str(RUN)],text=True)
    subprocess.run(["java","-cp",str(RUN),"LightNoiseOracle",str(RUN/"light-noise.bin")],check=True)
    light_noise=(RUN/"light-noise.bin").read_bytes()
    cases=[dict(id=int(id),size=int(size),expected=digest) for id,size,digest in (row.split("\t") for row in result.splitlines())]
    wanted={"materials"}|{name for name in manifest["files"] if name.startswith("texture/")}|{f"sprite/{id}" for id in (RUN/"sprite_ids.txt").read_text().splitlines()}
    spans={};data=bytearray()
    for name in sorted(wanted):
        off,size=manifest["files"][name];spans[name]=[len(data),size];data.extend(payload[off:off+size])
    header=json.dumps(dict(source=manifest["source"],files=spans),separators=(",",":")).encode()
    pack=b"RS09VIS1"+struct.pack("<I",len(header))+header+data
    fixture=dict(format=1,client_commit=json.loads((ROOT/"upstream.lock.json").read_text())["client"]["commit"],sources=sources,pack_sha256=hashlib.sha256(pack).hexdigest(),light_noise_sha256=hashlib.sha256(light_noise).hexdigest(),cases=cases)
    fixture=(json.dumps(fixture,indent=2)+"\n").encode();dest=ROOT/"fixtures/visual"
    for name,data in [("textures.json",fixture),("textures.pack",pack),("light-noise.bin",light_noise)]:
        if check:
            if (dest/name).read_bytes()!=data:raise SystemExit("Texture oracle changed: "+name)
        else:(dest/name).write_bytes(data)
    print(f"Verified {len(cases)} JVM texture images; {len(pack)} bytes of raw cache fixtures")

if __name__ == "__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--check",action="store_true");main(parser.parse_args().check)
