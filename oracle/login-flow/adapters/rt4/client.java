package rt4;

public class client {
    public static String hostname;
    public static String worldListHostname;
    public static int port, defaultPort, alternatePort;
    public static int worldListPort, worldListDefaultPort, worldListAlternatePort;
    public static int worldListId = 1;
    public static int modeWhere = 0;
    public static int gameState = 0;
    public static int affiliate = 0;
    public static int language = 0;
    public static AudioChannel musicChannel = new AudioChannel();
    public static AudioChannel soundChannel = null;
    public static JagString settings = JagString.EMPTY;
    public static boolean advertSuppressed = false;
    public static boolean objectTag = false;
    public static boolean javaScript = false;
    public static client instance = new client();
    public static byte[] uid = new byte[24];
    public static Js5 js5Archive0 = new Js5(), js5Archive1 = new Js5(), js5Archive2 = new Js5(), js5Archive3 = new Js5(),
        js5Archive4 = new Js5(), js5Archive5 = new Js5(), js5Archive6 = new Js5(), js5Archive7 = new Js5(),
        js5Archive8 = new Js5(), js5Archive9 = new Js5(), js5Archive10 = new Js5(), js5Archive11 = new Js5(),
        js5Archive12 = new Js5(), js5Archive13 = new Js5(), js5Archive14 = new Js5(), js5Archive15 = new Js5(),
        js5Archive16 = new Js5(), js5Archive17 = new Js5(), js5Archive18 = new Js5(), js5Archive19 = new Js5(),
        js5Archive20 = new Js5(), js5Archive21 = new Js5(), js5Archive22 = new Js5(), js5Archive23 = new Js5(),
        js5Archive24 = new Js5(), js5Archive25 = new Js5(), js5Archive26 = new Js5(), js5Archive27 = new Js5();

    public static Js5[] archives() {
        return new Js5[]{js5Archive0, js5Archive1, js5Archive2, js5Archive3, js5Archive4, js5Archive5, js5Archive6,
            js5Archive7, js5Archive8, js5Archive9, js5Archive10, js5Archive11, js5Archive12, js5Archive13,
            js5Archive14, js5Archive15, js5Archive16, js5Archive17, js5Archive18, js5Archive19, js5Archive20,
            js5Archive21, js5Archive22, js5Archive23, js5Archive24, js5Archive25, js5Archive26, js5Archive27};
    }
    /** The bytes client.writeUid would put; its file handling is outside this oracle. */
    public static void writeUid(Buffer buffer) { buffer.pdata(uid, 24); }
    public static void resetGameSessionState() { LoginOracle.sessionReset(); }
    public java.applet.AppletContext getAppletContext() { throw new IllegalStateException("no applet"); }
}