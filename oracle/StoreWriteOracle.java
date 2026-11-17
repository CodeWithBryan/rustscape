import rt4.*;
import java.io.RandomAccessFile;
import java.nio.file.*;
import java.util.*;

/**
 * Drives the original, unmodified Cache.write and Cache.read (over BufferedFile and FileOnDisk,
 * opened as the client opens them) through a fixed script and prints what happened: one JSON line
 * per step and, at three points, the length and SHA-256 of every file.
 * crates/cache/tests/store_write_oracle.rs replays the same lines against the Rust DiskStore and
 * compares every result and every digest.
 *
 * Compile with the sources xtask's cache oracle uses plus this file, then:
 *   java -cp CLASSES StoreWriteOracle EMPTY_WORK_DIR > crates/cache/tests/data/store_write.jsonl
 *
 * Not covered: writes past FileOnDisk's size limit, where the original plants a marker byte and the
 * Rust port deliberately does not.
 */
public final class StoreWriteOracle {
    private static final int[] ARCHIVES = {3, 7, 255};
    private static Path directory;
    private static BufferedFile data;
    private static final Map<Integer, BufferedFile> indexes = new TreeMap<Integer, BufferedFile>();
    private static final Map<Integer, Cache> caches = new TreeMap<Integer, Cache>();

    private static Path file(String name) {
        return directory.resolve("main_file_cache." + name);
    }
    private static void open() throws Exception {
        // The limits and buffer sizes of SignLink and client.mainInit.
        data = new BufferedFile(new FileOnDisk(file("dat2").toFile(), "rw", 104857600L), 5200, 0);
        for (int archive : ARCHIVES) {
            BufferedFile index = new BufferedFile(new FileOnDisk(file("idx" + archive).toFile(), "rw", 1048576L), 6000, 0);
            indexes.put(archive, index);
            caches.put(archive, new Cache(archive, data, index, archive == 255 ? 500000 : 1000000));
        }
    }
    private static void close() throws Exception {
        data.close();
        for (BufferedFile index : indexes.values()) index.close();
        indexes.clear();
        caches.clear();
    }
    static byte[] payload(int length, int seed) {
        byte[] bytes = new byte[length];
        for (int i = 0; i < length; i++) bytes[i] = (byte) (i * 31 + seed * 7 + (i >> 8));
        return bytes;
    }
    private static void write(int archive, int group, int length, int seed) {
        byte[] bytes = payload(length, seed);
        boolean ok = caches.get(archive).write(group, bytes.length, bytes);
        OracleMain.emit("kind", "write", "archive", archive, "group", group, "length", length, "seed", seed, "ok", ok);
    }
    private static void read(int archive, int group) {
        byte[] result = caches.get(archive).read(group);
        OracleMain.emit("kind", "read", "archive", archive, "group", group,
            "expected", result == null ? null : OracleMain.hex(result));
    }
    /** XORs bytes of a closed file; BufferedFile caches reads, so the files are reopened around it. */
    private static void mutate(String name, long offset, int count) throws Exception {
        close();
        RandomAccessFile raw = new RandomAccessFile(file(name).toFile(), "rw");
        byte[] bytes = new byte[count];
        raw.seek(offset);
        raw.readFully(bytes);
        for (int i = 0; i < count; i++) bytes[i] ^= (byte) 0xa5;
        raw.seek(offset);
        raw.write(bytes);
        raw.close();
        open();
        OracleMain.emit("kind", "mutate", "file", name, "offset", offset, "count", count);
    }
    private static void truncate(long length) throws Exception {
        close();
        RandomAccessFile raw = new RandomAccessFile(file("dat2").toFile(), "rw");
        raw.setLength(length);
        raw.close();
        open();
        OracleMain.emit("kind", "truncate", "length", length);
    }
    private static int sector(int archive, int group) throws Exception {
        close();
        byte[] index = Files.readAllBytes(file("idx" + archive));
        open();
        int at = group * 6 + 3;
        return (index[at] & 255) << 16 | (index[at + 1] & 255) << 8 | index[at + 2] & 255;
    }
    private static long length() throws Exception {
        close();
        long length = Files.size(file("dat2"));
        open();
        return length;
    }
    private static void files() throws Exception {
        close();
        Map<String, Object> index = new LinkedHashMap<String, Object>();
        for (int archive : ARCHIVES) index.put(Integer.toString(archive), digest(file("idx" + archive)));
        OracleMain.emit("kind", "files", "data", digest(file("dat2")), "indexes", index);
        open();
    }
    /** Length and SHA-256: one index file is mostly zeros and 420 KB long. */
    private static Object digest(Path path) throws Exception {
        byte[] bytes = Files.readAllBytes(path);
        return OracleMain.map("length", bytes.length,
            "sha256", OracleMain.hex(java.security.MessageDigest.getInstance("SHA-256").digest(bytes)));
    }

    public static void main(String[] args) throws Exception {
        directory = Paths.get(args[0]);
        Files.createDirectories(directory);
        try (DirectoryStream<Path> existing = Files.newDirectoryStream(directory)) {
            if (existing.iterator().hasNext()) throw new IllegalStateException("work directory is not empty");
        }
        open();
        // Fresh appends around the 512-byte sector payload, sparse and out-of-order group IDs.
        int[][] first = {{3, 0, 1}, {3, 1, 511}, {3, 2, 512}, {3, 3, 513}, {7, 9, 1024}, {3, 5, 1500},
            {7, 0, 1025}, {255, 3, 300}, {7, 300, 2600}, {3, 4, 0}};
        for (int i = 0; i < first.length; i++) write(first[i][0], first[i][1], first[i][2], i);
        for (int[] step : first) read(step[0], step[1]);
        read(3, 6); read(3, 400); read(7, 3); read(255, 0);
        // A zero-length group written while the file ends on a sector boundary, and one more after it.
        write(7, 1, 512, 40); write(7, 2, 0, 41); read(7, 2); write(7, 4, 10, 42); read(7, 2); read(7, 4);
        // Overwrites: shorter, same sector count, longer (reuses the chain, then appends), much shorter.
        write(3, 5, 700, 50); read(3, 5);
        write(3, 2, 512, 51); read(3, 2);
        write(3, 5, 3000, 52); read(3, 5);
        write(7, 300, 5, 53); read(7, 300);
        write(7, 300, 2600, 54); read(7, 300);
        // A group ID that does not fit the 16-bit sector header.
        write(7, 70000, 600, 60); read(7, 70000); write(7, 70000, 600, 61); read(7, 70000);
        files();
        // A sector that claims another group, part or archive: the chain is abandoned for a new one.
        int chain = sector(3, 5);
        mutate("dat2", chain * 520L + 1, 1); read(3, 5); write(3, 5, 1400, 70); read(3, 5);
        chain = sector(7, 9);
        mutate("dat2", chain * 520L + 3, 1); read(7, 9); write(7, 9, 1024, 71); read(7, 9);
        chain = sector(7, 0);
        mutate("dat2", chain * 520L + 7, 1); read(7, 0); write(7, 0, 1025, 72); read(7, 0);
        // A next-sector link past the end of the file.
        chain = sector(3, 3);
        mutate("dat2", chain * 520L + 4, 1); read(3, 3); write(3, 3, 513, 73); read(3, 3);
        // An index entry whose sector lies past the end, and one whose six bytes are all scrambled.
        mutate("idx3", 1 * 6 + 3, 1); read(3, 1); write(3, 1, 511, 74); read(3, 1);
        mutate("idx3", 0, 6); read(3, 0); write(3, 0, 1, 75); read(3, 0);
        files();
        // The file cut at the first sector of the last chain: sector boundary, then inside its header,
        // then just past its header.
        write(255, 9, 1200, 80); read(255, 9);
        long start = sector(255, 9) * 520L;
        truncate(start); read(255, 9); write(255, 9, 1200, 81); read(255, 9);
        write(255, 10, 700, 82); read(255, 10); read(255, 9);
        start = sector(255, 10) * 520L;
        truncate(start + 5); read(255, 10); write(255, 10, 700, 83); read(255, 10);
        write(255, 11, 900, 84); read(255, 11);
        start = sector(255, 11) * 520L;
        truncate(start + 8); read(255, 11); write(255, 11, 900, 85); read(255, 11);
        // A file that no longer ends on a sector boundary keeps appending at the next whole sector.
        truncate(length() - 3); write(3, 20, 100, 90); read(3, 20); write(3, 21, 600, 91); read(3, 21);
        for (int[] step : first) read(step[0], step[1]);
        files();
        close();
    }
}
