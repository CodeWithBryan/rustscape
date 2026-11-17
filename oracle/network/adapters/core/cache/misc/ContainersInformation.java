package core.cache.misc;
/** Explicit metadata inputs; not a replacement for the server's Kotlin parser. */
public final class ContainersInformation {
    public final Container informationContainer;
    private final int revision;
    public ContainersInformation(int crc, int revision) {
        this.informationContainer = new Container(crc); this.revision = revision;
    }
    public int getRevision() {return revision;}
}
