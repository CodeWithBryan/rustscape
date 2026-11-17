package oracle;

import content.data.skill.SkillingTool;
import content.global.skill.gather.SkillingResource;
import content.global.skill.gather.mining.MiningNode;
import content.global.skill.gather.woodcutting.WoodcuttingNode;
import core.game.interaction.Registry;
import core.game.node.entity.player.Player;
import core.game.node.item.WeightedChanceItem;
import core.game.node.scenery.Scenery;
import core.game.system.task.Pulse;
import core.tools.RandomFunction;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified gathering sources over the recording world, one JSON case per line. */
public final class Oracle {
    private static List<WeightedChanceItem> declaredGems;
    private static Field gemId;
    private static Scenery node;
    /** The interacted node: the scenery above, or a fishing spot NPC. */
    private static core.game.node.Node target;
    private static Pulse pulse;
    private static int scriptState;
    private static JSONObject previousState;

    public static void main(String[] args) throws Exception {
        Support.tlr();
        declaredGems = new ArrayList<>(MiningNode.gemRockGems);
        gemId = WeightedChanceItem.class.getDeclaredField("id");
        gemId.setAccessible(true);
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintStream out = new PrintStream(System.out, false, "UTF-8");
        JSONParser parser = new JSONParser();
        // Declared item-name inputs shared by every case (the cache definitions are not loaded here).
        JSONObject names = (JSONObject) parser.parse(new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(args[0])), StandardCharsets.UTF_8));
        for (Object o : names.entrySet()) { java.util.Map.Entry<?, ?> e = (java.util.Map.Entry<?, ?>) o; Support.declaredNames.put(Integer.parseInt((String) e.getKey()), (String) e.getValue()); }
        for (String line; (line = in.readLine()) != null; ) {
            if (line.isEmpty()) continue;
            JSONObject c = (JSONObject) parser.parse(line);
            c.put("expected", run(c));
            out.println(c.toJSONString());
        }
        out.flush();
    }

    private static Object run(JSONObject c) throws Exception {
        Support.reset(c);
        MiningNode.gemRockGems.clear();
        MiningNode.gemRockGems.addAll(declaredGems);
        Registry.interactions.clear();
        Registry.handlers.clear();
        node = null; target = null; pulse = null; scriptState = 0; previousState = null;
        String kind = (String) c.get("kind");
        switch (kind) {
            case "woodcuttingNode": return woodcutting(WoodcuttingNode.values()[Support.n(c.get("index"))]);
            case "miningNode": return mining(MiningNode.values()[Support.n(c.get("index"))]);
            case "tool": return tool(SkillingTool.values()[Support.n(c.get("index"))]);
            case "resource": return resource(SkillingResource.values()[Support.n(c.get("index"))]);
            case "counts": return Support.list(WoodcuttingNode.values().length, MiningNode.values().length, SkillingResource.values().length, SkillingTool.values().length, gems());
            case "lookup": return lookup(c);
            case "respawn": return respawn(c);
            case "random": return random(c);
            case "selectTool": return select(c);
            case "scenario": return scenario(c);
            case "fish": return fish(content.global.skill.fishing.Fish.values()[Support.n(c.get("index"))]);
            case "fishingOption": return fishingOption(content.global.skill.fishing.FishingOption.values()[Support.n(c.get("index"))]);
            case "fishingSpot": return fishingSpot(content.global.skill.fishing.FishingSpot.values()[Support.n(c.get("index"))]);
            case "fishingCounts": return Support.list(content.global.skill.fishing.Fish.values().length, content.global.skill.fishing.FishingOption.values().length, content.global.skill.fishing.FishingSpot.values().length, allSpotIds());
            case "fishingLookup": return fishingLookup(c);
            default: throw new IllegalArgumentException(kind);
        }
    }

    private static JSONObject woodcutting(WoodcuttingNode n) {
        JSONObject o = new JSONObject();
        o.put("name", n.name()); o.put("ordinal", (long) n.ordinal()); o.put("id", (long) n.getId()); o.put("empty", (long) n.getEmptyId());
        o.put("identifier", (long) n.identifier); o.put("farming", n.isFarming()); o.put("reward", (long) n.getReward());
        o.put("respawnRate", (long) n.getRespawnRate()); o.put("level", (long) n.getLevel()); o.put("rewardAmount", (long) n.getRewardAmount());
        o.put("experience", Support.bits(n.getExperience())); o.put("rate", Support.bits(n.getRate()));
        o.put("baseLow", Support.bits(n.baseLow)); o.put("baseHigh", Support.bits(n.baseHigh));
        o.put("tierModLow", Support.bits(n.tierModLow)); o.put("tierModHigh", Support.bits(n.tierModHigh));
        o.put("minimumRespawn", (long) n.getMinimumRespawn()); o.put("maximumRespawn", (long) n.getMaximumRespawn());
        return o;
    }
    private static JSONObject mining(MiningNode n) {
        JSONObject o = new JSONObject();
        o.put("name", n.name()); o.put("ordinal", (long) n.ordinal()); o.put("id", (long) n.getId()); o.put("empty", (long) n.getEmptyId());
        o.put("identifier", (long) n.identifier); o.put("reward", (long) n.getReward()); o.put("respawnRate", (long) n.getRespawnRate());
        o.put("level", (long) n.getLevel()); o.put("rewardAmount", (long) n.getRewardAmount());
        o.put("experience", Support.bits(n.getExperience())); o.put("rate", Support.bits(n.getRate()));
        o.put("minimumRespawn", (long) n.getMinimumRespawn()); o.put("maximumRespawn", (long) n.getMaximumRespawn());
        return o;
    }
    private static JSONObject tool(SkillingTool t) {
        JSONObject o = new JSONObject();
        o.put("name", t.name()); o.put("ordinal", (long) t.ordinal()); o.put("id", (long) t.getId()); o.put("level", (long) t.getLevel());
        o.put("ratio", Support.bits(t.getRatio())); o.put("animation", (long) t.getAnimation().getId());
        SkillingTool first = SkillingTool.forId(t.getId());
        o.put("forId", first == null ? null : first.name());
        return o;
    }
    private static JSONObject resource(SkillingResource r) {
        JSONObject o = new JSONObject();
        o.put("name", r.name()); o.put("ordinal", (long) r.ordinal()); o.put("id", (long) r.getId()); o.put("level", (long) r.getLevel());
        o.put("rate", Support.bits(r.getRate())); o.put("respawnRate", (long) r.getRespawnRate()); o.put("experience", Support.bits(r.getExperience()));
        o.put("reward", (long) r.getReward()); o.put("rewardAmount", (long) r.getRewardAmount()); o.put("label", r.getName());
        o.put("animation", r.getAnimation() == null ? null : (Object) (long) r.getAnimation().getId()); o.put("empty", (long) r.getEmptyId());
        o.put("skill", (long) r.getSkillId()); o.put("farming", r.isFarming());
        o.put("minimumRespawn", (long) r.getMinimumRespawn()); o.put("maximumRespawn", (long) r.getMaximumRespawn());
        return o;
    }
    private static JSONArray gems() throws Exception {
        JSONArray a = new JSONArray();
        Field min = WeightedChanceItem.class.getDeclaredField("minimum_amount"), max = WeightedChanceItem.class.getDeclaredField("maximum_amount");
        min.setAccessible(true); max.setAccessible(true);
        for (WeightedChanceItem g : MiningNode.gemRockGems) a.add(Support.list(gemId.getInt(g), min.getInt(g), max.getInt(g), g.weight));
        return a;
    }
    private static JSONArray gemOrder() throws Exception {
        JSONArray a = new JSONArray();
        for (WeightedChanceItem g : MiningNode.gemRockGems) a.add((long) gemId.getInt(g));
        return a;
    }
    private static JSONArray lookup(JSONObject c) {
        JSONArray out = new JSONArray();
        String table = (String) c.get("table");
        for (Object o : (JSONArray) c.get("ids")) {
            int id = Support.n(o);
            switch (table) {
                case "woodcutting": { WoodcuttingNode n = WoodcuttingNode.forId(id); out.add(Support.list(n == null ? null : n.name(), WoodcuttingNode.isEmpty(id))); break; }
                case "mining": { MiningNode n = MiningNode.forId(id); out.add(Support.list(n == null ? null : n.name(), MiningNode.isEmpty(id))); break; }
                case "resource": { SkillingResource n = SkillingResource.forId(id); out.add(Support.list(n == null ? null : n.name(), SkillingResource.isEmpty(id))); break; }
                default: { SkillingTool n = SkillingTool.forId(id); out.add(Support.list(n == null ? null : n.name(), false)); }
            }
        }
        return out;
    }
    private static JSONObject respawn(JSONObject c) {
        String table = (String) c.get("table"), name = (String) c.get("name");
        int v = table.equals("woodcutting") ? WoodcuttingNode.valueOf(name).getRespawnDuration()
            : table.equals("mining") ? MiningNode.valueOf(name).getRespawnDuration() : SkillingResource.valueOf(name).getRespawnDuration();
        JSONObject o = new JSONObject(); o.put("ret", (long) v); o.put("events", Support.events); return o;
    }
    private static double d(Object bits) { return Double.longBitsToDouble(Long.parseUnsignedLong(((String) bits).substring(2), 16)); }
    private static JSONArray random(JSONObject c) throws Exception {
        JSONArray out = new JSONArray();
        for (Object o : (JSONArray) c.get("calls")) {
            JSONArray call = (JSONArray) o, a = (JSONArray) call.get(1);
            Support.events = new JSONArray();
            Object r;
            switch ((String) call.get(0)) {
                case "random": r = (long) RandomFunction.random(Support.n(a.get(0))); break;
                case "randomRange": r = (long) RandomFunction.random(Support.n(a.get(0)), Support.n(a.get(1))); break;
                case "roll": r = RandomFunction.roll(Support.n(a.get(0))); break;
                case "randomize": r = (long) RandomFunction.randomize(Support.n(a.get(0))); break;
                case "getRandom": r = (long) RandomFunction.getRandom(Support.n(a.get(0))); break;
                case "chance": r = Support.bits(RandomFunction.getSkillSuccessChance(d(a.get(0)), d(a.get(1)), Support.n(a.get(2)))); break;
                case "randomDouble": r = Support.bits(RandomFunction.randomDouble(d(a.get(0)))); break;
                case "gems": r = (long) RandomFunction.rollWeightedChanceTable(MiningNode.gemRockGems).getId(); break;
                default: throw new IllegalArgumentException((String) call.get(0));
            }
            JSONObject x = new JSONObject(); x.put("ret", r); x.put("events", Support.events);
            if (call.get(0).equals("randomDouble")) x.put("tlr", Long.toString(Support.tlr()));
            if (call.get(0).equals("gems")) x.put("gems", gemOrder());
            out.add(x);
        }
        return out;
    }
    private static JSONObject select(JSONObject c) {
        Player p = Support.player;
        Object r;
        String method = (String) c.get("method");
        SkillingTool t;
        switch (method) {
            case "getHatchet": t = SkillingTool.getHatchet(p); r = t == null ? null : t.name(); break;
            case "getPickaxe": t = SkillingTool.getPickaxe(p); r = t == null ? null : t.name(); break;
            case "getHarpoon": t = SkillingTool.getHarpoon(p); r = t == null ? null : t.name(); break;
            case "getButterflyNet": t = SkillingTool.getButterflyNet(p); r = t == null ? null : t.name(); break;
            case "getToolForSkill": t = SkillingTool.getToolForSkill(p, Support.n(c.get("skill"))); r = t == null ? null : t.name(); break;
            case "checkTool": r = SkillingTool.checkTool(p, Support.n(c.get("skill")), SkillingTool.valueOf((String) c.get("tool"))); break;
            default: throw new IllegalArgumentException(method);
        }
        JSONObject o = new JSONObject(); o.put("ret", r); o.put("events", Support.events); return o;
    }

    private static JSONArray allSpotIds() { JSONArray a = new JSONArray(); for (int id : content.global.skill.fishing.FishingSpot.Companion.getAllIds()) a.add((long) id); return a; }
    private static JSONObject fish(content.global.skill.fishing.Fish f) {
        JSONObject o = new JSONObject();
        o.put("name", f.name()); o.put("ordinal", (long) f.ordinal()); o.put("id", (long) f.getId()); o.put("level", (long) f.getLevel());
        o.put("experience", Support.bits(f.getExperience())); o.put("lowChance", Support.bits(f.getLowChance())); o.put("highChance", Support.bits(f.getHighChance()));
        o.put("bigFish", content.global.skill.fishing.Fish.getBigFish(f) == null ? null : (Object) (long) content.global.skill.fishing.Fish.getBigFish(f));
        content.global.skill.fishing.Fish.Companion k = content.global.skill.fishing.Fish.Companion;
        o.put("barehandFishingReq", k.getBarehandFishingReq().get(f) == null ? null : (Object) (long) k.getBarehandFishingReq().get(f));
        o.put("barehandStrengthReq", k.getBarehandStrengthReq().get(f) == null ? null : (Object) (long) k.getBarehandStrengthReq().get(f));
        o.put("barehandStrengthXp", k.getBarehandStrengthXp().get(f) == null ? null : Support.bits(k.getBarehandStrengthXp().get(f)));
        content.global.skill.fishing.Fish first = content.global.skill.fishing.Fish.forItem(new core.game.node.item.Item(f.getId()));
        o.put("forItem", first == null ? null : first.name());
        return o;
    }
    private static JSONObject fishingOption(content.global.skill.fishing.FishingOption f) {
        JSONObject o = new JSONObject();
        o.put("name", f.name()); o.put("ordinal", (long) f.ordinal()); o.put("tool", (long) f.getTool()); o.put("level", (long) f.getLevel());
        o.put("animation", (long) f.getAnimation().getId()); o.put("option", f.getOption()); o.put("startMessage", f.getStartMessage());
        JSONArray bait = null; if (f.getBait() != null) { bait = new JSONArray(); for (int b : f.getBait()) bait.add((long) b); }
        o.put("bait", bait);
        JSONArray caught = new JSONArray(); for (content.global.skill.fishing.Fish x : f.getFish()) caught.add(x.name());
        o.put("fish", caught);
        content.global.skill.fishing.FishingOption named = content.global.skill.fishing.FishingOption.Companion.forName(f.getOption());
        o.put("forName", named == null ? null : named.name());
        return o;
    }
    private static JSONObject fishingSpot(content.global.skill.fishing.FishingSpot f) {
        JSONObject o = new JSONObject();
        o.put("name", f.name()); o.put("ordinal", (long) f.ordinal());
        JSONArray ids = new JSONArray(); for (int b : f.getIds()) ids.add((long) b);
        o.put("ids", ids);
        JSONArray options = new JSONArray(); for (content.global.skill.fishing.FishingOption x : f.getOptions()) options.add(x.name());
        o.put("options", options);
        return o;
    }
    private static JSONArray fishingLookup(JSONObject c) {
        JSONArray out = new JSONArray();
        String table = (String) c.get("table");
        for (Object o : (JSONArray) c.get("keys")) {
            switch (table) {
                case "spot": { content.global.skill.fishing.FishingSpot s = content.global.skill.fishing.FishingSpot.Companion.forId(Support.n(o)); out.add(s == null ? null : s.name()); break; }
                case "fish": { content.global.skill.fishing.Fish s = content.global.skill.fishing.Fish.forItem(new core.game.node.item.Item(Support.n(o))); out.add(s == null ? null : s.name()); break; }
                case "option": { content.global.skill.fishing.FishingOption s = content.global.skill.fishing.FishingOption.Companion.forName((String) o); out.add(s == null ? null : s.name()); break; }
                case "chance": { JSONArray a = (JSONArray) o; out.add(Support.bits(content.global.skill.fishing.Fish.valueOf((String) a.get(0)).getSuccessChance(Support.n(a.get(1))))); break; }
                default: { JSONArray a = (JSONArray) o; content.global.skill.fishing.FishingOption s = content.global.skill.fishing.FishingSpot.valueOf((String) a.get(0)).getOptionByName((String) a.get(1)); out.add(s == null ? null : s.name()); }
            }
        }
        return out;
    }

    private static JSONArray scenario(JSONObject c) throws Exception {
        // The listener is registered before the first operation; only an explicit "define" records it.
        register((String) c.get("subject"));
        Support.reset(c);
        JSONObject n = (JSONObject) c.get("node");
        node = new Scenery(Support.n(n.get("id")), Support.loc((JSONArray) n.get("location")), Support.i(n, "type", 10), Support.i(n, "rotation", 0), true);
        node.rawActive = Support.flag(n, "active", true);
        target = node;
        String subject = (String) c.get("subject");
        if (subject.startsWith("fishing") && !"scenery".equals(n.get("kind"))) {
            // The scenery above stays registered but is never touched; the spot is the interacted node.
            core.game.node.entity.npc.NPC spot = new core.game.node.entity.npc.NPC(Support.n(n.get("id")), Support.loc((JSONArray) n.get("location")));
            spot.ref = Support.register(spot); spot.rawActive = Support.flag(n, "active", true); spot.rawInvisible = Support.flag(n, "invisible", false);
            target = spot;
        }
        JSONArray out = new JSONArray();
        for (Object o : (JSONArray) c.get("ops")) {
            JSONObject op = (JSONObject) o, x = new JSONObject();
            Support.events = new JSONArray();
            Support.failAt = Support.i(op, "failAt", -1);
            try {
                x.put("ret", step(subject, op));
            } catch (Support.Failure e) {
                x.put("error", "service");
            } catch (RuntimeException e) {
                x.put("error", e.getClass().getSimpleName());
            }
            Support.failAt = -1;
            x.put("events", Support.events);
            JSONObject s = Support.state();
            s.put("gems", gemOrder()); s.put("scriptState", (long) scriptState);
            if (pulse != null) s.put("pulse", Support.list(pulse.isRunning(), pulse.getDelay(), pulse.getTicksPassed()));
            if (pulse instanceof content.global.skill.gather.fishing.FishingPulse) {
                Field caught = content.global.skill.gather.fishing.FishingPulse.class.getDeclaredField("fish"); caught.setAccessible(true);
                Object held = caught.get(pulse); s.put("fish", held == null ? null : ((Enum<?>) held).name());
            }
            if (Support.extraState != null) Support.extraState.accept(s);
            // Only the entries that changed since the previous operation are written.
            JSONObject delta = new JSONObject();
            for (Object key : s.keySet()) if (previousState == null || !java.util.Objects.equals(previousState.get(key), s.get(key))) delta.put(key, s.get(key));
            previousState = s;
            x.put("state", delta);
            out.add(x);
        }
        return out;
    }

    private static void register(String subject) {
        Registry.interactions.clear();
        Registry.handlers.clear();
        switch (subject) {
            case "woodcutting": new content.global.skill.gather.woodcutting.WoodcuttingListener().defineListeners(); break;
            case "mining": new content.global.skill.gather.mining.MiningListener().defineListeners(); break;
            case "fishing": new content.global.skill.gather.fishing.FishingListener().defineListeners(); break;
            case "prospect": new content.global.skill.gather.mining.ProspectListener().defineListeners(); break;
            default: break;
        }
    }

    private static Object step(String subject, JSONObject op) {
        Player p = Support.player;
        switch ((String) op.get("op")) {
            case "define":
                register(subject);
                Support.hasScript = true; Support.scriptNext = 0; scriptState = 0;
                return null;
            case "handler":
                return Registry.interactions.get(0).invoke(p, target, Support.n(op.get("state")));
            case "tick": {
                // Driver modelled on ScriptProcessor.processInteractScript; not the original processor.
                Support.ticks++;
                if (!Support.hasScript || !(Support.scriptNext < Support.ticks)) return "idle";
                boolean finished = Registry.interactions.get(0).invoke(p, target, scriptState);
                scriptState++;
                if (finished && Support.scriptPersist) scriptState = 0;
                return finished;
            }
            case "script":
                return Support.scripts.get(Support.n(op.get("index"))).invoke(Support.n(op.get("stage")));
            case "on":
                return Registry.handlers.get(0).invoke(p, node);
            case "runPulse":
                return Support.pulses.get(Support.n(op.get("index"))).pulse();
            case "pulseNew":
                if (subject.equals("fishingPulse")) {
                    Object name = op.get("option");
                    pulse = new content.global.skill.gather.fishing.FishingPulse(p, (core.game.node.entity.npc.NPC) target, name == null ? null : content.global.skill.fishing.FishingOption.valueOf((String) name));
                    return null;
                }
                pulse = subject.equals("woodcuttingPulse") ? new content.global.skill.gather.woodcutting.WoodcuttingSkillPulse(p, node)
                    : new content.global.skill.gather.mining.MiningSkillPulse(p, node);
                return null;
            case "pulseStart": pulse.start(); return null;
            case "pulseStop": pulse.stop(); return null;
            case "pulsePulse": return pulse.pulse();
            case "pulseUpdate": Support.ticks++; return pulse.update();
            // Public SkillPulse members, called directly.
            case "pulseCheck": return ((core.game.node.entity.skill.SkillPulse<?>) pulse).checkRequirements();
            case "pulseAnimate": ((core.game.node.entity.skill.SkillPulse<?>) pulse).animate(); return null;
            case "pulseReward": return ((core.game.node.entity.skill.SkillPulse<?>) pulse).reward();
            case "pulseMessage": ((core.game.node.entity.skill.SkillPulse<?>) pulse).message(Support.n(op.get("type"))); return null;
            case "set":
                if (Support.extraSet != null) Support.extraSet.accept(op);
                if (op.get("ticks") != null) Support.ticks = Support.n(op.get("ticks"));
                Support.levels(op.get("levels"));
                if (op.get("fill") != null) { int k = Support.n(op.get("fill")); for (int s = 0; s < 28 && 28 - Support.freeSlots() < k; s++) if (Support.inventory[s] == null) Support.inventory[s] = new core.game.node.item.Item(1); }
                if (op.get("animating") != null) Support.animating = (Boolean) op.get("animating");
                if (op.get("nodeActive") != null) target.rawActive = (Boolean) op.get("nodeActive");
                if (op.get("npcLocation") != null) target.rawLocation = Support.loc((JSONArray) op.get("npcLocation"));
                if (op.get("npcInvisible") != null) ((core.game.node.entity.npc.NPC) target).rawInvisible = (Boolean) op.get("npcInvisible");
                if (op.get("usedOption") != null) Support.usedOption = (String) op.get("usedOption");
                if (op.get("take") != null) Support.take(Support.n(op.get("take")));
                if (op.get("give") != null) { JSONArray a = (JSONArray) op.get("give"); Support.add(Support.n(a.get(0)), Support.n(a.get(1))); }
                if (op.get("attribute") != null) { JSONArray a = (JSONArray) op.get("attribute"); Support.attributes.put((String) a.get(0), a.get(1) instanceof Number ? (Object) ((Number) a.get(1)).intValue() : a.get(1)); }
                if (op.get("hasFamiliar") != null) Support.hasFamiliar = (Boolean) op.get("hasFamiliar");
                if (op.get("delay") != null) pulse.setDelay(Support.n(op.get("delay")));
                if (op.get("playerActive") != null) Support.playerActive = (Boolean) op.get("playerActive");
                if (op.get("hasScript") != null) Support.hasScript = (Boolean) op.get("hasScript");
                if (op.get("addResult") != null) Support.addResult = (Boolean) op.get("addResult");
                if (op.get("clock") != null) { JSONArray a = (JSONArray) op.get("clock"); Support.clocks[Support.n(a.get(0))] = Support.n(a.get(1)); }
                if (op.get("location") != null) Support.location = Support.loc((JSONArray) op.get("location"));
                if (op.get("sharedSeed") != null) Support.SHARED.setSeed(Support.l(op, "sharedSeed", 0));
                if (op.get("mathSeed") != null) Support.MATH.setSeed(Support.l(op, "mathSeed", 0));
                if (op.get("shuffleSeed") != null) Support.SHUFFLE.setSeed(Support.l(op, "shuffleSeed", 0));
                if (op.get("tlrSeed") != null) Support.tlr(Support.l(op, "tlrSeed", 0));
                return null;
            default: throw new IllegalArgumentException((String) op.get("op"));
        }
    }
}
