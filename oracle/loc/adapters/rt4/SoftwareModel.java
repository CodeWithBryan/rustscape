package rt4;

/** Oracle adapter: the software renderer's model. LocType and RawModel name it in their non-GL branches, which the oracle never takes. */
public final class SoftwareModel extends Model {
	public SoftwareModel(RawModel model, int ambient, int contrast, int x, int y, int z) {
		throw new UnsupportedOperationException();
	}
	public SoftwareModel alignToTerrain(int type, int amount, int[][] heights, int[][] above, int x, int base, int z, boolean copy) {
		throw new UnsupportedOperationException();
	}
	public int getMaxZ() { throw new UnsupportedOperationException(); }
	public int getMinY() { throw new UnsupportedOperationException(); }
	protected boolean hasAnimationBones() { throw new UnsupportedOperationException(); }
	public void rotate180() { throw new UnsupportedOperationException(); }
	public void rotateY(int angle) { throw new UnsupportedOperationException(); }
	protected void resetAfterAnimation() { throw new UnsupportedOperationException(); }
	public void resize(int x, int y, int z) { throw new UnsupportedOperationException(); }
	public Model copyForAnimation(boolean a, boolean b, boolean c) { throw new UnsupportedOperationException(); }
	public int getMaxX() { throw new UnsupportedOperationException(); }
	public int getMinX() { throw new UnsupportedOperationException(); }
	public void rotateCounterClockwise() { throw new UnsupportedOperationException(); }
	public void rotateZ(int angle) { throw new UnsupportedOperationException(); }
	public int getLengthXZ() { throw new UnsupportedOperationException(); }
	protected void transformShadowBone(int type, int x, int y, int z) { throw new UnsupportedOperationException(); }
	public Model copyForLoc(boolean a, boolean b, boolean c) { throw new UnsupportedOperationException(); }
	protected void transformBone(int type, int[] bones, int x, int y, int z, boolean vertices) { throw new UnsupportedOperationException(); }
	public void setCamera(int yaw, int yOffset, int pitch, int xOffset, int near, int far, long key) { throw new UnsupportedOperationException(); }
	public Model copyForEntity(boolean a, boolean b, boolean c) { throw new UnsupportedOperationException(); }
	public void rotateX(int angle) { throw new UnsupportedOperationException(); }
	public void translate(int x, int y, int z) { throw new UnsupportedOperationException(); }
	public int getMinZ() { throw new UnsupportedOperationException(); }
	public void render(int yaw, int sinPitch, int cosPitch, int sinYaw, int cosYaw, int x, int z, int y, long key, int plane, ParticleSystem particles) { throw new UnsupportedOperationException(); }
	protected void transformMaskedBone(int type, int[] bones, int x, int y, int z, boolean vertices, int bits, int[] labels) { throw new UnsupportedOperationException(); }
	public void rotateClockwise() { throw new UnsupportedOperationException(); }
}