package rt4;

public class Protocol {
    public static BufferedSocket socket;
    public static BufferedSocket previousSocket;
    public static PrivilegedRequest socketRequest;
    public static final Packet outboundBuffer = new Packet(5000);
    public static final Packet inboundBuffer = new Packet(65536);
    public static int verifyId = 0;
    public static int opcode = -1;
    public static int length = 0;
    public static void readRebuildPacket(boolean dynamic) { LoginOracle.rebuild(dynamic); }
}