"""The scripted inputs of the world-trace corpus.

A scenario is a list of steps; every byte a session receives is written out
here, so the script is the complete input of a run:

    connect  a new connection
    recv     bytes arriving on it, handed to the session's current producer
    tick     one MajorUpdateWorker.handleTickActions(false), clock + 600 ms
    close    the connection reached end of stream

The login block is built the way the pinned client builds it (the same field
order Login.decodeFromBuffer reads), encrypted with the client's public
exponent. Its header values copy the packet in the original's LoginTests.
"""
import re
import struct

# 2023-07-12T12:00:00Z, a Wednesday on which none of the original's holiday events is active.
CLOCK_START = 1689163200000
THREAD_LOCAL_SEED = 0x2009_0530
ISAAC_SEED = [0x11111111, 0x22222222, 0x33333333, 0x44444444]
IDLE_TICKS = 20
# The disconnection queue runs on every third tick; these cover removal and the save it queues.
TICKS_AFTER_CLOSE = 6


def name37(name):
    value = 0
    for ch in name[:12]:
        value *= 37
        if "a" <= ch <= "z":
            value += 1 + ord(ch) - ord("a")
        elif "0" <= ch <= "9":
            value += 27 + ord(ch) - ord("0")
        elif ch != "_":
            raise ValueError(name)
    while value and value % 37 == 0:
        value //= 37
    return value


def java_bytes(value):
    """BigInteger.toByteArray for a non-negative value."""
    return value.to_bytes(value.bit_length() // 8 + 1, "big")


def handshake(name):
    return bytes([14, (name37(name) >> 16) & 31])


def login(name, password, modulus, exponent, opcode=16):
    secret = bytes([10]) + b"".join(struct.pack(">i", s) for s in ISAAC_SEED) + struct.pack(">q", name37(name)) + password.encode("ascii") + b"\0"
    block = java_bytes(pow(int.from_bytes(secret, "big"), exponent, modulus))
    assert len(block) <= 255
    body = struct.pack(">i", 530)
    body += bytes([0, 1, 1, 2]) + struct.pack(">hh", 765, 503) + bytes([0])   # skipped, ads, skipped, window mode, size, display mode
    body += b"\xff" * 24 + b"\0"                                              # 24 skipped bytes and an empty string
    body += struct.pack(">iih", 0, 0, 0)                                      # affiliate, settings hash, packet count
    body += struct.pack(">29i", *([0] * 29))                                  # cache checksums (read, not compared)
    body += bytes([len(block)]) + block
    return bytes([opcode]) + struct.pack(">h", len(body)) + body


def session(name, steps_in_world, modulus, exponent, password="tracepassword"):
    return [
        dict(op="connect", session=name),
        dict(op="recv", session=name, hex=handshake(name).hex()),
        dict(op="recv", session=name, hex=login(name, password, modulus, exponent).hex()),
        *steps_in_world,
    ]


def build(server_constants, client_config):
    modulus = int(re.search(r'var MODULUS = BigInteger\("([0-9]+)"\)', server_constants).group(1))
    # The client file also carries Jagex's key in comments; only the declarations count.
    client_modulus = int(re.search(r'(?m)^\s*public static final BigInteger RSA_MODULUS = new BigInteger\("([0-9]+)"\)', client_config).group(1))
    if client_modulus != modulus:
        raise RuntimeError("client and server RSA moduli differ")
    exponent = int(re.search(r'(?m)^\s*public static final BigInteger RSA_EXPONENT = new BigInteger\("([0-9]+)"\)', client_config).group(1))
    idle = [dict(op="tick")] * IDLE_TICKS
    after = [dict(op="tick")] * TICKS_AFTER_CLOSE
    base = dict(clockStart=CLOCK_START, threadLocalSeed=THREAD_LOCAL_SEED)
    one = session("traceone", [*idle, dict(op="close", session="traceone"), *after], modulus, exponent)
    two = [
        *session("traceone", [dict(op="tick")] * 5, modulus, exponent),
        *session("tracetwo", [dict(op="tick")] * 15, modulus, exponent),
        dict(op="close", session="traceone"),
        *[dict(op="tick")] * 5,
        dict(op="close", session="tracetwo"),
        *after,
    ]
    return [
        dict(base, name="boot", description="Start-up only: no connection and no tick.", steps=[]),
        dict(base, name="new-character", description=f"Start-up, one new character logs in, {IDLE_TICKS} ticks with no client input, "
             f"the connection closes, {TICKS_AFTER_CLOSE} more ticks.", steps=one),
        dict(base, name="two-characters", description="Start-up, a new character logs in, 5 ticks, a second new character logs in, "
             f"15 ticks, the first connection closes, 5 ticks, the second closes, {TICKS_AFTER_CLOSE} more ticks.", steps=two),
        dict(base, name="returning-character", description="As new-character, but the world starts with the character save that "
             "new-character wrote.", steps=one, filesFrom="new-character"),
    ]
