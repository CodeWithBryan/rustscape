package rt4;

/** Oracle adapter: the light fields GlModel reads when it packs colours without high detail lighting (the oracle's sdcolour cases set light0Position) or projects a shadow, which the location oracle does not do. */
public final class FogManager {
	public static int lightX;
	public static int lightZ;
	public static final float[] light0Position = new float[4];
	public static float getLight0Diffuse() {
		throw new UnsupportedOperationException();
	}
	public static int getLightColor() {
		throw new UnsupportedOperationException();
	}
	public static float getLightingModelAmbient() {
		throw new UnsupportedOperationException();
	}
}
