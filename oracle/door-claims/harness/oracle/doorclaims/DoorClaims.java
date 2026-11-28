package oracle.doorclaims;

import core.cache.def.impl.SceneryDefinition;
import core.game.interaction.IntType;
import core.game.interaction.InteractionListeners;
import core.game.interaction.PluginInteractionManager;
import core.game.system.config.ServerConfigParser;
import core.game.world.GameWorld;

import java.io.File;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Map;
import java.util.TreeSet;

/**
 * Boots the unmodified original as Server.main does (without its networking, console reader and worker thread) and
 * lists what its start-up registered ahead of DoorManagingPlugin for the four option names that plugin defaults.
 */
public final class DoorClaims {
    static final String[] OPTIONS = {"open", "close", "shut", "go-through"};
    static final String DOORS = "content.global.handlers.scenery.DoorManagingPlugin";

    static Object field(Class<?> owner, Object target, String name) throws Exception {
        Field f = owner.getDeclaredField(name);
        f.setAccessible(true);
        return f.get(target);
    }

    public static void main(String[] args) throws Throwable {
        ServerConfigParser.INSTANCE.parse("worldprops" + File.separator + "default.conf");
        GameWorld.prompt(false);
        TreeSet<String> rows = new TreeSet<>();
        int scenery = IntType.SCENERY.ordinal();
        // InteractionListeners.get(id, type, option): "$id:$type:$option", then the catch-all "$type:$option".
        Map<String, ?> listeners = (Map<String, ?>) field(InteractionListeners.class, InteractionListeners.INSTANCE, "listeners");
        for (Map.Entry<String, ?> e : listeners.entrySet()) {
            String[] key = e.getKey().split(":", -1);
            for (String option : OPTIONS) {
                if (key.length == 3 && key[1].equals(String.valueOf(scenery)) && key[2].equals(option)) {
                    rows.add(pad(Integer.parseInt(key[0])) + " " + option + " listener " + e.getValue().getClass().getName());
                }
                if (key.length == 2 && key[0].equals(String.valueOf(scenery)) && key[1].equals(option)) {
                    rows.add(pad(-1) + " " + option + " listener " + e.getValue().getClass().getName());
                }
            }
        }
        // defineInteraction's metadata: "${type.ordinal}:$id:$option", then "${type.ordinal}:$option".
        Map<String, ?> interactions = (Map<String, ?>) field(InteractionListeners.class, InteractionListeners.INSTANCE, "interactions");
        for (Map.Entry<String, ?> e : interactions.entrySet()) {
            String[] key = e.getKey().split(":", -1);
            for (String option : OPTIONS) {
                Object handler = field(e.getValue().getClass(), e.getValue(), "handler");
                if (key.length == 3 && key[0].equals(String.valueOf(scenery)) && key[2].equals(option)) {
                    rows.add(pad(Integer.parseInt(key[1])) + " " + option + " interaction " + handler.getClass().getName());
                }
                if (key.length == 2 && key[0].equals(String.valueOf(scenery)) && key[1].equals(option)) {
                    rows.add(pad(-1) + " " + option + " interaction " + handler.getClass().getName());
                }
            }
        }
        // PluginInteractionManager.handle(player, object): by scenery id, whatever the option.
        Map<Integer, ?> plugins = (Map<Integer, ?>) field(PluginInteractionManager.class, null, "objectInteractions");
        for (Map.Entry<Integer, ?> e : plugins.entrySet()) {
            for (String option : OPTIONS) {
                rows.add(pad(e.getKey()) + " " + option + " plugin " + e.getValue().getClass().getName());
            }
        }
        // A handler on the scenery's own definition comes before the default handler of the option name.
        for (Map.Entry<Integer, SceneryDefinition> e : SceneryDefinition.getDefinitions().entrySet()) {
            for (String option : OPTIONS) {
                Object handler = e.getValue().getHandlers().get("option:" + option);
                if (handler != null && !handler.getClass().getName().equals(DOORS)) {
                    rows.add(pad(e.getKey()) + " " + option + " definition " + handler.getClass().getName());
                }
            }
        }
        StringBuilder out = new StringBuilder();
        for (String option : OPTIONS) {
            Object handler = field(SceneryDefinition.class, null, "OPTION_HANDLERS") == null ? null
                : ((Map<String, ?>) field(SceneryDefinition.class, null, "OPTION_HANDLERS")).get(option);
            out.append("# default ").append(option).append(' ').append(handler == null ? "none" : handler.getClass().getName()).append('\n');
        }
        for (String row : rows) {
            // The sort key is the zero-padded id; the row carries the plain one.
            String[] parts = row.split(" ", 2);
            out.append(Integer.parseInt(parts[0].trim()) ).append(' ').append(parts[1]).append('\n');
        }
        Files.write(Paths.get(args[0]), out.toString().getBytes(StandardCharsets.UTF_8));
        System.exit(0);
    }

    static String pad(int id) {
        return id < 0 ? "-0000001" : String.format("%08d", id);
    }
}
