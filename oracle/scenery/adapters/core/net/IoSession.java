package core.net;
import java.nio.ByteBuffer;import core.net.packet.IoBuffer;import core.net.event.GameWriteEvent;
public class IoSession {
 public byte[] bytes; public void disconnect(){throw new UnsupportedOperationException("outside scope");}
 public void queue(ByteBuffer b){bytes=new byte[b.remaining()];b.get(bytes);}
 public void write(IoBuffer b){new GameWriteEvent(this,b).write(this,b);}
 public IoSession getIsaacPair(){return this;} public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}
}
