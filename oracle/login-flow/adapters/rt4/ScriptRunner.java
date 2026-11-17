package rt4;

public class ScriptRunner {
    /** Transcribed from the original ScriptRunner.getWorld (one expression). */
    public static World getWorld(int worldId) {
        return WorldList.loaded && worldId >= WorldList.minId && worldId <= WorldList.maxId ? WorldList.worlds[worldId - WorldList.minId] : null;
    }
}