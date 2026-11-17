package core.cache;
import java.io.RandomAccessFile;
import java.util.function.IntFunction;
/** Test data provider; Cache.getArchiveData itself remains unmodified. */
public final class CacheFile {
    private final IntFunction<byte[]> fetch;
    public CacheFile(IntFunction<byte[]> fetch) {this.fetch = fetch;}
    public CacheFile(int id, RandomAccessFile index, RandomAccessFile data, int limit, byte[] buffer) {
        throw new UnsupportedOperationException("Full server cache initialization is outside this oracle");
    }
    public byte[] getContainerData(int group) {return fetch.apply(group);}
}
