package rt4;

/** Oracle adapter: the original starts the audio thread through SignLink, which the oracle never does. */
public class SignLink {
	public void startThread(int priority, Runnable runnable) {
		throw new UnsupportedOperationException();
	}
}
