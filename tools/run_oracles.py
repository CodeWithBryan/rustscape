"""Run the oracle checks listed in oracle/ci.toml, several at a time.

An entry may start once every earlier entry that touches one of the same
runs/ folders has finished, so the outcome is the one of running them in
manifest order. The folders an entry touches are read from its generator:
the runs/ folders it and the modules beside it name, and those of the
generators it imports (an
imported generator usually re-runs its compile into its own folder). An
entry whose folders cannot be worked out (a command implemented in Rust
alone, a generator naming runs/ itself) waits for everything before it, and
everything after waits for it. The pinned Kotlin compiler folder is shared
read-only after the exclusive entry that downloads it.

Every entry runs; failures are listed at the end and make the exit status 1.

    cargo xtask oracles --check [--jobs N] [--plan] [--manifest FILE]
    python tools/run_oracles.py [same options]

--plan prints each entry's folders and the entries it waits for, then exits.
"""
import argparse
import concurrent.futures
import json
import os
import pathlib
import re
import subprocess
import sys
import threading
import time
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
SHARED = {"kotlin-1.8.20"}
GROUPS = os.environ.get("GITHUB_ACTIONS") == "true"


def generator_folders(folder, seen):
    """runs/ folders oracle/<folder>/generate.py touches, or None if unknown."""
    if folder in seen:
        return set()
    seen.add(folder)
    path = ROOT / "oracle" / folder / "generate.py"
    if not path.is_file():
        return None
    # The generator and the helper modules beside it (oracle/sql/database.py).
    text = "\n".join(p.read_text(encoding="utf-8") for p in sorted(path.parent.glob("*.py")))
    named = set(re.findall(r"runs/([A-Za-z0-9_.-]+)", text))
    if "." in named:
        return None
    folders = named - SHARED
    for other in set(re.findall(r"oracle/([a-z0-9-]+)/generate\.py", text)) - {"login", folder}:
        sub = generator_folders(other, seen)
        if sub is None:
            return None
        folders |= sub
    return folders


def command_folders(command, main_rs):
    """Folders an xtask command touches, or None if unknown."""
    arm = re.search(
        r'Some\("' + re.escape(command) + r'"\)\s*=>\s*\{?\s*([a-z_]+)::[a-z_]+\(([^)]*)\)',
        main_rs,
    )
    if not arm:
        # xtask's fallback arm: `<topic>-oracle` runs oracle/<topic>/generate.py
        # (content ports laid out by tools/content_scaffold.py).
        topic = command[: -len("-oracle")] if command.endswith("-oracle") else None
        if topic and (ROOT / "oracle" / topic / "generate.py").is_file():
            return generator_folders(topic, set())
        return None
    module, args = arm.groups()
    if module == "profile_oracle":
        folder = re.search(r'"([a-z0-9-]+)"', args)
        return generator_folders(folder.group(1), set()) if folder else None
    source = ROOT / "xtask/src" / f"{module}.rs"
    if not source.is_file():
        return None
    wrapped = set(re.findall(r"oracle/([a-z0-9-]+)/generate\.py", source.read_text(encoding="utf-8")))
    return generator_folders(wrapped.pop(), set()) if len(wrapped) == 1 else None


def load(manifest):
    entries = tomllib.loads(manifest.read_text(encoding="utf-8"))["check"]
    main_rs = (ROOT / "xtask/src/main.rs").read_text(encoding="utf-8")
    for entry in entries:
        for tool in entry.get("tools", []):
            if not (ROOT / tool).is_file():
                raise SystemExit(f"{manifest}: {entry['name']}: missing tool {tool}")
        if f'Some("{entry["xtask"][0]}")' not in main_rs:
            raise SystemExit(f"{manifest}: {entry['name']}: unknown xtask command {entry['xtask'][0]}")
        entry["folders"] = None if entry.get("exclusive") else command_folders(entry["xtask"][0], main_rs)
    for i, entry in enumerate(entries):
        entry["waits"] = [
            j
            for j in range(i)
            if entry["folders"] is None
            or entries[j]["folders"] is None
            or entry["folders"] & entries[j]["folders"]
        ]
    return entries


def build_xtask():
    """Build xtask once and return its executable (not `cargo run` per entry)."""
    result = subprocess.run(
        ["cargo", "build", "--locked", "-p", "xtask", "--message-format=json-render-diagnostics"],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        check=True,
    )
    for line in result.stdout.decode().splitlines():
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message["target"]["name"] == "xtask" and message.get("executable"):
            return message["executable"]
    raise SystemExit("cargo did not report the xtask executable")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="accepted for symmetry: every entry already checks")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    parser.add_argument("--plan", action="store_true")
    parser.add_argument("--manifest", type=pathlib.Path, default=ROOT / "oracle/ci.toml")
    options = parser.parse_args()
    entries = load(options.manifest)

    if options.plan:
        for i, entry in enumerate(entries):
            folders = "everything (barrier)" if entry["folders"] is None else ", ".join(sorted(entry["folders"])) or "-"
            waits = [entries[j]["xtask"][0] for j in entry["waits"]]
            print(f"{i + 1:3} {entry['xtask'][0]}\n      folders: {folders}\n      waits for: {', '.join(waits) or '-'}")
        return 0

    xtask = build_xtask()
    done = [threading.Event() for _ in entries]
    results = [None] * len(entries)
    lock = threading.Lock()

    def run(i):
        entry = entries[i]
        for j in entry["waits"]:
            done[j].wait()
        start = time.perf_counter()
        commands = [[xtask, *entry["xtask"]]] + [[sys.executable, tool, "--check"] for tool in entry.get("tools", [])]
        output, ok = [], True
        for command in commands:
            process = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            output.append(f"$ {' '.join(map(str, command[1:] if command[0] == xtask else command))}\n")
            output.append(process.stdout.decode("utf-8", errors="replace"))
            if process.returncode:
                ok = False
                break
        seconds = time.perf_counter() - start
        results[i] = (ok, seconds)
        with lock:
            status = "ok  " if ok else "FAIL"
            if GROUPS:
                print(f"::group::{status} {seconds:6.1f}s {entry['name']}")
                print("".join(output).rstrip())
                print("::endgroup::")
                if not ok:
                    print(f"::error title=Oracle check failed::{entry['name']}")
            else:
                print(f"{status} {seconds:6.1f}s {entry['name']}")
                if not ok:
                    print("".join(output).rstrip())
            sys.stdout.flush()
        done[i].set()

    start = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, options.jobs)) as pool:
        list(pool.map(run, range(len(entries))))
    wall = time.perf_counter() - start

    failed = [entries[i]["name"] for i, (ok, _) in enumerate(results) if not ok]
    total = sum(seconds for _, seconds in results)
    print(f"\n{len(entries) - len(failed)} of {len(entries)} oracle checks passed in {wall:.0f}s "
          f"({total:.0f}s of work, {options.jobs} at a time). Slowest:")
    for i in sorted(range(len(entries)), key=lambda i: -results[i][1])[:5]:
        print(f"  {results[i][1]:6.1f}s {entries[i]['name']}")
    for name in failed:
        print(f"FAILED: {name}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
