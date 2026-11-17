package oracle;

import core.game.node.entity.player.Player;
import core.game.node.scenery.Scenery;
import core.game.world.update.flag.context.Animation;
import core.net.packet.IoBuffer;
import core.net.packet.context.AnimateObjectContext;
import core.net.packet.context.AreaPositionContext;
import core.net.packet.context.CSConfigContext;
import core.net.packet.context.CameraContext;
import core.net.packet.context.SystemUpdateContext;
import core.net.packet.out.AnimateObjectPacket;
import core.net.packet.out.CSConfigPacket;
import core.net.packet.out.CameraViewPacket;
import core.net.packet.out.SystemUpdatePacket;
import core.net.packet.out.UpdateAreaPosition;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

/**
 * The outgoing packet writers of this slice, unchanged, with their contexts: the session adapter records what each
 * writes (opcode, header and payload bytes); the ISAAC stream is the transport's and adds nothing here.
 */
public final class Packets {
    static int n(Object o) { return ((Number) o).intValue(); }

    /** What a session was given: [opcode, header, payload hex]. */
    public static Object written(Object message) {
        IoBuffer buffer = (IoBuffer) message;
        java.nio.ByteBuffer bytes = buffer.toByteBuffer();
        StringBuilder hex = new StringBuilder();
        for (int i = 0; i < bytes.position(); i++) hex.append(String.format("%02x", bytes.get(i) & 0xFF));
        return Arrays.asList(buffer.opcode(), buffer.getHeader().name(), hex.toString());
    }

    static Object write(Map op, Player player) {
        String kind = (String) op.get("kind");
        switch (kind) {
            case "systemUpdate":
                new SystemUpdatePacket().send(new SystemUpdateContext(player, n(op.get("time"))));
                return null;
            case "camera":
                new CameraViewPacket().send(new CameraContext(player, CameraContext.CameraType.valueOf((String) op.get("type")), n(op.get("x")),
                    n(op.get("y")), n(op.get("height")), n(op.get("speed")), n(op.get("zoomSpeed"))));
                return null;
            case "csConfig":
                new CSConfigPacket().send(new CSConfigContext(player, n(op.get("id")), n(op.get("value")), null, null));
                return null;
            case "areaPosition":
                new UpdateAreaPosition().send(new AreaPositionContext(player, oracle.Support.location(op.get("location")), 0, 0));
                return null;
            case "chunkBuffer": {
                IoBuffer buffer = UpdateAreaPosition.getChunkUpdateBuffer(player, oracle.Support.location(op.get("location")));
                return written(buffer);
            }
            case "animateObject": {
                List row = (List) op.get("object");
                Scenery object = Scenery.placed(row);
                new AnimateObjectPacket().send(new AnimateObjectContext(player, new Animation(n(op.get("animation")), object)));
                return null;
            }
            default:
                throw new IllegalArgumentException(kind);
        }
    }
}
