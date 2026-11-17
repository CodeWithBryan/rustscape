package rt4;

/** Oracle adapter: remembers the key CursorType.getSprite looks up, which is its private sprite id. */
public final class SoftLruHashTable {
	public long lastKey = -1L;

	public Object get(long key) {
		this.lastKey = key;
		return null;
	}

	public void put(Object value, long key) {
	}
}