import rt4.*;
import java.io.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import java.util.zip.GZIPOutputStream;

/** Calls the original complete Cache, Js5Index and Js5 classes; no extracted method copies. */
public final class CacheOracle {
    private static byte[] container(byte[] payload, boolean gzip) throws Exception {
        byte[] compressed = payload;
        if (gzip) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            GZIPOutputStream stream = new GZIPOutputStream(out);
            stream.write(payload);
            stream.close();
            compressed = out.toByteArray();
        }
        Buffer result = new Buffer(compressed.length + (gzip ? 9 : 5));
        result.p1(gzip ? 2 : 0);
        result.p4(compressed.length);
        if (gzip) result.p4(payload.length);
        result.pdata(compressed, compressed.length);
        return result.data;
    }
    private static byte[] reference(int protocol, int flags, int version, int[] ids,
            int[][] files, int[] hashes, int[][] fileHashes, byte[][] packed) throws Exception {
        Buffer out = new Buffer(1_000_000);
        out.p1(protocol);
        if (protocol == 6) out.p4(version);
        out.p1(flags);
        out.p2(ids.length);
        int last = 0;
        for (int id : ids) {out.p2(id-last); last = id;}
        if (flags != 0) for (int hash : hashes) out.p4(hash);
        for (int i = 0; i < ids.length; i++) out.p4(packed == null ? hashes[i] : Buffer.crc32(packed[i], packed[i].length-2));
        for (int i = 0; i < ids.length; i++) out.p4(version + i);
        for (int[] groupFiles : files) out.p2(groupFiles.length);
        for (int[] groupFiles : files) {
            last = 0;
            for (int id : groupFiles) {out.p2(id-last); last = id;}
        }
        if (flags != 0) for (int[] names : fileHashes) for (int hash : names) out.p4(hash);
        return container(Arrays.copyOf(out.data, out.offset), protocol == 6);
    }
    private static Object describe(Js5Index index) {
        List<Object> groups = new ArrayList<Object>();
        for (int id : index.groupIds) {
            int[] files = index.fileIds[id];
            if (files == null) {
                files = new int[index.groupSizes[id]];
                for (int i = 0; i < files.length; i++) files[i] = i;
            }
            List<Object> entries = new ArrayList<Object>();
            for (int file : files) entries.add(OracleMain.map("id", file,
                "name", index.fileNameHashes == null ? null : index.fileNameHashes[id][file]));
            List<Object> lookups = new ArrayList<Object>();
            if (index.fileNameHashTables != null) {
                for (int hash : new int[] {-1, 0, 1, Integer.MIN_VALUE, Integer.MAX_VALUE})
                    lookups.add(OracleMain.map("hash", hash, "id", index.fileNameHashTables[id].get(hash)));
                for (int file : files) {
                    int hash = index.fileNameHashes[id][file];
                    lookups.add(OracleMain.map("hash", hash, "id", index.fileNameHashTables[id].get(hash)));
                }
            }
            groups.add(OracleMain.map("id", id, "checksum", Integer.toUnsignedLong(index.groupChecksums[id]),
                "version", index.groupVersions[id], "capacity", index.groupCapacities[id],
                "dense", index.fileIds[id] == null, "files", entries, "lookups", lookups,
                "name", index.groupNameHashes == null ? null : index.groupNameHashes[id]));
        }
        List<Object> lookups = new ArrayList<Object>();
        if (index.groupNameHashTable != null) {
            for (int hash : new int[] {-1, 0, 1, Integer.MIN_VALUE, Integer.MAX_VALUE})
                lookups.add(OracleMain.map("hash", hash, "id", index.groupNameHashTable.get(hash)));
            for (int id : index.groupIds) {
                int hash = index.groupNameHashes[id];
                lookups.add(OracleMain.map("hash", hash, "id", index.groupNameHashTable.get(hash)));
            }
        }
        return OracleMain.map("version", index.version, "checksum", Integer.toUnsignedLong(index.checksum),
            "capacity", index.capacity, "groups", groups, "lookups", lookups);
    }
    private static final class Provider extends Js5ResourceProvider {
        private final Js5Index index;
        private final Map<Integer, byte[]> groups;
        Provider(byte[] reference, Map<Integer, byte[]> groups) {
            this.index = new Js5Index(reference, Buffer.crc32(reference, reference.length));
            this.groups = groups;
        }
        public Js5Index fetchIndex() {return index;}
        public byte[] fetchGroup(int group) {return groups.get(group);}
        public void prefetchGroup(int group) {}
        public int getPercentageComplete(int group) {return groups.containsKey(group) ? 100 : 0;}
    }
    private static void encrypt(byte[] bytes, int[] key) {
        // Input construction only; expected decryption/extraction uses unmodified Buffer/Js5.
        Buffer buffer = new Buffer(bytes);
        for (int offset = 5; offset + 8 <= bytes.length; offset += 8) {
            buffer.offset = offset;
            int a = buffer.g4(), b = buffer.g4(), sum = 0;
            for (int round = 0; round < 32; round++) {
                a += ((b << 4 ^ b >>> 5) + b) ^ (sum + key[sum & 3]);
                sum += 0x9e3779b9;
                b += ((a << 4 ^ a >>> 5) + a) ^ (sum + key[sum >>> 11 & 3]);
            }
            buffer.offset = offset;
            buffer.p4(a);
            buffer.p4(b);
        }
    }
    private static void indexCases() throws Exception {
        Random random = new Random(530);
        for (int scenario = 0; scenario < 64; scenario++) {
            int count = scenario == 0 ? 0 : 1 + random.nextInt(16);
            int[] ids = new int[count], hashes = new int[count];
            int[][] files = new int[count][], names = new int[count][];
            int last = 0;
            for (int g = 0; g < count; g++) {
                ids[g] = last += g == 0 ? random.nextInt(3) : 1 + random.nextInt(8);
                hashes[g] = g % 3 == 0 ? -1 : g % 3 == 1 ? 1 : random.nextInt();
                int size = random.nextInt(12);
                files[g] = new int[size];
                names[g] = new int[size];
                int fileId = 0;
                for (int f = 0; f < size; f++) {
                    files[g][f] = fileId += f == 0 ? random.nextInt(2) : 1 + (scenario % 3 == 0 ? 0 : random.nextInt(5));
                    names[g][f] = f % 3 == 0 ? -1 : f % 3 == 1 ? 1 : random.nextInt();
                }
            }
            int protocol = scenario % 2 == 0 ? 5 : 6;
            int flags = new int[] {0, 1, 2, 255}[scenario % 4];
            byte[] data = reference(protocol, flags, random.nextInt(), ids, files, hashes, names, null);
            Js5Index index = new Js5Index(data, Buffer.crc32(data, data.length));
            OracleMain.emit("kind", "index", "input", OracleMain.hex(data),
                "protocol", protocol, "expected", describe(index));
        }
    }
    private static BufferedFile open(Path path, String mode) throws Exception {
        return new BufferedFile(new FileOnDisk(path.toFile(), mode, Long.MAX_VALUE), 4096, 0);
    }
    private static int u24(byte[] bytes, int offset) {
        return (bytes[offset] & 255) << 16 | (bytes[offset+1] & 255) << 8 | bytes[offset+2] & 255;
    }
    private static void synthetic(Path directory) throws Exception {
        Files.createDirectories(directory);
        String prefix = "main_file_cache.";
        Files.write(directory.resolve(prefix + "dat2"), new byte[0]);
        for (int archive : new int[] {3, 7, 255}) Files.write(directory.resolve(prefix + "idx" + archive), new byte[0]);
        BufferedFile data = open(directory.resolve(prefix+"dat2"), "rw");
        BufferedFile idx3 = open(directory.resolve(prefix+"idx3"), "rw");
        BufferedFile idx7 = open(directory.resolve(prefix+"idx7"), "rw");
        BufferedFile idx255 = open(directory.resolve(prefix+"idx255"), "rw");
        Cache storage3 = new Cache(3, data, idx3, 1_000_000);
        Cache storage7 = new Cache(7, data, idx7, 1_000_000);
        Cache storage255 = new Cache(255, data, idx255, 1_000_000);
        Random random = new Random(2009);
        int[] rawIds = {0, 1, 2, 255, 512, 65535};
        int[] sizes = {1, 511, 512, 513, 2048, 65000};
        for (int i = 0; i < rawIds.length; i++) {
            byte[] payload = new byte[sizes[i]];
            random.nextBytes(payload);
            if (!storage7.write(rawIds[i], payload.length, payload)) throw new AssertionError("Cache.write");
            OracleMain.emit("kind", "store", "archive", 7, "group", rawIds[i],
                "expected", OracleMain.hex(storage7.read(rawIds[i])), "mutations", new Object[0]);
        }
        int count = 48;
        int[] ids = new int[count], hashes = new int[count];
        int[][] fids = new int[count][], names = new int[count][], keys = new int[count][];
        byte[][] packed = new byte[count][];
        Map<Integer, byte[]> groups = new LinkedHashMap<Integer, byte[]>();
        for (int g = 0; g < count; g++) {
            ids[g] = 2 + g * 3;
            hashes[g] = g % 3 == 0 ? -1 : random.nextInt();
            int files = 1 + g % 8, stripes = 1 + g % 7;
            fids[g] = new int[files];
            names[g] = new int[files];
            for (int f = 0; f < files; f++) {
                fids[g][f] = g % 2 == 0 ? f : f * 3 + 1;
                names[g][f] = f % 2 == 0 ? -1 : random.nextInt();
            }
            ByteArrayOutputStream payload = new ByteArrayOutputStream();
            Buffer table = new Buffer(stripes * files * 4 + 1);
            if (files == 1) {
                byte[] file = new byte[g == 0 ? 0 : g * 37 + 1];
                random.nextBytes(file);
                payload.write(file);
            } else {
                for (int stripe = 0; stripe < stripes; stripe++) {
                    int previous = 0;
                    for (int file = 0; file < files; file++) {
                        int length = random.nextInt(64);
                        byte[] bytes = new byte[length];
                        random.nextBytes(bytes);
                        payload.write(bytes);
                        table.p4(length-previous);
                        previous = length;
                    }
                }
                payload.write(table.data, 0, table.offset);
                payload.write(stripes);
            }
            byte[] wrapped = container(payload.toByteArray(), g % 2 != 0);
            wrapped = Arrays.copyOf(wrapped, wrapped.length+2);
            wrapped[wrapped.length-1] = (byte) g;
            keys[g] = g % 3 == 0 ? new int[] {random.nextInt(), random.nextInt(), random.nextInt(), random.nextInt()} : new int[4];
            if (g % 3 == 0) encrypt(wrapped, keys[g]);
            packed[g] = wrapped;
            groups.put(ids[g], wrapped);
            if (!storage3.write(ids[g], wrapped.length, wrapped)) throw new AssertionError("Cache.write");
        }
        byte[] reference = reference(6, 1, 0, ids, fids, hashes, names, packed);
        if (!storage255.write(3, reference.length, reference)) throw new AssertionError("Cache.write");
        OracleMain.emit("kind", "index", "input", OracleMain.hex(reference), "protocol", 6,
            "expected", describe(new Js5Index(reference, Buffer.crc32(reference, reference.length))));
        for (int g = 0; g < count; g++) {
            Js5 js5 = new Js5(new Provider(reference, groups), false, false);
            List<Object> files = new ArrayList<Object>();
            for (int id : fids[g]) files.add(OracleMain.map("id", id, "expected", OracleMain.hex(js5.fetchFile(ids[g], keys[g], id))));
            OracleMain.emit("kind", "group", "archive", 3, "group", ids[g], "key", keys[g], "files", files);
        }
        for (int selected : new int[] {1, 8}) {
            byte[] broken = packed[selected].clone();
            broken[1] = (byte) 0xff; // force an invalid declared length
            Map<Integer, byte[]> changed = new LinkedHashMap<Integer, byte[]>(groups);
            changed.put(ids[selected], broken);
            Js5 js5 = new Js5(new Provider(reference, changed), false, false);
            PrintStream stdout = System.out, stderr = System.err;
            ByteArrayOutputStream messages = new ByteArrayOutputStream();
            List<Object> files = new ArrayList<Object>();
            try {
                System.setOut(new PrintStream(messages));
                System.setErr(new PrintStream(new ByteArrayOutputStream()));
                for (int id : fids[selected]) files.add(OracleMain.map("id", id,
                    "expected", OracleMain.hex(js5.fetchFile(ids[selected], keys[selected], id))));
            } finally {System.setOut(stdout); System.setErr(stderr);}
            if (!messages.toString("UTF-8").contains("T3 - ")) throw new AssertionError("expected original fallback");
            OracleMain.emit("kind", "group_fallback", "group", ids[selected], "input", OracleMain.hex(broken),
                "key", keys[selected], "files", files);
        }
        // Persisted byte fixtures now contain all valid headers and multi-sector chains.
        idx3.close(); idx7.close(); idx255.close(); data.close();
        byte[] originalData = Files.readAllBytes(directory.resolve(prefix+"dat2"));
        byte[] originalIndex = Files.readAllBytes(directory.resolve(prefix+"idx7"));
        int sector = u24(originalIndex, 3 * 6 + 3); // missing group 3 => sector zero
        OracleMain.emit("kind", "store", "archive", 7, "group", 3, "expected", null, "mutations", new Object[0]);
        OracleMain.emit("kind", "store", "archive", 7, "group", 65536, "expected", null, "mutations", new Object[0]);
        // Probe corruption of each header field on an actual chained group.
        sector = u24(originalIndex, 255 * 6 + 3);
        for (int field : new int[] {0, 2, 4, 5, 6, 7}) {
            int offset = sector * 520 + field;
            byte[] changed = originalData.clone();
            changed[offset] ^= (byte) 0xff;
            Files.write(directory.resolve(prefix+"dat2"), changed);
            BufferedFile corruptedData = open(directory.resolve(prefix+"dat2"), "r");
            BufferedFile readIndex = open(directory.resolve(prefix+"idx7"), "r");
            byte[] result = new Cache(7, corruptedData, readIndex, 1_000_000).read(255);
            OracleMain.emit("kind", "store", "archive", 7, "group", 255,
                "expected", result == null ? null : OracleMain.hex(result), "mutations",
                new Object[] {OracleMain.map("offset", offset, "value", changed[offset] & 255)});
            readIndex.close(); corruptedData.close();
        }
        Files.write(directory.resolve(prefix+"dat2"), originalData);
    }
    private static int hash(String text) {
        int hash = 0;
        for (int i = 0; i < text.length(); i++) hash = hash * 31 + text.charAt(i);
        return hash;
    }
    private static void word(MessageDigest digest, int value) {
        digest.update(new byte[] {(byte) (value >>> 24), (byte) (value >>> 16), (byte) (value >>> 8), (byte) value});
    }
    private static void audit(Path directory, Path keyFile) throws Exception {
        Map<Integer, int[]> regionKeys = new LinkedHashMap<Integer, int[]>();
        for (String line : Files.readAllLines(keyFile)) {
            String[] values = line.split(" ");
            int[] key = new int[4];
            for (int i = 0; i < key.length; i++) key[i] = (int) Long.parseLong(values[i + 1]);
            regionKeys.put(Integer.parseInt(values[0]), key);
        }
        BufferedFile data = open(directory.resolve("main_file_cache.dat2"), "r");
        BufferedFile idx255 = open(directory.resolve("main_file_cache.idx255"), "r");
        Cache references = new Cache(255, data, idx255, 16 * 1024 * 1024);
        for (int id = 0; id < 255; id++) {
            Path path = directory.resolve("main_file_cache.idx" + id);
            if (!Files.isRegularFile(path)) continue;
            BufferedFile idx = open(path, "r");
            final Cache storage = new Cache(id, data, idx, 16 * 1024 * 1024);
            byte[] reference = references.read(id);
            final Js5Index index = new Js5Index(reference, Buffer.crc32(reference, reference.length));
            Js5ResourceProvider provider = new Js5ResourceProvider() {
                public Js5Index fetchIndex() {return index;}
                public byte[] fetchGroup(int group) {return storage.read(group);}
                public void prefetchGroup(int group) {}
                public int getPercentageComplete(int group) {return 100;}
            };
            Js5 js5 = new Js5(provider, true, true);
            Map<Integer, int[]> keys = new HashMap<Integer, int[]>();
            if (id == 5 && index.groupNameHashTable != null) {
                for (Map.Entry<Integer, int[]> entry : regionKeys.entrySet()) {
                    int region = entry.getKey();
                    int group = index.groupNameHashTable.get(hash("l" + (region >> 8) + "_" + (region & 255)));
                    if (group >= 0) keys.put(group, entry.getValue());
                }
            }
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            int groups = 0, files = 0;
            long bytes = 0;
            List<Object> failures = new ArrayList<Object>();
            for (int group : index.groupIds) {
                if (index.groupCapacities[group] == 0) continue;
                try {
                    int[] fids = js5.getFileIds(group);
                    for (int file : fids) {
                        byte[] contents = js5.fetchFile(group, keys.get(group), file);
                        if (contents == null) throw new IllegalStateException("missing file " + file);
                        word(digest, group);
                        word(digest, file);
                        word(digest, contents.length);
                        digest.update(contents);
                        files++;
                        bytes += contents.length;
                    }
                    groups++;
                } catch (Throwable exception) {
                    failures.add(OracleMain.map("group", group, "error", exception.getClass().getName()));
                }
            }
            OracleMain.emit("archive", id, "groups", groups, "files", files, "bytes", bytes,
                "sha256", OracleMain.hex(digest.digest()), "failures", failures);
            idx.close();
        }
        idx255.close();
        data.close();
    }
    public static void main(String[] args) throws Exception {
        if (args[0].equals("fixtures")) {
            indexCases();
            synthetic(Paths.get(args[1]));
        } else if (args[0].equals("audit")) {
            audit(Paths.get(args[1]), Paths.get(args[2]));
        } else {
            throw new IllegalArgumentException("unknown CacheOracle mode");
        }
    }
}
