package oracle;

import content.data.tables.BirdNest;
import content.global.handlers.item.SpadeDigListener;
import content.global.handlers.item.equipment.brawling_gloves.BrawlingGloves;
import content.global.handlers.item.equipment.brawling_gloves.BrawlingGlovesManager;
import content.global.handlers.item.equipment.fistofguthixgloves.FOGGlovesManager;
import core.game.global.action.DigSpadeHandler;
import core.game.node.Node;
import core.game.node.item.Item;
import core.game.world.map.Location;
import java.util.Map;

/** The static entry points other content calls (the `call` operation). */
public class Calls {
    static int i(Map op, String k) { return ((Number) op.get(k)).intValue(); }
    static Location at(Map op) { return Location.create(i(op, "x"), i(op, "y"), i(op, "z")); }
    static String name(Enum<?> e) { return e == null ? null : e.name(); }

    static Object call(String name, Map op, Node node) throws Throwable {
        switch (name) {
            case "spadeRegister": {
                String tag = (String) op.get("tag");
                SpadeDigListener.INSTANCE.registerListener(at(op), p -> { Support.emit("spadeListener", tag); return kotlin.Unit.INSTANCE; });
                return null;
            }
            case "spadeRun": return SpadeDigListener.runListener(at(op), Support.player);
            case "digRegister": {
                String tag = (String) op.get("tag");
                return DigSpadeHandler.register(at(op), p -> Support.emit("digAction", tag));
            }
            case "dig": return DigSpadeHandler.dig(Support.player);
            case "nestDrop": BirdNest.drop(Support.player); return null;
            case "nestRandom": return name(BirdNest.getRandomNest(Boolean.TRUE.equals(op.get("wyson"))));
            case "nestFor": return name(BirdNest.forNest((Item) node));
            case "nestSearch": BirdNest.valueOf((String) op.get("nest")).search(Support.player, (Item) node); return null;
            case "fogUpdate": return FOGGlovesManager.updateCharges(Support.player, i(op, "charges"));
            case "bgRegister": {
                BrawlingGlovesManager m = BrawlingGlovesManager.getInstance(Support.player);
                if (op.containsKey("charges")) m.registerGlove(i(op, "id"), i(op, "charges")); else m.registerGlove(i(op, "id"));
                return null;
            }
            case "bgUpdate": BrawlingGlovesManager.getInstance(Support.player).updateCharges(i(op, "glove"), i(op, "charges")); return null;
            case "bgBonus": return BrawlingGlovesManager.getInstance(Support.player).getExperienceBonus();
            case "bgCharges": return new java.util.TreeMap<>(BrawlingGlovesManager.getInstance(Support.player).GloveCharges).toString();
            case "bgForId": return name(BrawlingGloves.forId(i(op, "value")));
            case "bgForIndicator": return name(BrawlingGloves.forIndicator(i(op, "value")));
            case "bgForSkill": return name(BrawlingGloves.forSkill(i(op, "value")));
            case "bgName": return BrawlingGloves.valueOf((String) op.get("glove")).getName();
            case "tlrInt": return java.util.concurrent.ThreadLocalRandom.current().nextInt(i(op, "bound"));
            case "tlrDouble": return Double.doubleToRawLongBits(core.tools.RandomFunction.randomDouble(((Number) op.get("max")).doubleValue()));
            case "random": return core.tools.RandomFunction.random(i(op, "bound"));
            case "randomRange": return core.tools.RandomFunction.random(i(op, "a"), i(op, "b"));
            default: throw new IllegalArgumentException(name);
        }
    }
}
