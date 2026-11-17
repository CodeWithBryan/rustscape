package rt4;

/** Oracle adapter: the two statics CursorType.getSprite reads. No archive is opened. */
public final class CursorTypeList {
	public static final SoftLruHashTable sprites = new SoftLruHashTable();
	public static Js5 spritesArchive;
}