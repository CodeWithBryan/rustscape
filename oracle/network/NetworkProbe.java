import rt4.*;
import java.io.*;
import java.net.Socket;
import java.nio.ByteBuffer;
import java.nio.file.*;
import java.lang.reflect.*;
import java.security.MessageDigest;
import java.util.*;

/** Original client queue receives bytes from the real Rust TCP server. */
public final class NetworkProbe {
    private static BufferedFile open(Path file) throws Exception {
        return new BufferedFile(new FileOnDisk(file.toFile(), "r", Long.MAX_VALUE), 4096, 0);
    }
    private static Js5NetQueue connect(String host, int port, int key) throws Exception {
        Socket socket = new Socket(host, port); socket.setSoTimeout(3000); socket.setTcpNoDelay(true);
        socket.getOutputStream().write(ByteBuffer.allocate(5).put((byte) 15).putInt(530).array());
        if (socket.getInputStream().read() != 0) throw new AssertionError("handshake");
        Js5NetQueue queue = new Js5NetQueue();
        Field encryption = Js5NetQueue.class.getDeclaredField("encryptionKey"); encryption.setAccessible(true);
        encryption.setByte(queue, (byte) key);
        queue.start(false, new BufferedSocket(socket));
        return queue;
    }
    private static void complete(Js5NetQueue queue, List<Js5NetRequest> requests) throws Exception {
        long deadline = System.nanoTime() + 10_000_000_000L;
        while (true) {
            if (!queue.loop() || queue.errors != 0) throw new AssertionError("original queue rejected server reply");
            boolean pending = false;
            for (Js5NetRequest request : requests) pending |= request.incomplete;
            if (!pending) return;
            if (System.nanoTime() > deadline) throw new AssertionError("original queue timed out");
            Thread.sleep(1);
        }
    }
    private static byte[] expected(byte[] packed, int trailer) {
        int length = ByteBuffer.wrap(packed, 1, 4).getInt() + (packed[0] == 0 ? 5 : 9);
        byte[] expected = Arrays.copyOf(packed, length + trailer);
        Arrays.fill(expected, length, expected.length, (byte) 0);
        return expected;
    }
    public static void main(String[] args) throws Exception {
        String host = args[0]; int port = Integer.parseInt(args[1]); Path directory = Paths.get(args[2]);
        BufferedFile data = open(directory.resolve("main_file_cache.dat2"));
        BufferedFile index255 = open(directory.resolve("main_file_cache.idx255"));
        Cache references = new Cache(255, data, index255, 500000);
        List<Integer> archives = new ArrayList<Integer>();
        for (int id = 0; id < 255; id++)
            if (Files.isRegularFile(directory.resolve("main_file_cache.idx" + id))) archives.add(id);
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        int count = 0;
        core.cache.CacheFileManager[] managers = new core.cache.CacheFileManager[(int) index255.length() / 6];
        for (int archive : archives) {
            byte[] raw = references.read(archive);
            Js5Index index = new Js5Index(raw, Buffer.crc32(raw, raw.length));
            managers[archive] = new core.cache.CacheFileManager(new core.cache.CacheFile(group -> null), index.checksum, index.version);
        }
        NetworkOracle.install(managers, new core.cache.CacheFile(group -> references.read(group)));
        Js5NetQueue masterQueue = connect(host, port, 0);
        try {
            Js5NetRequest request = masterQueue.read(255, (byte) 0, 255, true);
            complete(masterQueue, Collections.singletonList(request));
            byte[] payload = core.cache.Cache.generateReferenceData();
            byte[] expected = ByteBuffer.allocate(payload.length + 5).put((byte) 0).putInt(payload.length).put(payload).array();
            if (!Arrays.equals(request.getData(), expected)) throw new AssertionError("master index bytes");
            digest.update(request.getData()); count++;
        } finally {masterQueue.quit();}
        // The cache-only service exposes an explicit static world stamp of 1.
        core.game.world.GameWorld.ticks = 1;
        core.net.lobby.WorldList.flagUpdate();
        for (int stamp : new int[] {0, 1}) {
            core.net.IoSession session = new core.net.IoSession();
            core.net.lobby.WorldList.sendUpdate(session, stamp);
            try (Socket socket = new Socket(host, port)) {
                socket.setSoTimeout(3000);
                socket.getOutputStream().write(ByteBuffer.allocate(5).put((byte) 255).putInt(stamp).array());
                DataInputStream input = new DataInputStream(socket.getInputStream());
                int status = input.readUnsignedByte(), length = input.readUnsignedShort();
                byte[] received = new byte[length + 3];
                ByteBuffer.wrap(received).put((byte) status).putShort((short) length);
                input.readFully(received, 3, length);
                if (!Arrays.equals(received, session.responses.get(0))) throw new AssertionError("world list bytes");
                digest.update(received); count++;
            }
        }
        for (int key : new int[] {0, 1, 128, 255}) {
            for (int archive : archives) {
                byte[] reference = references.read(archive);
                Js5Index index = new Js5Index(reference, Buffer.crc32(reference, reference.length));
                int group = -1;
                for (int id : index.groupIds) if (index.groupCapacities[id] != 0) {group = id; break;}
                if (group == -1) continue;
                BufferedFile idx = open(directory.resolve("main_file_cache.idx" + archive));
                byte[] packed = new Cache(archive, data, idx, 1000000).read(group);
                idx.close();
                for (boolean urgent : new boolean[] {true, false}) {
                    Js5NetQueue queue = connect(host, port, key);
                    try {
                        Js5NetRequest request = queue.read(archive, (byte) 2, group, urgent);
                        complete(queue, Collections.singletonList(request));
                        byte[] received = request.getData();
                        if (!Arrays.equals(received, expected(packed, 2))) throw new AssertionError(archive + "/" + group + " bytes");
                        digest.update(received); count++;
                    } finally {queue.quit();}
                }
            }
            // Coalesced/in-flight requests exercise the original urgent/prefetch matching.
            Js5NetQueue queue = connect(host, port, key);
            try {
                List<Js5NetRequest> requests = new ArrayList<Js5NetRequest>();
                for (int archive : archives.subList(0, Math.min(20, archives.size())))
                    requests.add(queue.read(255, (byte) 0, archive, archive % 2 == 0));
                complete(queue, requests);
                for (int i = 0; i < requests.size(); i++) {
                    byte[] received = requests.get(i).getData();
                    byte[] packed = references.read(archives.get(i));
                    if (!Arrays.equals(received, expected(packed, 0))) throw new AssertionError("reference index bytes");
                    digest.update(received); count++;
                }
            } finally {queue.quit();}
        }
        index255.close(); data.close();
        OracleMain.emit("requests", count, "sha256", OracleMain.hex(digest.digest()),
            "scope", "Unmodified original Js5NetQueue receiving real Rust server TCP replies");
    }
}
