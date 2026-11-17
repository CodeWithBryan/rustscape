package core.game.system;
import core.game.system.security.EncryptionManager;
/** Deterministic external entropy/cost adapter for auth scenarios; full BCrypt remains original. */
public final class SystemManager {
    public static String salt="$2a$04$......................";
    public static boolean failHash=false;
    public static long elapsedMillis=0,hashDelayMillis=0,verifyDelayMillis=0;
    private static final EncryptionManager ENCRYPTION=new EncryptionManager(){
        @Override public String hashPassword(String password){
            try {
                if(failHash)throw new IllegalStateException("test hashing failure");
                return super.hashPassword(password,salt);
            } finally {elapsedMillis+=hashDelayMillis;}
        }
        @Override public boolean checkPassword(String password,String stored){
            try {return super.checkPassword(password,stored);}
            finally {elapsedMillis+=verifyDelayMillis;}
        }
    };
    public static EncryptionManager getEncryption(){return ENCRYPTION;}
}
