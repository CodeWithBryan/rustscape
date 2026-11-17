package rt4;

/** Oracle adapter: the NpcType fields the movement and scene-insertion code reads. */
public class NpcType {
	public boolean rotationflag = true;
	public boolean toprenderpriority;
	public boolean interactive = true;
	public boolean multiNpcValid = true;
	public int size = 1;

	public final boolean isMultiNpcValid() {
		return this.multiNpcValid;
	}
}