package rt4;

/** Oracle adapter: LocType.getAnimatedEntity is called without a sequence, so neither method runs. */
public final class SeqType {
	public Model animateLocSoftware(int frame, int delay, SoftwareModel model, int rotation, int tween) {
		throw new UnsupportedOperationException();
	}
	public Model animateLocGl(int delay, int frame, int tween, int rotation, GlModel model) {
		throw new UnsupportedOperationException();
	}
}