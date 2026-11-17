package rt4;

/** Oracle adapter: the fields AudioChannel touches. The oracle calls the block reader directly and starts no thread. */
public class AudioThread implements Runnable {
	public final AudioChannel[] channels = new AudioChannel[2];
	public SignLink signLink;
	public volatile boolean stop;
	public volatile boolean running;

	public void run() {
	}
}
