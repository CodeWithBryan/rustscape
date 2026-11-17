package oracle.worldtrace;

import core.game.system.command.Command;
import core.game.system.command.CommandMapping;
import core.game.system.command.CommandPlugin;
import core.game.system.command.CommandSet;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/**
 * Boots the original server exactly as WorldTrace.main does (the same statements up to and including the boot
 * record), then writes what the boot left in the command registries, read without changing them:
 * CommandMapping in the order getNames() walks its HashMap (with each Command's name, privilege, usage, description
 * and the class of its handler lambda, which names the class and source order that defined it), the plugins each
 * legacy CommandSet constant holds, and CommandMapping.getPageIndices for rights 0 to 3.
 *
 * Usage (working directory = the world directory): CommandRegistry SCENARIO_JSON OUT
 */
public final class CommandRegistry {
    public static void main(String[] args) throws Throwable {
        WorldTrace.world = Paths.get("").toAbsolutePath();
        JSONObject scenario = (JSONObject) new JSONParser().parse(new String(Files.readAllBytes(Paths.get(args[0])), StandardCharsets.UTF_8));
        System.setErr(new PrintStream(new WorldTrace.CountingErr(System.err), true));
        System.setSecurityManager(new Guard(WorldTrace.world.getParent().toFile()));
        Pin.installGenerators((Long) scenario.get("clockStart"), (Long) scenario.get("threadLocalSeed"));
        Handoffs.install();
        WorldTrace.files = WorldTrace.snapshot();

        Map<String, Object> header = new LinkedHashMap<>();
        header.put("t", "scenario");
        header.put("name", scenario.get("name"));
        header.put("clockStart", scenario.get("clockStart"));
        header.put("threadLocalSeed", scenario.get("threadLocalSeed"));
        header.put("tickMillis", Pin.TICK_MILLIS);
        WorldTrace.lines.add(Json.write(header));
        WorldTrace.record("boot", WorldTrace.boot());

        List<Object> registry = new ArrayList<>();
        for (String key : CommandMapping.INSTANCE.getNames()) {
            Command command = CommandMapping.INSTANCE.get(key);
            List<Object> row = new ArrayList<>();
            row.add(key);
            row.add(command.getName());
            row.add(command.getPrivilege().name());
            row.add(command.getUsage());
            row.add(command.getDescription());
            row.add(command.getHandle().getClass().getName());
            registry.add(row);
        }
        Map<String, Object> commands = new LinkedHashMap<>();
        commands.put("t", "commands");
        commands.put("registry", registry);
        Map<String, Object> legacy = new LinkedHashMap<>();
        for (CommandSet set : CommandSet.values()) {
            List<Object> plugins = new ArrayList<>();
            for (CommandPlugin plugin : set.getPlugins()) plugins.add(plugin.getClass().getName());
            legacy.put(set.name(), plugins);
        }
        commands.put("legacySets", legacy);
        List<Object> pages = new ArrayList<>();
        for (int rights = 0; rights <= 3; rights++) {
            List<Object> row = new ArrayList<>();
            for (int index : CommandMapping.INSTANCE.getPageIndices(rights)) row.add(index);
            pages.add(row);
        }
        commands.put("pageIndices", pages);
        WorldTrace.lines.add(Json.write(commands));
        Files.write(Paths.get(args[1]), (String.join("\n", WorldTrace.lines) + "\n").getBytes(StandardCharsets.UTF_8));
        System.out.flush();
        System.err.flush();
        Runtime.getRuntime().halt(0);
    }
}
