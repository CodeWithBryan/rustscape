package core.net.packet;
import core.net.packet.context.ChildPositionContext;
/** Adapter: records the one packet the stringing listener sends. */
public final class PacketRepository {
    public static void send(Class<?> type, Object context) {
        ChildPositionContext c = (ChildPositionContext) context;
        oracle.W.emit("packet", type.getSimpleName(), c.interfaceId, c.childId, c.x, c.y);
    }
}
