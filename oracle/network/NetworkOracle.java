import core.cache.Cache;
import core.cache.CacheFile;
import core.cache.CacheFileManager;
import core.net.*;
import core.net.event.*;
import core.net.producer.JS5EventProducer;
import core.net.lobby.*;
import core.game.world.GameWorld;
import core.game.world.repository.Repository;
import java.lang.reflect.*;
import java.nio.*;
import java.util.*;

/** Complete original server formatters/handlers, with explicit I/O and metadata adapters. */
public final class NetworkOracle {
    static void field(Class<?> owner, String name, Object value) throws Exception {
        Field field = owner.getDeclaredField(name); field.setAccessible(true); field.set(null, value);
    }
    static void install(CacheFileManager[] managers, CacheFile reference) throws Exception {
        field(Cache.class, "cacheFileManagers", managers);
        field(Cache.class, "referenceFile", reference);
        field(JS5WriteEvent.class, "cachedReference", null);
    }
    static byte[] bytes(ByteBuffer buffer) {
        byte[] output = new byte[buffer.remaining()]; buffer.get(output); return output;
    }
    private static void archivePackets() throws Exception {
        Random random = new Random(530);
        int sequence = 0;
        for (int length : new int[] {0, 1, 2, 498, 499, 500, 501, 502, 503, 504, 505, 506, 510, 511, 512, 513, 1014, 1015, 1016, 1017, 1527, 1528, 1529, 8192}) {
            for (int compression : new int[] {0, 1, 2}) for (boolean version : new boolean[] {false, true}) {
                int archive = new int[] {0, 5, 255}[sequence % 3];
                int group = sequence++ % 2 == 0 ? 0 : 65535;
                byte[] content = new byte[length]; random.nextBytes(content);
                ByteBuffer input = ByteBuffer.allocate(5 + length + (compression == 0 ? 0 : 4) + (version ? 2 : 0));
                input.put((byte) compression).putInt(length);
                if (compression != 0) input.putInt(length * 3 + 7);
                input.put(content);
                if (version) input.putShort((short) 530);
                final byte[] packed = input.array();
                CacheFile provider = new CacheFile(id -> id == group ? packed : null);
                CacheFileManager[] managers = new CacheFileManager[256];
                managers[archive] = new CacheFileManager(provider, 0, 0);
                install(managers, provider);
                for (int key : new int[] {0, 1, -128, -1}) for (boolean urgent : new boolean[] {false, true}) {
                    byte[] expected = bytes(Cache.getArchiveData(archive, group, urgent, key));
                    OracleMain.emit("kind", "archive_reply", "archive", archive, "group", group,
                        "urgent", urgent, "key", key, "input", OracleMain.hex(packed), "expected", OracleMain.hex(expected));
                }
            }
        }
    }
    private static void masters() throws Exception {
        Random random = new Random(2009);
        for (int count : new int[] {0, 1, 29, 63, 64, 70, 128, 256}) {
            CacheFileManager[] managers = new CacheFileManager[count];
            List<Object> inputs = new ArrayList<Object>();
            for (int i = 0; i < count; i++) {
                if (i % 5 == 0 || i == 24) {inputs.add(null); continue;}
                int crc = random.nextInt(), revision = random.nextInt();
                managers[i] = new CacheFileManager(new CacheFile(id -> null), crc, revision);
                inputs.add(new int[] {crc, revision});
            }
            install(managers, new CacheFile(id -> null));
            byte[] payload = Cache.generateReferenceData();
            OracleMain.emit("kind", "master_payload", "entries", inputs, "expected", OracleMain.hex(payload));
            if (count != 0) {
                for (int key : new int[] {0, -128}) for (boolean urgent : new boolean[] {false, true}) {
                    IoSession session = new IoSession(); session.setJs5Encryption(key);
                    new JS5WriteEvent(session, new int[] {255, 255, urgent ? 1 : 0}).run();
                    OracleMain.emit("kind", "master_reply", "input", OracleMain.hex(payload),
                        "key", key, "urgent", urgent, "expected", OracleMain.hex(session.responses.get(0)));
                }
            }
        }
    }
    private static void handshakes() throws Exception {
        for (long seed : new long[] {0, 1, -1, 530, Long.MIN_VALUE, Long.MAX_VALUE}) {
            for (int nameHash : new int[] {0, 1, 127, 128, 255}) {
                core.tools.RandomFunction.RANDOM.setSeed(seed);
                IoSession session = new IoSession();
                byte[] input = new byte[] {14, (byte) nameHash};
                new HSReadEvent(session, ByteBuffer.wrap(input)).run();
                OracleMain.emit("kind", "login_handshake", "input", OracleMain.hex(input),
                    "seed", seed, "nameHash", session.getNameHash(), "serverKey", session.getServerKey(),
                    "expected", OracleMain.hex(session.responses.get(0)),
                    "phase", session.getProducer().getClass().getSimpleName(), "closed", session.disconnected);
            }
        }
        for (int revision : new int[] {0, 289, 529, 530, 531, -1, Integer.MIN_VALUE, Integer.MAX_VALUE}) {
            IoSession session = new IoSession();
            byte[] input = ByteBuffer.allocate(5).put((byte) 15).putInt(revision).array();
            new HSReadEvent(session, ByteBuffer.wrap(input)).run();
            OracleMain.emit("kind", "cache_handshake", "input", OracleMain.hex(input), "revision", revision,
                "expected", session.responses.isEmpty() ? "" : OracleMain.hex(session.responses.get(0)),
                "closed", session.disconnected, "phase", session.getProducer().getClass().getSimpleName());
        }
    }
    private static void requestStreams() throws Exception {
        Random random = new Random(530);
        for (int scenario = 0; scenario < 8; scenario++) {
            ByteBuffer stream = ByteBuffer.allocate(4096);
            for (int i = 0; i < 60; i++) {
                int opcode = new int[] {0, 1, 2, 3, 4, 5, 6, 9, 88}[random.nextInt(9)];
                stream.put((byte) opcode);
                if (opcode == 5 || opcode == 9) stream.putInt(random.nextInt());
                else if (opcode == 4) stream.put((byte) random.nextInt()).putShort((short) 0);
                else stream.put((byte) random.nextInt()).putShort((short) random.nextInt());
            }
            if (scenario % 3 == 0) stream.put((byte) 7).put(new byte[3]);
            else if (scenario % 3 == 1) stream.put((byte) 4).put((byte) 255).putShort((short) 1);
            else stream.put((byte) 1).put((byte) 5); // a fragmented final request
            byte[] input = Arrays.copyOf(stream.array(), stream.position());
            for (int chunk : new int[] {1, 2, 3, 4, 5, 7, 32, 1024}) {
                IoSession session = new IoSession(); session.setProducer(new JS5EventProducer());
                for (int offset = 0; offset < input.length && !session.disconnected; offset += chunk) {
                    byte[] part = Arrays.copyOfRange(input, offset, Math.min(input.length, offset + chunk));
                    new JS5ReadEvent(session, ByteBuffer.wrap(part)).run();
                }
                int pending = session.getReadingQueue() == null ? 0 : session.getReadingQueue().position();
                OracleMain.emit("kind", "request_stream", "input", OracleMain.hex(input), "chunk", chunk,
                    "requests", session.requests, "key", session.getJs5Encryption(),
                    "closed", session.disconnected, "pending", pending);
            }
        }
    }
    @SuppressWarnings("unchecked")
    private static void worlds() throws Exception {
        Field listField = WorldList.class.getDeclaredField("WORLD_LIST"); listField.setAccessible(true);
        List<WorldDefinition> list = (List<WorldDefinition>) listField.get(null);
        for (int count : new int[] {1, 3}) for (int stamp : new int[] {0, 1, 530, Integer.MAX_VALUE, Integer.MIN_VALUE}) {
            list.clear(); GameWorld.ticks = stamp; Repository.players.clear();
            for (int i = 0; i < 128; i++) Repository.players.add(new Object());
            List<Object> entries = new ArrayList<Object>();
            for (int i = 0; i < count; i++) {
                int id = i == 0 ? 1 : 127 + i;
                WorldDefinition world = new WorldDefinition(id, i, i == 0 ? 9 : -1,
                    i == 0 ? "2009Scape Classic" : "World " + id, "127.0.0." + (i+1),
                    "Anywhere, USA", i == 0 ? 225 : 16);
                world.setPlayers(65535);
                WorldList.addWorld(world);
                entries.add(OracleMain.map("id", id, "location", i, "flags", world.getFlag(),
                    "activity", world.getActivity(), "host", world.getIp(), "region", world.getRegion(),
                    "country", world.getCountry(), "players", world.getPlayerCount()));
            }
            for (int requested : new int[] {stamp, stamp ^ 1}) {
                IoSession session = new IoSession();
                WorldList.sendUpdate(session, requested);
                OracleMain.emit("kind", "world_list", "worlds", entries, "stamp", stamp, "requested", requested,
                    "expected", OracleMain.hex(session.responses.get(0)));
            }
        }
    }
    public static void main(String[] args) throws Exception {
        archivePackets(); masters(); handshakes(); requestStreams(); worlds();
        rt4.Js5NetQueue queue = new rt4.Js5NetQueue();
        int urgent = 0, prefetch = 0;
        for (; urgent < 30; urgent++) {
            try {queue.read(3, (byte) 0, urgent, true);}
            catch (RuntimeException full) {break;}
        }
        for (; prefetch < 30; prefetch++) {
            try {queue.read(3, (byte) 0, prefetch, false);}
            catch (RuntimeException full) {break;}
        }
        OracleMain.emit("kind", "client_queue_bounds", "urgent", urgent, "prefetch", prefetch);
    }
}
