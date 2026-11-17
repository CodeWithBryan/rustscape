package content.global.skill.farming;
import oracle.Support;
public class Patch {
    public int getCurrentState() { return Support.emit("patch.getCurrentState", Support.patchState); }
    public void setCurrentState(int v) { Support.emit("patch.setCurrentState", null, v); Support.patchState = v; }
}
