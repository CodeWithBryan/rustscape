package core.cache;
import core.cache.misc.ContainersInformation;
public final class CacheFileManager {
    private final CacheFile file;
    private final ContainersInformation info;
    public CacheFileManager(CacheFile file, int crc, int revision) {
        this.file = file; this.info = new ContainersInformation(crc, revision);
    }
    public CacheFileManager(CacheFile file, boolean discard) {throw new UnsupportedOperationException();}
    public CacheFile getCacheFile() {return file;}
    public ContainersInformation getInformation() {return info;}
    public int getContainersSize() {throw new UnsupportedOperationException();}
    public int getFilesSize(int group) {throw new UnsupportedOperationException();}
}
