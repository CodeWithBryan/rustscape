import java.nio.ByteBuffer;
import core.auth.AuthResponse;
/** Original write methods, with explicit holders for world/player/session state. */
public final class LoginReplies {
    private static final EventProducer GAME_PRODUCER=new EventProducer();
    /* ORIGINAL_WRITE */
    /* ORIGINAL_WORLD_REPLY */
    public static byte[] encode(int response,int rights,int index,long key){
        IoSession session=new IoSession(rights,index,key);
        new LoginReplies().write(session,AuthResponse.values()[response]);return session.bytes;
    }
    private static class EventProducer {}
    private static class IoSession {
        final Player player;final long key;byte[] bytes;
        IoSession(int rights,int index,long key){player=new Player(rights,index);this.key=key;}
        Player getPlayer(){return player;}long getServerKey(){return key;}
        void setProducer(EventProducer value){}
        void queue(ByteBuffer buffer){bytes=new byte[buffer.remaining()];buffer.get(bytes);}
    }
    private static class Player {
        final int index;final Details details;
        Player(int rights,int index){this.index=index;details=new Details(rights);}
        Details getDetails(){return details;}int getIndex(){return index;}
    }
    private static class Details {final Rights rights;Details(int value){rights=new Rights(value);}Rights getRights(){return rights;}}
    private static class Rights {final int value;Rights(int value){this.value=value;}int ordinal(){return value;}}
}
