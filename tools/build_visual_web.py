"""Build/stage the same Rust visual client for a local browser. No remote hosting."""
import argparse
import http.server
import hashlib
import json
import pathlib
import shutil
import subprocess

ROOT=pathlib.Path(__file__).resolve().parents[1]
OUT=ROOT/"runs/visual-web"
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--serve",action="store_true")
parser.add_argument("--port",type=int,default=8099)
parser.add_argument("--pack",type=pathlib.Path,default=ROOT/"assets/visual/demo.pack")
args=parser.parse_args()
subprocess.run(["cargo","build","-p","rs09-client","-p","rs09-audio-worklet","--release","--target","wasm32-unknown-unknown"],cwd=ROOT,check=True)
metadata=json.loads(subprocess.check_output(["cargo","metadata","--format-version","1"],cwd=ROOT))
macroquad=next(pathlib.Path(p["manifest_path"]).parent for p in metadata["packages"] if p["name"]=="macroquad")
(OUT/"assets/visual").mkdir(parents=True,exist_ok=True)
shutil.copyfile(pathlib.Path(metadata["target_directory"])/"wasm32-unknown-unknown/release/rs09-client.wasm",OUT/"client.wasm")
shutil.copyfile(pathlib.Path(metadata["target_directory"])/"wasm32-unknown-unknown/release/rs09_audio_worklet.wasm",OUT/"audio_worklet.wasm")
bundle=(macroquad/"js/mq_js_bundle.js").read_bytes().decode("utf-8")
# The pinned bundle's quad_net callback assignment runs under strict mode.
# Declare that callback inside its IIFE; retain the original comma expression.
unbound="}register_plugin=function(e){e.env.ws_connect"
if unbound in bundle:
    assert bundle.count(unbound)==1
    bundle=bundle.replace(unbound,"}var register_plugin;register_plugin=function(e){e.env.ws_connect")
(OUT/"mq_js_bundle.js").write_bytes(bundle.encode("utf-8"))
for name in ["LICENSE","THIRD_PARTY_NOTICES.md","upstream.lock.json"]:shutil.copyfile(ROOT/name,OUT/name)
version=hashlib.sha256(b''.join((OUT/name).read_bytes()for name in ['client.wasm','audio_worklet.wasm'])+b''.join((ROOT/'web/visual'/name).read_bytes()for name in ['game_transport.js','cache_storage.js','pcm_audio_worklet.js','pcm_worklet.js','graphics_lifecycle.js'])).hexdigest()[:16]
(OUT/"index.html").write_text((ROOT/"web/visual/index.html").read_text(encoding="utf-8").replace("__CLIENT_VERSION__",version),encoding="utf-8")
shutil.copyfile(ROOT/"web/visual/game_transport.js",OUT/"game_transport.js")
for name in ['cache_storage.js','pcm_audio_worklet.js','pcm_worklet.js','graphics_lifecycle.js']:shutil.copyfile(ROOT/'web/visual'/name,OUT/name)
# Optional: region keys for the starting scene when it is built from the downloaded cache.
if (ROOT/"runs/player-session/xteas.json").exists():shutil.copyfile(ROOT/"runs/player-session/xteas.json",OUT/"assets/visual/xteas.json")
shutil.copyfile(args.pack,OUT/"assets/visual/demo.pack")
shutil.copyfile(ROOT/"assets/visual/ui-players.pack",OUT/"assets/visual/ui-players.pack")
shutil.copyfile(ROOT/"assets/visual/animations.pack",OUT/"assets/visual/animations.pack")
for gender in ['male','female']:shutil.copyfile(ROOT/f"runs/visual/player-{gender}.bin",OUT/f"assets/visual/player-{gender}.bin")
for source in (ROOT/'fixtures/players/forms').iterdir():
    if source.is_file():
        target=OUT/'assets/visual/forms'/source.name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
shutil.copyfile(ROOT/"fixtures/players/world-spots-replay.json",OUT/"assets/visual/world-spots-replay.json")
shutil.copyfile(ROOT/"fixtures/players/npc-render-replay.json",OUT/"assets/visual/npc-render-replay.json")
shutil.copyfile(ROOT/"fixtures/players/npc-overheads-replay.json",OUT/"assets/visual/npc-overheads-replay.json")
shutil.copyfile(ROOT/"fixtures/players/npc-nearby-replay.json",OUT/"assets/visual/npc-nearby-replay.json")
shutil.copyfile(ROOT/"fixtures/players/slot-render-replay.json",OUT/"assets/visual/slot-render-replay.json")
for source,target in [("ui-preview.bin","ui-bank.bin"),("ui-shop.bin","ui-shop.bin"),("ui-gameframe.bin","ui-gameframe.bin")]:
    if source=="ui-gameframe.bin" and not (ROOT/"runs/visual"/source).exists():raise FileNotFoundError(f"Required offline browser gameframe: {ROOT/'runs/visual'/source}")
    if (ROOT/"runs/visual"/source).exists():shutil.copyfile(ROOT/"runs/visual"/source,OUT/"assets/visual"/target)
print(f"Browser artifact staged at {OUT}",flush=True)
if args.serve:
    class Handler(http.server.SimpleHTTPRequestHandler):
        extensions_map={**http.server.SimpleHTTPRequestHandler.extensions_map,".wasm":"application/wasm"}
        def __init__(self,*a,**kw):super().__init__(*a,directory=str(OUT),**kw)
    print(f"Open http://127.0.0.1:{args.port}",flush=True)
    http.server.ThreadingHTTPServer(("127.0.0.1",args.port),Handler).serve_forever()
