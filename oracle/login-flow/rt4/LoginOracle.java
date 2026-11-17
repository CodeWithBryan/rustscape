package rt4;

import java.io.IOException;
import java.io.PrintStream;
import java.net.SocketTimeoutException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * Runs scripted scenarios against the original LoginManager loop methods, CreateManager and
 * WorldList, and prints a trace the Rust test reproduces.
 *
 * Original code executed: LoginManager.clear/loopAuto/loop/continueDelayedLogin/startLogin
 * (sliced unchanged from the reference file), CreateManager, WorldList, World, WorldInfo,
 * AbstractWorld, Base37, JagString, Buffer, Packet, IsaacRandom, GlobalConfig.
 * Transcribed here (NOT original code): the ScriptRunner guards, client.setGameState's
 * login-relevant lines, the game-state 40 branch of client.mainLoop, handleConnectionLost,
 * client.mainInit's port arithmetic, the host and port a JS5 connection attempt sets
 * (client.java 1320-1323, command js5) and the 5610 command.
 */
public final class LoginOracle {
    static final class Wire {
        final ArrayDeque<Integer> in = new ArrayDeque<>();
        boolean eof;
        boolean ioerr;
        void check() throws IOException {
            if (this.ioerr) {
                this.ioerr = false;
                throw new IOException("scripted");
            }
        }
    }
    static final class ScriptedSocket extends java.net.Socket {
        final Wire wire;
        ScriptedSocket(Wire wire) { this.wire = wire; }
    }

    static PrintStream out;
    static List<String[]> commands = new ArrayList<>();
    static List<String> raw = new ArrayList<>();
    static int pc;
    static PrivilegedRequest request;
    static Wire current;
    static int pendingTicks;
    static boolean inFetch;
    static boolean titleScene;
    static String lastSig = "";
    static final List<String> events = new ArrayList<>();

    static String hex(byte[] bytes, int len) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < len; i++) sb.append(String.format("%02x", bytes[i] & 0xFF));
        return sb.length() == 0 ? "-" : sb.toString();
    }
    static String hex(JagString s) {
        if (s == null) return "null";
        byte[] b = s.toByteArray();
        return hex(b, b.length);
    }
    static byte[] bytes(String token) {
        if (token.equals("-")) return new byte[0];
        if (token.startsWith("'")) return token.substring(1).getBytes(StandardCharsets.ISO_8859_1);
        byte[] b = new byte[token.length() / 2];
        for (int i = 0; i < b.length; i++) b[i] = (byte) Integer.parseInt(token.substring(i * 2, i * 2 + 2), 16);
        return b;
    }
    static JagString str(String token) {
        byte[] b = bytes(token);
        return JagString.decodeString(b, b.length, 0);
    }

    static PrivilegedRequest open(String host, int port) {
        out.println("open " + host + " " + port);
        request = new PrivilegedRequest();
        return request;
    }

    static String stateLine(boolean signature) {
        String loops = signature ? "*" : Integer.toString(LoginManager.loops);
        String hopLoops = signature ? "*" : Integer.toString(LoginManager.hopLoops);
        String createLoops = signature ? "*" : Integer.toString(CreateManager.loops);
        return "st g=" + client.gameState
            + " L=" + LoginManager.step + "/" + loops + "/" + LoginManager.errors + "/" + LoginManager.reply
            + " H=" + LoginManager.hopStep + "/" + hopLoops + "/" + LoginManager.hopErrors + "/" + LoginManager.hopTime
            + " d=" + LoginManager.disallowResult
            + " C=" + CreateManager.step + "/" + createLoops + "/" + CreateManager.errors + "/" + CreateManager.reply
            + " W=" + WorldList.step + "/" + WorldList.errors + "/" + (WorldList.loaded ? 1 : 0) + "/" + WorldList.size
            + " P=" + client.hostname + ":" + client.port + "/" + client.defaultPort + "/" + client.alternatePort
            + " WL=" + client.worldListHostname + ":" + client.worldListPort
            + " w=" + Player.worldId
            + " s=" + (Protocol.socket != null ? 1 : 0)
            + " p=" + (Protocol.previousSocket != null ? 1 : 0)
            + " r=" + (Protocol.socketRequest != null ? 1 : 0);
    }
    static void printState() {
        out.println(stateLine(false));
        lastSig = stateLine(true);
    }
    static void afterTick() {
        if (Preferences.sentToServer) {
            Preferences.sentToServer = false;
            out.println("ev prefs");
        }
        for (String event : events) out.println(event);
        events.clear();
        String sig = stateLine(true);
        if (!sig.equals(lastSig) || pendingTicks == 0) printState();
    }

    static String[] next() {
        if (pc >= commands.size()) throw new IllegalStateException("scenario ended inside a blocking read");
        out.println("> " + raw.get(pc));
        return commands.get(pc++);
    }
    static void socketCommand(String[] c) {
        switch (c[0]) {
            case "recv":
                for (int i = 1; i < c.length; i++) for (byte b : bytes(c[i])) current.in.add(b & 0xFF);
                break;
            case "recvlen": {
                int total = 0;
                for (int i = 1; i < c.length; i++) total += bytes(c[i]).length;
                current.in.add(total >> 8 & 0xFF);
                current.in.add(total & 0xFF);
                for (int i = 1; i < c.length; i++) for (byte b : bytes(c[i])) current.in.add(b & 0xFF);
                break;
            }
            case "eof":
                current.eof = true;
                break;
            case "ioerr":
                current.ioerr = true;
                break;
            default:
                throw new IllegalStateException("command not allowed during a blocking read: " + c[0]);
        }
        printState();
    }

    /** BufferedSocket.read(): the original blocks here; the scenario supplies what happens meanwhile. */
    static int blockingRead(Wire wire) throws IOException {
        wire.check();
        if (!wire.in.isEmpty()) return wire.in.poll();
        if (wire.eof) return -1;
        if (inFetch) {
            out.println("ret -1");
            printState();
        } else {
            afterTick();
        }
        int waited = 1;
        while (true) {
            boolean ready = wire.ioerr || !wire.in.isEmpty() || wire.eof;
            if (inFetch) {
                String[] c = next();
                if (c[0].equals("fetch")) {
                    MonotonicClock.now = Long.parseLong(c[1]);
                    if (ready) break;
                    out.println("ret -1");
                    printState();
                } else {
                    socketCommand(c);
                }
                continue;
            }
            if (pendingTicks == 0) {
                String[] c = next();
                if (c[0].equals("tick")) pendingTicks = Integer.parseInt(c[1]);
                else socketCommand(c);
                continue;
            }
            pendingTicks--;
            if (ready) break;
            waited++;
            if (waited > 1500) throw new SocketTimeoutException();
            afterTick();
        }
        wire.check();
        if (!wire.in.isEmpty()) return wire.in.poll();
        return -1;
    }

    // ----- transcribed client pieces -----

    static void setGameState(int state) {
        if (client.gameState == state) return;
        if (state == 40) LoginManager.clear();
        if (state != 40 && Protocol.previousSocket != null) {
            Protocol.previousSocket.close();
            Protocol.previousSocket = null;
        }
        client.gameState = state;
    }
    static void processLogout() {
        if (Protocol.socket != null) {
            Protocol.socket.close();
            Protocol.socket = null;
        }
        setGameState(titleScene ? 28 : 10);
    }
    static void sessionReset() {
        // client.resetGameSessionState: nothing the trace observes.
    }
    static void rebuild(boolean dynamic) {
        events.add("ev entered staff=" + LoginManager.staffModLevel + " marks=" + LoginManager.blackmarks
            + " underage=" + LoginManager.playerUnderage + " chat=" + LoginManager.parentalChatConsent
            + " advert=" + LoginManager.parentalAdvertConsent + " quick=" + LoginManager.mapQuickChat
            + " mouse=" + MouseRecorder.enabled + " self=" + PlayerList.selfId
            + " member=" + LoginManager.playerMember + " members=" + LoginManager.mapMembers
            + " opcode=" + Protocol.opcode + " rebuild=" + hex(Protocol.inboundBuffer.data, Protocol.length));
        // readRebuildPacket ends in LoginManager.loadRegion, which calls setGameState(25).
        setGameState(25);
    }
    static boolean idle(boolean withWorldList) {
        return client.gameState == 10 && LoginManager.hopStep == 0 && LoginManager.step == 0 && CreateManager.step == 0
            && (!withWorldList || WorldList.step == 0);
    }
    static void cycle() {
        if (client.gameState == 10) {
            CreateManager.loop();
            LoginManager.loopAuto();
            LoginManager.loop();
        } else if (client.gameState == 40) {
            LoginManager.loop();
            if (LoginManager.reply != -3) {
                if (LoginManager.reply == 15) {
                    setGameState(30);
                    events.add("ev reconnected");
                } else if (LoginManager.reply != 2) {
                    int reply = LoginManager.reply;
                    processLogout();
                    events.add("ev loggedout reply=" + reply);
                }
            }
        }
    }
    static void config(String[] c) {
        client.modeWhere = Integer.parseInt(c[1]);
        client.worldListId = Integer.parseInt(c[2]);
        boolean selectDefault = c[3].equals("1");
        titleScene = c[4].equals("1");
        GlobalJsonConfig.instance = null;
        if (c.length > 5) {
            GlobalJsonConfig json = new GlobalJsonConfig();
            json.ip_management = c[5];
            json.ip_address = c[6];
            json.world = Integer.parseInt(c[7]);
            json.server_port = Integer.parseInt(c[8]);
            json.wl_port = Integer.parseInt(c[9]);
            GlobalJsonConfig.instance = json;
        }
        // client.mainInit
        if (client.modeWhere == 0) {
            client.worldListHostname = "host";
            client.worldListAlternatePort = GlobalConfig.ALTERNATE_PORT + 1;
            client.worldListDefaultPort = GlobalConfig.DEFAULT_PORT + 1;
        } else {
            client.worldListHostname = "host";
            client.worldListAlternatePort = GlobalConfig.ALTERNATE_PORT + client.worldListId;
            client.worldListDefaultPort = GlobalConfig.DEFAULT_PORT + client.worldListId;
        }
        if (GlobalJsonConfig.instance != null) {
            client.worldListHostname = GlobalJsonConfig.instance.ip_address;
            client.worldListAlternatePort = GlobalJsonConfig.instance.server_port + client.worldListId;
            client.worldListDefaultPort = GlobalJsonConfig.instance.server_port;
        }
        client.alternatePort = client.worldListAlternatePort;
        client.defaultPort = client.worldListDefaultPort;
        client.hostname = client.worldListHostname;
        client.worldListPort = client.worldListDefaultPort;
        client.port = client.worldListPort;
        Player.worldId = selectDefault ? client.worldListId : -1;
    }

    static void reset() {
        LoginManager.hopStep = 0;
        LoginManager.pingFailed = false;
        LoginManager.hopTime = 0;
        LoginManager.reply = -2;
        LoginManager.loops = 0;
        LoginManager.step = 0;
        LoginManager.disallowResult = -1;
        LoginManager.errors = 0;
        LoginManager.hopLoops = 0;
        LoginManager.hopErrors = 0;
        LoginManager.serverKey = 0L;
        LoginManager.loginType = -1;
        CreateManager.step = 0;
        CreateManager.loops = 0;
        CreateManager.errors = 0;
        CreateManager.reply = -2;
        CreateManager.suggestedNames = null;
        WorldList.step = 0;
        WorldList.openTime = 0L;
        WorldList.closeTime = 0L;
        WorldList.loaded = false;
        WorldList.bufferLen = 0;
        WorldList.buffer = null;
        WorldList.bufferOff = 0;
        WorldList.sorted = new World[0];
        WorldList.size = 0;
        WorldList.checksum = 0;
        WorldList.countries = null;
        WorldList.minId = 0;
        WorldList.maxId = 0;
        WorldList.worlds = null;
        WorldList.errors = 0;
        WorldList.worldPos = 1;
        Protocol.socket = null;
        Protocol.previousSocket = null;
        Protocol.socketRequest = null;
        Protocol.outboundBuffer.offset = 0;
        Protocol.verifyId = 0;
        Protocol.opcode = -1;
        Protocol.length = 0;
        Player.usernameInput = JagString.EMPTY;
        Player.password = null;
        Player.name37 = 0L;
        client.gameState = 0;
        client.settings = JagString.EMPTY;
        client.affiliate = 0;
        client.advertSuppressed = false;
        client.uid = new byte[24];
        for (Js5 archive : client.archives()) archive.crc = 0;
        DisplayMode.mode = 0;
        GameShell.canvasWidth = 765;
        GameShell.canvasHeight = 503;
        Preferences.antiAliasingMode = 0;
        Preferences.value = 0;
        Preferences.sentToServer = false;
        MonotonicClock.now = 0L;
        Math.queue.clear();
        request = null;
        current = null;
        pendingTicks = 0;
        inFetch = false;
        lastSig = "";
        events.clear();
        config(new String[]{"config", "0", "1", "1", "0"});
    }

    static void describe(String prefix, World world, boolean withId) {
        if (world == null) {
            out.println(prefix + " null");
            return;
        }
        WorldInfo info = world.getWorldInfo();
        out.println(prefix + (withId ? " " + world.id : "") + " " + world.flags + " " + hex(world.activity) + " "
            + info.flag + " " + hex(info.name) + " " + world.players);
    }

    static void exec(String[] c) {
        switch (c[0]) {
            case "scenario":
                reset();
                return;
            case "config":
                config(c);
                break;
            case "random":
                for (int i = 1; i < c.length; i++) Math.queue.add(Double.parseDouble(c[i]));
                return;
            case "js5":
                // Transcribed (not original code): client.java 1320-1323, the start of
                // every JS5 connection attempt (js5ConnectState 0), which leaves
                // client.hostname and client.port for the requests that follow.
                if (GlobalJsonConfig.instance != null) {
                    client.hostname = GlobalJsonConfig.instance.ip_management;
                    client.port = GlobalJsonConfig.instance.server_port + client.worldListId;
                }
                break;
            case "env": {
                DisplayMode.mode = Integer.parseInt(c[1]);
                GameShell.canvasWidth = Integer.parseInt(c[2]);
                GameShell.canvasHeight = Integer.parseInt(c[3]);
                Preferences.antiAliasingMode = Integer.parseInt(c[4]);
                byte[] uid = bytes(c[5]);
                client.uid = new byte[24];
                System.arraycopy(uid, 0, client.uid, 0, java.lang.Math.min(24, uid.length));
                client.settings = str(c[6]);
                client.affiliate = Integer.parseInt(c[7]);
                Preferences.value = Integer.parseInt(c[8]);
                Protocol.verifyId = Integer.parseInt(c[9]);
                int base = Integer.parseInt(c[10]);
                Js5[] archives = client.archives();
                for (int i = 0; i < 28; i++) archives[i].crc = base + i * 7919;
                client.advertSuppressed = c[11].equals("1");
                return;
            }
            case "state":
                client.gameState = Integer.parseInt(c[1]);
                break;
            case "setstate":
                setGameState(Integer.parseInt(c[1]));
                break;
            case "login":
                if (idle(true)) {
                    try {
                        LoginManager.startLogin(str(c[1]), str(c[2]), Integer.parseInt(c[3]));
                    } catch (RuntimeException ex) {
                        out.println("err");
                    }
                }
                break;
            case "continue":
                LoginManager.continueDelayedLogin();
                break;
            case "resetlogin":
                if (LoginManager.step == 0) LoginManager.reply = -2;
                break;
            case "checkinfo":
                if (idle(true)) {
                    CreateManager.checkInfo(Integer.parseInt(c[3]), Integer.parseInt(c[4]), Integer.parseInt(c[1]), Integer.parseInt(c[2]));
                }
                break;
            case "checkname":
                if (idle(true)) {
                    try {
                        CreateManager.checkName(str(c[1]).encode37());
                    } catch (RuntimeException ex) {
                        out.println("err");
                    }
                }
                break;
            case "create":
                if (idle(true)) {
                    client.affiliate = Integer.parseInt(c[7]);
                    try {
                        CreateManager.createAccount(Integer.parseInt(c[1]), Integer.parseInt(c[4]), Integer.parseInt(c[2]),
                            str(c[6]), str(c[5]).encode37(), Integer.parseInt(c[3]));
                    } catch (RuntimeException ex) {
                        out.println("err");
                    }
                }
                break;
            case "resetcreate":
                if (CreateManager.step == 0) CreateManager.reply = -2;
                break;
            case "names":
                try {
                    StringBuilder sb = new StringBuilder("names");
                    for (int i = 0; i < 5; i++) {
                        sb.append(' ').append(CreateManager.suggestedNames.length > i ? hex(CreateManager.suggestedNames[i].toTitleCase()) : "-");
                    }
                    CreateManager.suggestedNames = null;
                    out.println(sb);
                } catch (RuntimeException ex) {
                    out.println("names err");
                }
                break;
            case "outbound":
                out.println("outbound " + hex(Protocol.outboundBuffer.data, Protocol.outboundBuffer.offset));
                return;
            case "fetch": {
                MonotonicClock.now = Long.parseLong(c[1]);
                inFetch = true;
                int value = WorldList.fetch();
                inFetch = false;
                out.println("ret " + value);
                break;
            }
            case "hop":
                out.println("ret " + (idle(false) ? (WorldList.hopWorld(Integer.parseInt(c[1])) ? 1 : 0) : 0));
                break;
            case "sort":
                try {
                    WorldList.sortWorldList(Integer.parseInt(c[3]), c[2].equals("1"), Integer.parseInt(c[1]), c[4].equals("1"));
                } catch (RuntimeException ex) {
                    out.println("err");
                }
                return;
            case "list": {
                int n = 0;
                for (World world = WorldList.getFirstWorld(); world != null; world = WorldList.getNextWorld()) {
                    describe("w", world, true);
                    n++;
                }
                out.println("listed " + n);
                return;
            }
            case "get":
                describe("g", ScriptRunner.getWorld(Integer.parseInt(c[1])), false);
                return;
            case "connect":
                if (c[1].equals("ok")) {
                    current = new Wire();
                    request.result = new ScriptedSocket(current);
                    request.status = 1;
                } else {
                    request.status = 2;
                }
                return;
            case "recv":
            case "recvlen":
            case "eof":
            case "ioerr":
                socketCommand(c);
                return;
            case "lost":
                if (Integer.parseInt(c[1]) > 0) {
                    processLogout();
                    out.println("lost loggedout");
                } else {
                    Protocol.previousSocket = Protocol.socket;
                    Protocol.socket = null;
                    setGameState(40);
                    out.println("lost reconnecting");
                }
                break;
            case "tick":
                pendingTicks = Integer.parseInt(c[1]);
                return;
            default:
                throw new IllegalStateException("unknown command " + c[0]);
        }
        printState();
    }

    public static void main(String[] args) throws Exception {
        System.setProperty("line.separator", "\n");
        out = new PrintStream(new java.io.FileOutputStream(args[1]), false, "ISO-8859-1");
        for (String line : Files.readAllLines(Paths.get(args[0]), StandardCharsets.ISO_8859_1)) {
            line = line.trim();
            if (line.isEmpty() || line.startsWith("#")) continue;
            raw.add(line);
            commands.add(line.split(" +"));
        }
        while (pc < commands.size() || pendingTicks > 0) {
            if (pendingTicks > 0) {
                pendingTicks--;
                cycle();
                afterTick();
                continue;
            }
            out.println("> " + raw.get(pc));
            exec(commands.get(pc++));
        }
        out.flush();
        out.close();
    }
}
