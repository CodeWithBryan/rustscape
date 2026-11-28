"""Write the world folder `rs09-server --world` boots from, for local play (docs/play-locally.md).

The folder is made by the world-trace oracle's own world_template (oracle/world-trace/generate.py): the pinned
Server/data files and worldprops/default.conf with enable_bots=false. Only its output folder differs, so the
oracle's run folder and fixtures are left alone. Missing Server/data blobs are fetched by git from the pinned
commit into the partial reference/server clone (RS09_WORLD_TRACE_FETCH_DATA=1).

Usage: python tools/play_world.py OUTPUT_DIR
"""
import importlib.util
import json
import os
import pathlib
import shutil
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    target = pathlib.Path(sys.argv[1]).resolve()
    os.environ["RS09_WORLD_TRACE_FETCH_DATA"] = "1"
    spec = importlib.util.spec_from_file_location("world_trace", ROOT / "oracle/world-trace/generate.py")
    world_trace = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(world_trace)
    lock = json.loads((ROOT / "upstream.lock.json").read_text(encoding="utf-8"))
    # world_template writes RUN/"template"; point RUN at a staging folder beside the target.
    staging = target.parent / (target.name + ".partial")
    if staging.exists():
        shutil.rmtree(staging)
    world_trace.RUN = staging
    template, data, _config = world_trace.world_template(lock)
    if target.exists():
        shutil.rmtree(target)
    template.rename(target)
    shutil.rmtree(staging)
    print(f"Wrote {len(data)} pinned data files and worldprops/default.conf to {target}")


if __name__ == "__main__":
    main()
