package core.net;
import java.nio.ByteBuffer;
import java.util.*;
import core.net.producer.HSEventProducer;
/** Observable protocol state/queues. No live game, authentication, or socket mock success. */
public final class IoSession {
    public final List<byte[]> responses = new ArrayList<byte[]>();
    public final List<int[]> requests = new ArrayList<int[]>();
    private final JS5Queue js5 = new JS5Queue(this);
    private EventProducer producer = new HSEventProducer();
    private ByteBuffer reading;
    private long serverKey;
    private int nameHash, encryption;
    public boolean disconnected;
    public String getAddress() {return "127.0.0.1";}
    public void setNameHash(int value) {nameHash = value;}
    public int getNameHash() {return nameHash;}
    public void setServerKey(long value) {serverKey = value;}
    public long getServerKey() {return serverKey;}
    public void setJs5Encryption(int value) {encryption = value;}
    public int getJs5Encryption() {return encryption;}
    public JS5Queue getJs5Queue() {return js5;}
    public void disconnect() {disconnected = true;}
    public boolean isActive() {return !disconnected;}
    public void setProducer(EventProducer value) {producer = value;}
    public EventProducer getProducer() {return producer;}
    public void write(Object context) {producer.produceWriter(this, context).run();}
    public void queue(ByteBuffer buffer) {
        byte[] bytes = new byte[buffer.remaining()]; buffer.get(bytes); responses.add(bytes);
    }
    public ByteBuffer getReadingQueue() {return reading;}
    public void setReadingQueue(ByteBuffer buffer) {reading = buffer;}
}
