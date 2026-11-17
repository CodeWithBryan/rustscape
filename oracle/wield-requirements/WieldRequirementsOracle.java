package oracle;

import core.cache.def.impl.ItemDefinition;
import core.game.node.entity.player.Player;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.JSONValue;

/**
 * Runs the unchanged ItemDefinition.hasRequirement over each case: the case's
 * configuration (absent, a value of another class, or a java.util.HashMap
 * filled in the case's order), the player's static levels and the two flags.
 * Records the result, the exception class and every message sent.
 */
public final class WieldRequirementsOracle {
    /** PacketDispatch.sendMessage calls of the current case. */
    public static final List<String> MESSAGES = new ArrayList<>();

    public static void main(String[] args) throws Exception {
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintWriter out = new PrintWriter(new OutputStreamWriter(System.out, StandardCharsets.UTF_8));
        String line;
        while ((line = in.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            JSONObject row = (JSONObject) JSONValue.parse(line);
            out.println(JSONValue.toJSONString(evaluate(row)));
        }
        out.flush();
    }

    @SuppressWarnings("unchecked")
    static JSONObject evaluate(JSONObject row) {
        MESSAGES.clear();
        ItemDefinition definition = new ItemDefinition();
        Object config = row.get("requirements");
        if (config instanceof JSONArray) {
            HashMap<Integer, Integer> map = new HashMap<>();
            for (Object entry : (JSONArray) config) {
                JSONArray pair = (JSONArray) entry;
                map.put(((Number) pair.get(0)).intValue(), ((Number) pair.get(1)).intValue());
            }
            definition.configuration = map;
        } else if (config instanceof String) {
            definition.configuration = config;
        } else {
            definition.configuration = null;
        }
        JSONArray levels = (JSONArray) row.get("levels");
        int[] statics = new int[24];
        for (int i = 0; i < 24; i++) {
            statics[i] = ((Number) levels.get(i)).intValue();
        }
        Player player = new Player(statics);
        JSONObject expected = new JSONObject();
        try {
            boolean result = definition.hasRequirement(player, (Boolean) row.get("wield"), (Boolean) row.get("message"));
            expected.put("result", result);
            expected.put("error", null);
        } catch (Exception e) {
            expected.put("result", null);
            expected.put("error", e.getClass().getSimpleName());
        }
        JSONArray messages = new JSONArray();
        messages.addAll(MESSAGES);
        expected.put("messages", messages);
        JSONObject output = new JSONObject();
        output.putAll(row);
        output.put("expected", expected);
        return output;
    }
}
