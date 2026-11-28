package core.tools;
/** Adapter shell around the pinned random(int, int), random(int) and getSkillSuccessChance, spliced verbatim; RANDOM draws from the case and randomDouble(min, max) is the case's declared chance. */
public class RandomFunction {
    public static final java.util.Random RANDOM = new oracle.W.Draws();
    /* RANDOM_AB */
    /* RANDOM_MAX */
    /* SKILL_SUCCESS_CHANCE */
    public static final double randomDouble(double min, double max) { return oracle.W.chance(min, max); }
}
