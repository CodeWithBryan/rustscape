package rt4;

/** The fields of rt4.Light that LightingManager, LightMesh and buildGlLighting read. */
public final class Light {
	public boolean extendsDown, extendsUp, onBridge;
	public int x, y, z, level, radius, color;
	public short[] spanProfile;
	public float alpha, quadraticAttenuation;
	public final float[] diffuse = new float[4];
	public LightMesh mesh;

	public void updateAnimation(boolean disableFlicker, int time) {
	}
}