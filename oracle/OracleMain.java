import rt4.*;
import java.io.*;
import java.lang.reflect.Array;
import java.lang.reflect.Method;
import java.nio.file.*;
import java.util.*;
import java.util.zip.GZIPOutputStream;

/**
 * Runs pinned, unmodified RT4 classes. Each JSON line is one independent case.
 * No third-party JSON library and no network or cache downloads are needed.
 */
public final class OracleMain {
    private static final int[] EDGES = {
        Integer.MIN_VALUE, -1000000001, -65537, -32769, -32768, -129, -128, -1,
        0, 1, 2, 3, 7, 8, 63, 64, 127, 128, 255, 256, 32767, 32768, 65535,
        65536, 0x12345678, 0x7ffffffe, Integer.MAX_VALUE
    };
    static String hex(byte[] bytes) {
        StringBuilder result = new StringBuilder(bytes.length * 2);
        for (byte value : bytes) result.append(String.format(Locale.ROOT, "%02x", value & 255));
        return result.toString();
    }
    private static byte[] bytes(Random random, int count) {
        byte[] result = new byte[count];
        random.nextBytes(result);
        return result;
    }
    static Map<String, Object> map(Object... entries) {
        Map<String, Object> result = new LinkedHashMap<String, Object>();
        for (int i = 0; i < entries.length; i += 2) result.put((String) entries[i], entries[i + 1]);
        return result;
    }
    private static String json(Object value) {
        if (value == null) return "null";
        if (value instanceof String) {
            return "\"" + ((String) value).replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
        }
        if (value instanceof Number || value instanceof Boolean) return value.toString();
        if (value instanceof Map) {
            List<String> entries = new ArrayList<String>();
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) value).entrySet())
                entries.add(json(entry.getKey()) + ":" + json(entry.getValue()));
            return "{" + String.join(",", entries) + "}";
        }
        List<String> entries = new ArrayList<String>();
        if (value instanceof Iterable) {
            for (Object entry : (Iterable<?>) value) entries.add(json(entry));
        } else {
            for (int i = 0; i < Array.getLength(value); i++) entries.add(json(Array.get(value, i)));
        }
        return "[" + String.join(",", entries) + "]";
    }
    static void emit(Object... entries) { System.out.println(json(map(entries))); }

    private static void ciphers() {
        int[] full = new int[256];
        for (int i = 0; i < full.length; i++) full[i] = i * 0x9e3779b9;
        int[][] seeds = { {}, {0, 0, 0, 0}, {1, 2, 3, 4},
            {Integer.MIN_VALUE, Integer.MAX_VALUE, -1, 0}, full };
        GlobalConfig.USE_ISAAC = true;
        for (int[] seed : seeds) {
            IsaacRandom random = new IsaacRandom(seed);
            int[] output = new int[4096];
            for (int i = 0; i < output.length; i++) output[i] = random.getNextKey();
            emit("kind", "isaac", "seed", seed, "values", output);
        }
        GlobalConfig.USE_ISAAC = false;
        IsaacRandom random = new IsaacRandom(new int[] {1, 2, 3, 4});
        int[] disabled = new int[600];
        for (int i = 0; i < disabled.length; i++) disabled[i] = random.getNextKey();
        GlobalConfig.USE_ISAAC = true;
        int[] enabled = new int[600];
        for (int i = 0; i < enabled.length; i++) enabled[i] = random.getNextKey();
        GlobalConfig.USE_ISAAC = false;
        int paused = random.getNextKey();
        GlobalConfig.USE_ISAAC = true;
        int resumed = random.getNextKey();
        emit("kind", "isaac_toggle", "seed", new int[] {1, 2, 3, 4},
            "disabled", disabled, "enabled", enabled, "paused", paused, "resumed", resumed);
        GlobalConfig.USE_ISAAC = false;
    }
    private static void buffers() throws Exception {
        Random corpus = new Random(530L);
        String[] methods = { "g1", "g1b", "g1add", "g1neg", "g1sub", "g1badd",
            "g1bneg", "g1bsub", "g2", "ig2", "g2add", "ig2add", "g2b", "ig2b",
            "g2badd", "ig2badd", "g3", "ig3", "g4", "ig4", "mg4", "img4",
            "g8", "gsmart", "gsmarts", "gVarSmart" };
        for (int i = 0; i < 80; i++) {
            byte[] input = bytes(corpus, 24);
            if (i == 0) Arrays.fill(input, (byte) 0);
            if (i == 1) Arrays.fill(input, (byte) 255);
            // Ensure a repeated-32767 smart has a terminating value.
            input[20] = 0;
            for (String method : methods) {
                Buffer buffer = new Buffer(input.clone());
                buffer.offset = i % 4;
                Number result = (Number) Buffer.class.getMethod(method).invoke(buffer);
                emit("kind", "read", "method", method, "input", hex(input),
                    "start", i % 4, "value", result.longValue(), "end", buffer.offset);
            }
            for (int length = 1; length <= 8; length++) {
                Buffer buffer = new Buffer(input.clone());
                buffer.offset = i % 4;
                long result = buffer.gVarLong(length);
                emit("kind", "read_long", "input", hex(input), "start", i % 4,
                    "length", length, "value", result, "end", buffer.offset);
            }
            Packet packet = new Packet(input.length);
            packet.data = input;
            packet.offset = i % 4;
            packet.accessBits();
            int[] sizes = {i % 8, 0, 1, 2, 7, 8, 9, 15, 16, 31, 32};
            int[] values = new int[sizes.length];
            for (int j = 0; j < sizes.length; j++) values[j] = packet.gBits(sizes[j]);
            int remaining = packet.availableBits(input.length);
            packet.accessBytes();
            emit("kind", "bits", "input", hex(input), "start", i % 4, "sizes", sizes,
                "values", values, "remaining", remaining, "end", packet.offset);
            long crc64 = -1;
            for (byte value : input)
                crc64 = (crc64 >>> 8) ^ Buffer.CRC64_TABLE[(int) (crc64 ^ value) & 255];
            emit("kind", "checksum", "input", hex(input), "crc32",
                Integer.toUnsignedLong(Buffer.crc32(input, input.length)),
                "crc64", Long.toUnsignedString(~crc64));
        }
        String[] writes = {"p1", "p1add", "p1sub", "p2", "p2add", "ip2",
            "ip2add", "p3", "p4", "ip4", "mp4", "imp4", "pVarInt"};
        for (int value : EDGES) {
            for (String method : writes) {
                Buffer buffer = new Buffer(64);
                buffer.p1(0x59);
                Buffer.class.getMethod(method, int.class).invoke(buffer, value);
                emit("kind", "write", "method", method, "value", value,
                    "bytes", hex(Arrays.copyOf(buffer.data, buffer.offset)));
            }
            Buffer varint = new Buffer(8);
            varint.pVarInt(value);
            int end = varint.offset;
            varint.offset = 0;
            int read = varint.gVarInt();
            emit("kind", "varint", "value", value, "input",
                hex(Arrays.copyOf(varint.data, end)), "read", read, "end", varint.offset);
        }
        for (int value : new int[] {0, 1, 63, 64, 127, 128, 255, 16383, 16384, 32766, 32767}) {
            Buffer buffer = new Buffer(8);
            buffer.psmarts(value);
            emit("kind", "write", "method", "psmarts", "value", value,
                "bytes", "59" + hex(Arrays.copyOf(buffer.data, buffer.offset)));
        }
        for (long value : new long[] {Long.MIN_VALUE, Long.MAX_VALUE, -1, 0, 1, 0x123456789abcdef0L}) {
            Buffer buffer = new Buffer(8);
            buffer.p8(value);
            emit("kind", "write_long", "value", value, "length", 8, "bytes", hex(buffer.data));
            for (int length = 1; length <= 8; length++) {
                buffer.offset = 0;
                buffer.pVarLong(length, value);
                emit("kind", "write_long", "value", value, "length", length,
                    "bytes", hex(Arrays.copyOf(buffer.data, buffer.offset)));
            }
        }
        for (int i = 0; i < 120; i++) {
            byte[] input = bytes(corpus, 5 + i);
            int[] key = {corpus.nextInt(), corpus.nextInt(), corpus.nextInt(), corpus.nextInt()};
            Buffer buffer = new Buffer(input.clone());
            buffer.offset = i % input.length;
            buffer.tinydec(key, input.length);
            emit("kind", "xtea", "input", hex(input), "key", key, "length", input.length,
                "bytes", hex(buffer.data), "restoredOffset", buffer.offset);
        }
    }
    private static void huffman() {
        byte[] fixed = new byte[256];
        Arrays.fill(fixed, (byte) 8);
        byte[] unsorted = {3, 2, 3, 1};
        byte[] ladder = new byte[33];
        for (int i = 0; i < 32; i++) ladder[i] = (byte) (i + 1);
        ladder[32] = 32;
        Random random = new Random(2009);
        for (byte[] table : new byte[][] {fixed, unsorted, ladder}) {
            HuffmanCodec codec = new HuffmanCodec(table);
            for (int size : new int[] {1, 2, 7, 16, 63, 128, 511}) {
                byte[] source = new byte[size + 5];
                for (int i = 0; i < source.length; i++) source[i] = (byte) random.nextInt(table.length);
                byte[] encoded = new byte[4 * size + 12];
                Arrays.fill(encoded, (byte) 0x59);
                int length = codec.encode(size + 3, encoded, source, 3, 2);
                byte[] decoded = new byte[size + 9];
                Arrays.fill(decoded, (byte) 0x33);
                int consumed = codec.decode(4, size + 4, decoded, encoded, 2);
                emit("kind", "huffman", "table", table, "source", hex(source),
                    "sourceStart", 3, "sourceEnd", size + 3, "encoded",
                    hex(Arrays.copyOfRange(encoded, 2, 2 + length)), "length", length,
                    "decoded", hex(decoded), "consumed", consumed);
            }
        }
    }
    private static void integers() {
        for (int value : EDGES) {
            emit("kind", "integer", "value", value, "clp2", IntUtils.clp2(value),
                "bitCount", IntUtils.bitCount(value), "bitCountFast", IntUtils.bitCountFast(value),
                "powerOfTwo", IntUtils.isPowerOfTwo(value));
            for (int count : new int[] {-1, 0, 1, 2, 7, 8, 15, 16, 31, 32, 33, 63, 64, 65})
                emit("kind", "reverse", "value", value, "count", count,
                    "result", IntUtils.bitReverse(count, value));
            for (int exponent : new int[] {-1, 0, 1, 2, 3, 7, 31, 32, 1000})
                emit("kind", "pow", "value", value, "exponent", exponent,
                    "result", IntUtils.pow(exponent, value));
            for (int bound : new int[] {1, 2, 3, 127, 128, 65537, Integer.MAX_VALUE})
                emit("kind", "shuffle", "value", value, "bound", bound,
                    "result", RandomUtils.shuffle(value, bound));
            for (int[] limits : new int[][] {{-10, 10}, {10, -10}, {Integer.MIN_VALUE, Integer.MAX_VALUE}})
                emit("kind", "clamp", "value", value, "min", limits[0], "max", limits[1],
                    "result", IntUtils.clamp(limits[1], value, limits[0]));
        }
    }
    private static void random() {
        for (long seed : new long[] {0, 1, -1, 530, Long.MIN_VALUE, Long.MAX_VALUE}) {
            for (String mode : new String[] {"int", "long", "double"}) {
                Random random = new Random(seed);
                List<Object> values = new ArrayList<Object>();
                for (int i = 0; i < 128; i++) {
                    if (mode.equals("int")) values.add(random.nextInt());
                    if (mode.equals("long")) values.add(random.nextLong());
                    if (mode.equals("double")) values.add(Long.toUnsignedString(Double.doubleToRawLongBits(random.nextDouble())));
                }
                emit("kind", "random", "seed", seed, "mode", mode, "values", values);
            }
            for (int bound : new int[] {1, 2, 3, 7, 128, 255, 32767, 65537, 1000000001, 1073741825, Integer.MAX_VALUE}) {
                for (boolean rt4 : new boolean[] {false, true}) {
                    Random random = new Random(seed);
                    int[] values = new int[256];
                    for (int i = 0; i < values.length; i++)
                        values[i] = rt4 ? RandomUtils.nextInt(bound, random) : random.nextInt(bound);
                    emit("kind", "random", "seed", seed, "mode", rt4 ? "rt4_bound" : "java_bound",
                        "bound", bound, "values", values);
                }
            }
        }
    }
    private static void compression(String bzipPath) throws Exception {
        Random random = new Random(530);
        for (int size : new int[] {0, 1, 255, 4096, 131071}) {
            byte[] data = bytes(random, size);
            for (int type : new int[] {0, 2}) {
                byte[] compressed = data;
                if (type == 2) {
                    ByteArrayOutputStream stream = new ByteArrayOutputStream();
                    GZIPOutputStream gzip = new GZIPOutputStream(stream);
                    gzip.write(data);
                    gzip.close();
                    compressed = stream.toByteArray();
                }
                Buffer container = new Buffer(compressed.length + (type == 0 ? 5 : 9));
                container.p1(type);
                container.p4(compressed.length);
                if (type != 0) container.p4(data.length);
                container.pdata(compressed, compressed.length);
                emit("kind", "js5", "input", hex(container.data),
                    "output", hex(Js5Compression.uncompress(container.data)));
                if (type == 2) {
                    // The pinned edited cache relies on RT4 ignoring both trailer fields.
                    byte[] edited = container.data.clone();
                    Arrays.fill(edited, edited.length - 8, edited.length, (byte) 0x59);
                    emit("kind", "js5", "input", hex(edited),
                        "output", hex(Js5Compression.uncompress(edited)));
                }
            }
        }
        byte[] bzip = Files.readAllBytes(Paths.get(bzipPath));
        emit("kind", "js5", "input", hex(bzip),
            "output", hex(Js5Compression.uncompress(bzip)));
    }
    private static int[] flatten(CollisionMap map) {
        int[] result = new int[map.flags.length * map.flags[0].length];
        int position = 0;
        for (int[] column : map.flags) for (int value : column) result[position++] = value;
        return result;
    }
    private static void fill(CollisionMap map, int[] values) {
        int position = 0;
        for (int x = 0; x < map.flags.length; x++)
            for (int y = 0; y < map.flags[x].length; y++) map.flags[x][y] = values[position++];
    }
    private static void collision() {
        Random random = new Random(530);
        for (int[] size : new int[][] {{1, 1}, {4, 6}, {8, 8}, {32, 32}}) {
            emit("kind", "collision_clear", "width", size[0], "height", size[1],
                "flags", flatten(new CollisionMap(size[0], size[1])));
        }
        for (int shape = 0; shape <= 3; shape++) {
            for (int angle = 0; angle <= 3; angle++) {
                for (boolean projectiles : new boolean[] {false, true}) {
                    CollisionMap map = new CollisionMap(16, 16);
                    int[] initial = new int[256];
                    for (int i = 0; i < initial.length; i++) initial[i] = random.nextInt();
                    fill(map, initial);
                    map.flagWall(angle, shape, projectiles, 7, 8);
                    int[] added = flatten(map);
                    map.unflagWall(angle, projectiles, 7, shape, 8);
                    emit("kind", "collision_wall", "width", 16, "height", 16,
                        "x", 8, "y", 7, "shape", shape, "angle", angle,
                        "projectiles", projectiles, "initial", initial, "added", added,
                        "removed", flatten(map));
                }
            }
        }
        for (int[] at : new int[][] {{-2, 7}, {2, -2}, {2, 7}, {8, 7}, {15, 15}, {16, 16}}) {
            for (int angle = 0; angle <= 3; angle++) {
                for (boolean projectiles : new boolean[] {false, true}) {
                    CollisionMap map = new CollisionMap(16, 16);
                    int[] initial = new int[256];
                    for (int i = 0; i < initial.length; i++) initial[i] = random.nextInt();
                    fill(map, initial);
                    map.flagScenery(at[0], projectiles, at[1], 3, 5);
                    int[] added = flatten(map);
                    map.unflagScenery(at[0], 3, projectiles, angle, 5, at[1]);
                    emit("kind", "collision_scenery", "width", 16, "height", 16,
                        "x", at[0], "y", at[1], "sizeX", 3, "sizeY", 5, "angle", angle,
                        "projectiles", projectiles, "initial", initial, "added", added, "removed", flatten(map));
                }
            }
        }
        for (int bit : new int[] {0x200000, 0x40000}) {
            CollisionMap map = new CollisionMap(16, 16);
            if (bit == 0x200000) map.flagTile(7, 8); else map.flagGroundDecor(8, 7);
            int[] added = flatten(map);
            // Only ground decor has a public removal method.
            if (bit == 0x40000) map.unflagGroundDecor(7, 8);
            emit("kind", "collision_tile", "mask", bit, "added", added, "removed", flatten(map));
        }
        for (int pattern = 0; pattern < 25; pattern++) {
            CollisionMap map = new CollisionMap(32, 32);
            int[] flags = new int[1024];
            if (pattern == 1) flags = flatten(map);
            else if (pattern > 1) {
                for (int i = 0; i < flags.length; i++)
                    if (random.nextInt(100) < 5) flags[i] = 1 << (pattern - 2);
            }
            fill(map, flags);
            List<Object> queries = new ArrayList<Object>();
            for (int i = 0; i < 200; i++) {
                int sx = random.nextInt(32), sy = random.nextInt(32);
                int dx = random.nextInt(34) - 1, dy = random.nextInt(34) - 1;
                queries.add(map("sx", sx, "sy", sy, "dx", dx, "dy", dy,
                    "clear", map.isPathClear(sy, dx, dy, sx)));
            }
            emit("kind", "collision_path", "width", 32, "height", 32, "flags", flags, "queries", queries);
        }
    }
    private static void directions() {
        for (int x : EDGES) for (int y : EDGES) {
            core.game.world.map.Location start = new core.game.world.map.Location(0, 0, 0);
            core.game.world.map.Location end = new core.game.world.map.Location(x, y, 0);
            emit("kind", "direction_delta", "x", x, "y", y,
                "code", core.game.world.map.Direction.getDirection(x, y).value,
                "locationCode", core.game.world.map.Direction.getDirection(start, end).value,
                "logicalCode", core.game.world.map.Direction.getLogicalDirection(start, end).value);
        }
        for (int flags = 0; flags < 16; flags++) for (int rotation = 0; rotation < 4; rotation++) {
            core.game.world.map.Direction direction = core.game.world.map.Direction.forWalkFlag(flags, rotation);
            emit("kind", "direction_flag", "flags", flags, "rotation", rotation,
                "code", direction == null ? -1 : direction.value);
        }
        for (core.game.world.map.Direction direction : core.game.world.map.Direction.values()) {
            core.game.world.map.Point point = core.game.world.map.Direction.getWalkPoint(direction);
            emit("kind", "direction", "code", direction.value, "x", direction.getStepX(),
                "y", direction.getStepY(), "walkX", point.getX(), "walkY", point.getY(),
                "opposite", direction.getOpposite().value);
            for (int bit = -2; bit < 32; bit++) for (int slot = -1; slot < 4; slot++) {
                final int mask = bit == -2 ? 0 : bit == -1 ? -1 : 1 << bit;
                final int selected = slot;
                final List<Object> probes = new ArrayList<Object>();
                boolean allowed = direction.canMoveFrom(3, Integer.MAX_VALUE, Integer.MIN_VALUE,
                    (z, x, y) -> {
                        int index = probes.size();
                        probes.add(new int[] {z, x, y});
                        return selected < 0 || selected == index ? mask : 0;
                    });
                emit("kind", "direction_probe", "code", direction.value, "mask", mask,
                    "slot", slot, "z", 3, "x", Integer.MAX_VALUE, "y", Integer.MIN_VALUE,
                    "probes", probes, "allowed", allowed);
            }
        }
    }
    public static void main(String[] args) throws Exception {
        emit("kind", "defaults", "isaac", GlobalConfig.USE_ISAAC, "loginStrings",
            GlobalConfig.LOGIN_USE_STRINGS, "extraInfo", GlobalConfig.LOGIN_EXTRA_INFO,
            "fakeIndex28", GlobalConfig.LOGIN_FAKE_IDX28, "sampleRate", GlobalConfig.AUDIO_SAMPLE_RATE);
        ciphers();
        buffers();
        huffman();
        integers();
        random();
        compression(args[0]);
        collision();
        directions();
    }
}
