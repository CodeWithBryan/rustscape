import java.io.BufferedReader;
import java.io.FileInputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;

import rt4.Camera;
import rt4.Player;
import rt4.PlayerList;
import rt4.Preferences;
import rt4.RoofLoc;
import rt4.Scenery;
import rt4.SceneGraph;
import rt4.ScriptRunner;
import rt4.Tile;

/**
 * Drives the original roof fill over scenes prepared by `cargo xtask
 * roof-oracle` and prints what it leaves behind.
 *
 * Per scene it receives the render flags and tile heights, a table of location
 * facts and the raw location files of the scene's regions. The original
 * `readLocs` decodes those files into the tile array (through the adapter
 * `addLoc` described in SceneGraph.java.in), and `bridgeTile` then lowers the
 * bridge columns as the end of `buildScene` does.
 *
 * Per step it sets the preferences, plane, render cycle, player and camera,
 * optionally calls `updateRoofRemovalMode`, calls
 * `updateRoofVisibilityGroups`, and reports the raw marks, the group bounds
 * and the tiles `SceneGraph.setShapedTile` would hide. That last test is
 * transcribed here from setShapedTile and from the `roofCycle` line of
 * ScriptRunner's scene draw; those two methods cannot run without a renderer.
 */
public final class RoofOracle {
    public static void main(String[] args) throws Exception {
        BufferedReader in = new BufferedReader(new InputStreamReader(new FileInputStream(args[0]), "US-ASCII"), 1 << 20);
        PrintStream out = new PrintStream(System.out, false, "US-ASCII");
        String line;
        java.util.ArrayList<String> lands = new java.util.ArrayList<String>();
        while ((line = in.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            int space = line.indexOf(' ');
            String op = space < 0 ? line : line.substring(0, space);
            String rest = space < 0 ? "" : line.substring(space + 1);
            if (op.equals("scene")) {
                out.print("scene " + rest + "\n");
                RoofLoc.clear();
                lands.clear();
                SceneGraph.width = 104;
                SceneGraph.length = 104;
                SceneGraph.tiles = new Tile[4][104][104];
                SceneGraph.tileHeights = new int[4][105][105];
                SceneGraph.underwaterTileHeights = null;
                SceneGraph.tileWaterData = null;
                SceneGraph.scenery = new Scenery[5000];
                SceneGraph.sceneryLen = 0;
                ScriptRunner.roofVisibility = null;
                ScriptRunner.renderCycle = 0;
            } else if (op.equals("flags")) {
                int at = 0;
                for (int plane = 0; plane < 4; plane++) {
                    for (int x = 0; x < 104; x++) {
                        for (int y = 0; y < 104; y++) {
                            SceneGraph.renderFlags[plane][x][y] = (byte) Integer.parseInt(rest.substring(at, at + 2), 16);
                            at += 2;
                        }
                    }
                }
            } else if (op.equals("heights")) {
                String[] t = rest.split(" ");
                int at = 0;
                for (int plane = 0; plane < 4; plane++) {
                    for (int x = 0; x <= 104; x++) {
                        for (int y = 0; y <= 104; y++) {
                            SceneGraph.tileHeights[plane][x][y] = Integer.parseInt(t[at++]);
                        }
                    }
                }
            } else if (op.equals("loc")) {
                String[] t = rest.split(" ");
                RoofLoc loc = new RoofLoc();
                loc.width = Integer.parseInt(t[1]);
                loc.length = Integer.parseInt(t[2]);
                loc.render = t[3].equals("1");
                loc.animated = t[4].equals("1");
                loc.models = Integer.parseInt(t[5]);
                int count = Integer.parseInt(t[6]);
                if (count >= 0) {
                    loc.shapes = new int[count];
                    for (int i = 0; i < count; i++) {
                        loc.shapes[i] = Integer.parseInt(t[7 + i]);
                    }
                }
                RoofLoc.put(Integer.parseInt(t[0]), loc);
            } else if (op.equals("land")) {
                lands.add(rest);
            } else if (op.equals("build")) {
                for (String land : lands) {
                    String[] t = land.split(" ");
                    byte[] data = new byte[t[2].length() / 2];
                    for (int i = 0; i < data.length; i++) {
                        data[i] = (byte) Integer.parseInt(t[2].substring(i * 2, i * 2 + 2), 16);
                    }
                    // LoginManager.readStaticLocs: (baseX, underwater, data, baseY, collision maps).
                    SceneGraph.readLocs(Integer.parseInt(t[0]), false, data, Integer.parseInt(t[1]), new rt4.CollisionMap[4]);
                }
                // The end of SceneGraph.buildScene.
                for (int x = 0; x < 104; x++) {
                    for (int y = 0; y < 104; y++) {
                        if ((SceneGraph.renderFlags[1][x][y] & 0x2) == 2) {
                            SceneGraph.bridgeTile(x, y);
                        }
                    }
                }
                dump(out);
            } else if (op.equals("step")) {
                String[] t = rest.split(" ");
                boolean update = t[0].equals("1");
                ScriptRunner.neverRemoveRoofs = t[1].equals("1");
                Preferences.removeRoofsSelectively = t[2].equals("1");
                Player.plane = Integer.parseInt(t[3]);
                PlayerList.self.xFine = Integer.parseInt(t[4]);
                PlayerList.self.yFine = Integer.parseInt(t[5]);
                Camera.cameraType = t[6].equals("1") ? 1 : 2;
                Camera.renderX = Integer.parseInt(t[7]);
                Camera.renderY = Integer.parseInt(t[8]);
                Camera.renderZ = Integer.parseInt(t[9]);
                Camera.cameraPitch = Integer.parseInt(t[10]);
                ScriptRunner.renderCycle = Integer.parseInt(t[11]);
                if (update) {
                    ScriptRunner.updateRoofRemovalMode();
                }
                ScriptRunner.updateRoofVisibilityGroups();
                step(out);
            } else {
                throw new IllegalArgumentException(op);
            }
        }
        out.flush();
    }

    private static void dump(PrintStream out) {
        int count = 0;
        StringBuilder text = new StringBuilder();
        for (int level = 0; level < 4; level++) {
            for (int x = 0; x < 104; x++) {
                for (int y = 0; y < 104; y++) {
                    Tile tile = SceneGraph.tiles[level][x][y];
                    if (tile == null) {
                        continue;
                    }
                    if (tile.wall != null) {
                        text.append("w ").append(level).append(' ').append(x).append(' ').append(y).append(' ').append(tile.wall.primaryFlags).append(' ').append(tile.wall.secondaryFlags).append('\n');
                        count++;
                    }
                    for (int i = 0; i < tile.sceneryLen; i++) {
                        Scenery s = tile.scenery[i];
                        text.append("s ").append(level).append(' ').append(x).append(' ').append(y).append(' ').append(i).append(' ').append((int) (s.key >> 14 & 0x3FL)).append(' ').append((int) (s.key >> 20 & 0x3L)).append(' ').append(s.xMin).append(' ').append(s.xMax).append(' ').append(s.yMin).append(' ').append(s.yMax).append('\n');
                        count++;
                    }
                }
            }
        }
        out.print(text);
        out.print("tiles " + count + "\n");
    }

    private static void step(PrintStream out) {
        StringBuilder text = new StringBuilder("step ");
        byte[][][] marks = ScriptRunner.roofVisibility;
        if (marks == null) {
            text.append("null");
        } else {
            long hash = 0xcbf29ce484222325L;
            for (int level = 0; level < 4; level++) {
                for (int x = 0; x < 104; x++) {
                    for (int y = 0; y < 104; y++) {
                        hash ^= marks[level][x][y] & 0xFF;
                        hash *= 0x100000001b3L;
                    }
                }
            }
            text.append(Long.toHexString(hash));
        }
        int[][] groups = {ScriptRunner.roofGroupMaxHeight, ScriptRunner.roofGroupMinX, ScriptRunner.roofGroupMaxX, ScriptRunner.roofGroupMinZ, ScriptRunner.roofGroupMaxZ};
        text.append(" g ").append(groups[0].length);
        for (int[] group : groups) {
            for (int value : group) {
                text.append(' ').append(value);
            }
        }
        // ScriptRunner's scene draw: roofCycle, and the level Player.plane + 1.
        byte roofCycle = ScriptRunner.getRoofRemovalMode() == 2 ? (byte) ScriptRunner.renderCycle : 1;
        int occlusionLevel = Player.plane + 1;
        StringBuilder hidden = new StringBuilder();
        int count = 0;
        for (int lvl = 0; lvl < 4; lvl++) {
            for (int x = 0; x < 104; x++) {
                for (int y = 0; y < 104; y++) {
                    // SceneGraph.setShapedTile: visible when this holds.
                    if (!(marks == null || lvl < occlusionLevel || marks[lvl][x][y] != roofCycle)) {
                        hidden.append(' ').append(lvl * 10816 + x * 104 + y);
                        count++;
                    }
                }
            }
        }
        text.append(" h ").append(count).append(hidden).append('\n');
        out.print(text);
    }
}
