package rt4;

public class SignLink {
    /** The pinned signlink constant. */
    public static final int clientMode = 1;
    public java.applet.Applet applet;
    public PrivilegedRequest openSocket(String host, int port) { return LoginOracle.open(host, port); }
}