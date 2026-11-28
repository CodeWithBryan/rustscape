package core.net.packet;
import core.net.packet.context.ChildPositionContext;
import oracle.W;
/** Adapter: records the one packet the plugins send. */
public final class PacketRepository {
    public static void send(Class<?> packet, Context context) {
        ChildPositionContext c = (ChildPositionContext) context;
        W.emit("send", packet.getSimpleName(), c.getInterfaceId(), c.getChildId(), c.getPosition().x, c.getPosition().y);
    }
}
