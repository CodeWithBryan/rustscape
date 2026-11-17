public static class Skull {
 public final int ref; public boolean wilderness,disabled;public int levelField=17;
 public Skull(int r){ref=r;}
 public boolean isWilderness(){oracle.WriterOracle.hit("wilderness",ref);return wilderness;}
 public boolean isWildernessDisabled(){oracle.WriterOracle.hit("disabled",ref);return disabled;}
 public int getLevel(){oracle.WriterOracle.hit("level",ref);return levelField;}
}
public static class Familiar {
 public final int ref;public boolean using;public int summoning=6;
 public Familiar(int r){ref=r;}
 public boolean isUsingSummoning(){oracle.WriterOracle.hit("using",ref);return using;}
 public int getSummoningCombatLevel(){oracle.WriterOracle.hit("summoning",ref);return summoning;}
}
public static class Skills {
 public final int ref;public int total=2001;public Skills(int r){ref=r;}
 public int getTotalLevel(){oracle.WriterOracle.hit("total",ref);return total;}
}
