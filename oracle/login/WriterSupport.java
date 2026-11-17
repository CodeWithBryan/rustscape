package rt4;
import java.util.Arrays;
/** Test-only holders for state read by the verbatim LoginManager writing slice. */
public final class WriterSupport {
    public static Buffer buffer;
    public static int loginType;
    public static int[] checksums;
    public static int windowMode, preferences;
    public static byte[] uid;
    public static byte[] write(byte[] username, byte[] password, int[] key, int[] fields,
        byte[] uidBytes, byte[] settings, int[] crcs, boolean extra, boolean fake28) {
        Player.usernameInput=new JagString(username); Player.password=new JagString(password);
        Protocol.outboundBuffer=new Buffer(65536); buffer=new Buffer(65536);
        loginType=fields[0]; client.advertSuppressed=fields[1]!=0;
        windowMode=fields[2]; GameShell.canvasWidth=fields[3]; GameShell.canvasHeight=fields[4];
        Preferences.antiAliasingMode=fields[5]; client.affiliate=fields[6]; preferences=fields[7];
        Protocol.verifyId=fields[8]; client.gameState=fields[9];
        uid=uidBytes; client.settings=new JagString(settings);checksums=crcs;
        GlobalConfig.LOGIN_EXTRA_INFO=extra;GlobalConfig.LOGIN_FAKE_IDX28=fake28;
        /* ORIGINAL_LOGIN_WRITER */
        return Arrays.copyOf(buffer.data,buffer.offset);
    }
    public static long name(byte[] bytes){return new JagString(bytes).encode37();}
    public static byte[] rsa(byte[] input, String exp, String mod) {
        Buffer b=new Buffer(4096);b.pdata(input,input.length);
        b.rsaenc(new java.math.BigInteger(exp),new java.math.BigInteger(mod));
        return Arrays.copyOf(b.data,b.offset);
    }
}
class Protocol {static Buffer outboundBuffer;static int verifyId;}
class Player {static JagString usernameInput,password;}
class GameShell {static int canvasWidth,canvasHeight;}
class DisplayMode {static int getWindowMode(){return WriterSupport.windowMode;}}
class Preferences {static int antiAliasingMode;static boolean sentToServer;static int toInt(){return WriterSupport.preferences;}}
class Checksum {final int id;Checksum(int id){this.id=id;}int getChecksum(){return WriterSupport.checksums[id];}}
class client {
    static boolean advertSuppressed;static int gameState,affiliate;static JagString settings;
    static void writeUid(Buffer b){b.pdata(WriterSupport.uid,24);}
    /* CHECKSUM_FIELDS */
}
