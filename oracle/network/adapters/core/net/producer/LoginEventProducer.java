package core.net.producer;
import core.net.*;
import java.nio.ByteBuffer;
/** Marker for the handshake transition; authentication is not emulated. */
public final class LoginEventProducer implements EventProducer {
    public IoReadEvent produceReader(IoSession session, ByteBuffer buffer) {throw new UnsupportedOperationException();}
    public IoWriteEvent produceWriter(IoSession session, Object context) {throw new UnsupportedOperationException();}
}
