"""Compile verbatim pinned login methods with Kotlin 1.8.20 and explicit I/O adapters.

Java/Kotlin are test oracles only. --check never updates fixtures or reference files.
Compiler artifacts are acquired over HTTPS and verified against kotlin.lock.json.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
import pathlib
import re
import subprocess
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / "runs/login-oracle"
SERVER = ROOT / "reference/server/Server/src/main"
CLIENT = ROOT / "reference/client/client/src/main/java/rt4"
SOURCES = {}
JAVA_HOME = None

def read(path):
    data = path.read_bytes().replace(b"\r", b"")
    SOURCES[path.relative_to(ROOT).as_posix()] = hashlib.sha256(data).hexdigest()
    return data.decode()

def run(args):
    if JAVA_HOME is not None and args[0] in ["java", "javac"]:
        args = [JAVA_HOME / "bin" / (args[0] + (".exe" if os.name == "nt" else "")), *args[1:]]
    # The Kotlin compiler's JVM stops at the C1 JIT: each compile is short, so
    # it is mostly start-up, and C2 never pays off (4.4 s to 2.4 s measured).
    # The classes it writes are byte for byte the same; only this compiler
    # process is affected, never an oracle's own JVM.
    if "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler" in map(str, args):
        args = [args[0], "-XX:TieredStopAtLevel=1", *args[1:]]
    result = subprocess.run(list(map(str, args)), cwd=ROOT, capture_output=True, check=False)
    if result.returncode:
        raise RuntimeError(result.stdout.decode(errors="replace") + result.stderr.decode(errors="replace"))
    return result

def method(text, signature):
    start = text.index(signature)
    opening = text.index("{", start)
    depth = 1
    end = opening + 1
    # Selected methods have no braces inside literals/comments. Preserve their bodies verbatim.
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[start:end]

def clean_annotations(text):
    return re.sub(r"@(?:OriginalArg|Pc)\([^)]*\)\s*", "", text)

def artifacts():
    lock = json.loads(read(HERE / "kotlin.lock.json"))
    folder = ROOT / "runs/kotlin-1.8.20"
    folder.mkdir(parents=True, exist_ok=True)
    def fetch(item):
        name, entry = item
        target = folder / name
        def valid():
            return target.is_file() and target.stat().st_size == entry["size"] and hashlib.sha256(target.read_bytes()).hexdigest() == entry["sha256"]
        if not valid():
            with urllib.request.urlopen(entry["url"], timeout=60) as response:
                data = response.read()
            if len(data) != entry["size"] or hashlib.sha256(data).hexdigest() != entry["sha256"]:
                raise RuntimeError(f"Kotlin artifact differs from pin: {name}")
            target.write_bytes(data)
        return target
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        return list(pool.map(fetch, sorted(lock.items())))

def generate(check, interop_client=None):
    lock = json.loads(read(ROOT / "upstream.lock.json"))
    for part in ["server", "client"]:
        path = ROOT / "reference" / part
        actual = run(["git", "-C", path, "rev-parse", "HEAD"]).stdout.decode().strip()
        if actual != lock[part]["commit"]:
            raise RuntimeError(f"{part} reference differs from pin")
        run(["git", "-C", path, "diff", "--quiet", "HEAD", "--"])
    jars = artifacts()
    RUN.mkdir(parents=True, exist_ok=True)
    java = RUN / "java"
    kotlin = RUN / "kotlin"
    classes = RUN / "classes"
    for path in [java, kotlin, classes]:
        path.mkdir(exist_ok=True)
    def write(folder, name, text):
        target = folder / name
        target.write_text(text, encoding="utf-8", newline="\n")
        return target
    java_inputs = []
    for name in ["Buffer", "Node", "GlobalConfig"]:
        source = CLIENT / (name + ".java")
        read(source)
        java_inputs.append(source)
    for source in sorted((ROOT / "reference/client/deob-annotations/src/main/java/org/openrs2/deob/annotation").glob("*.java")):
        read(source)
        java_inputs.append(source)
    constants = SERVER / "core/net/Constants.java"
    read(constants)
    java_inputs.append(constants)
    jag = read(CLIENT / "JagString.java")
    adapter = read(ROOT / "oracle/adapters/rt4/JagString.java")
    adapter = adapter[:adapter.rfind("}")]
    adapter += "public byte[] chars; public int length;\n" + clean_annotations(method(jag, "public final long encode37()")) + "\n}"
    adapter = adapter.replace("public byte[] chars;", 'public static JagString parse(String value){if(!value.isEmpty())throw new UnsupportedOperationException("Only empty literal used by selected login writer");return new JagString(new byte[0]);} public byte[] chars;')
    adapter = adapter.replace("this.bytes = bytes.clone();", "this.bytes = bytes.clone(); this.chars = this.bytes; this.length = bytes.length;")
    java_inputs.append(write(java, "JagString.java", adapter))
    strings = read(SERVER / "core/tools/StringUtils.java")
    alphabet = re.search(r"public static final char\[\] VALID_CHARS = .*?;", strings).group()
    java_inputs.append(write(java, "StringUtils.java", "package core.tools; public final class StringUtils {" + alphabet + method(strings, "public static String longToString(") + "}"))
    client_writer = read(CLIENT / "LoginManager.java")
    start = client_writer.index("Protocol.outboundBuffer.p1(10);")
    finish = client_writer.index("buffer.pdata(Protocol.outboundBuffer.data, Protocol.outboundBuffer.offset);", start)
    finish += len("buffer.pdata(Protocol.outboundBuffer.data, Protocol.outboundBuffer.offset);")
    body = client_writer[start:finish]
    support = read(HERE / "WriterSupport.java")
    support = support.replace("/* ORIGINAL_LOGIN_WRITER */", clean_annotations(body))
    support = support.replace("/* CHECKSUM_FIELDS */", "\n".join(f"public static Checksum js5Archive{i} = new Checksum({i});" for i in range(28)))
    java_inputs.append(write(java, "WriterSupport.java", support))
    run(["javac", "--release", "8", "-encoding", "UTF-8", "-d", classes, *java_inputs])
    # Only world/account integration is adapted. The selected Kotlin methods and
    # complete LoginInfo/AuthResponse/ISAAC/ByteBufferUtils classes stay unchanged.
    server_constants = read(SERVER / "core/ServerConstants.kt")
    exp = re.search(r'var EXPONENT = BigInteger\("([0-9]+)"\)', server_constants).group(1)
    mod = re.search(r'var MODULUS = BigInteger\("([0-9]+)"\)', server_constants).group(1)
    source = read(SERVER / "core/net/packet/in/Login.kt")
    constants = "\n".join(re.findall(r"(?:private )?const val (?:ENCRYPTION_VERIFICATION_BYTE|NORMAL_LOGIN_OP|RECONNECT_LOGIN_OP|CACHE_INDEX_COUNT)[^\n]*", source))
    selected = "\n".join(method(source, signature) for signature in ["fun decodeFromBuffer(", "private fun produceISAACPairFrom(", "@JvmStatic fun decryptRSABuffer(", "private fun noop("])
    header = """package core.net.packet.`in`
import core.cache.crypto.*
import core.cache.misc.buffer.ByteBufferUtils
import core.auth.AuthResponse
import core.ServerConstants
import core.net.Constants
import core.tools.*
import core.api.log
import core.game.world.repository.Repository
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.BufferUnderflowException
object Login {
"""
    kotlin_inputs = [write(kotlin, "Login.kt", header + constants + "\n" + selected + "\n}")]
    for name in ["core/auth/AuthResponse.kt", "core/cache/crypto/ISAACCipher.kt", "core/cache/crypto/ISAACPair.kt", "core/cache/misc/buffer/ByteBufferUtils.kt", "core/net/packet/in/LoginInfo.kt"]:
        path = SERVER / name
        read(path)
        kotlin_inputs.append(path)
    kotlin_inputs.append(write(kotlin, "ServerConstants.kt", f'package core\nimport java.math.BigInteger\nobject ServerConstants {{val EXPONENT=BigInteger("{exp}");val MODULUS=BigInteger("{mod}")}}'))
    for name in ["Repository.kt", "Logging.kt", "Log.kt"]:
        kotlin_inputs.append(write(kotlin, name, read(HERE / name)))
    compiler_cp = os.pathsep.join(map(str, jars))
    stdlib = next(path for path in jars if path.name.startswith("kotlin-stdlib-"))
    library_cp = os.pathsep.join(map(str, [classes, stdlib]))
    compiler = ["java", "-cp", compiler_cp, "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler", "-no-stdlib", "-no-reflect", "-jvm-target", "11", "-classpath", library_cp, "-d", classes]
    run([*compiler, *kotlin_inputs])
    reply_source = read(SERVER / "core/net/event/LoginWriteEvent.java")
    reply_adapter = read(HERE / "LoginReplies.java")
    reply_adapter = reply_adapter.replace("/* ORIGINAL_WRITE */", method(reply_source, "public void write("))
    reply_adapter = reply_adapter.replace("/* ORIGINAL_WORLD_REPLY */", method(reply_source, "private static ByteBuffer getWorldResponse("))
    reply_path = write(java, "LoginReplies.java", reply_adapter)
    run(["javac", "--release", "11", "-encoding", "UTF-8", "-cp", library_cp, "-d", classes, reply_path])
    harness = HERE / "LoginOracle.kt"
    read(harness)
    probe = HERE / "LoginProbe.kt"
    read(probe)
    read(pathlib.Path(__file__))
    run([*compiler, harness, probe])
    java_path = str(JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java")) if JAVA_HOME else "java"
    version = run(["java", "-version"])
    (RUN / "runtime.json").write_text(json.dumps({"java": java_path, "classpath": library_cp,
        "javaVersion": version.stderr.decode().strip()}), encoding="utf-8")
    data = run(["java", "-cp", library_cp, "LoginOracleKt"]).stdout.replace(b"\r", b"")
    cases = [json.loads(line) for line in data.splitlines() if line]
    provenance = {"format": 1, "upstream": lock, "sources": dict(sorted(SOURCES.items())),
        "caseCount": len(cases), "fixtureSha256": hashlib.sha256(data).hexdigest(),
        "sourceHashAlgorithm": "sha256, CR removed", "kotlinCompiler": "1.8.20",
        "scope": "Verbatim LoginManager packet-writing slice, selected original Kotlin Login decoder/RSA methods, complete LoginInfo/ISAAC/AuthResponse/ByteBufferUtils, and original LoginWriteEvent methods. Explicit byte-storage, static client state, logger, online-name lookup and reply-I/O adapters. No authentication backend, world entry, UI or game simulation."}
    fixture = ROOT / "fixtures/login"
    provenance_data = (json.dumps(provenance, indent=2, sort_keys=True) + "\n").encode()
    if check:
        if (fixture / "oracle.jsonl").read_bytes().replace(b"\r", b"") != data or json.loads((fixture / "provenance.json").read_bytes()) != provenance:
            raise RuntimeError("Login oracle output/provenance differs; inspect before regeneration")
        print(f"{len(cases)} original Java/Kotlin login cases reproduced.")
    else:
        fixture.mkdir(parents=True, exist_ok=True)
        (fixture / "oracle.jsonl").write_bytes(data)
        (fixture / "provenance.json").write_bytes(provenance_data)
        print(f"Generated {len(cases)} original Java/Kotlin login cases.")
    if interop_client:
        host, port = interop_client.rsplit(":", 1)
        print(run(["java", "-cp", library_cp, "LoginProbe", "client", host, port]).stdout.decode(), end="")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path, help="JDK 11 home for the original Kotlin 1.8.20 compiler")
    parser.add_argument("--interop-client", help="Test original JVM client-writing slice against a native endpoint")
    args = parser.parse_args()
    JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check, args.interop_client)
