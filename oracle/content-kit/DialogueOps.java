// Harness actions oracle/content-kit/dialogue.py splices into the dialogue-dsl Oracle.act
// (before its default branch) and helpers it adds to the Oracle class. Not compiled on its own.
//// actions
   case"register":{Object p=Class.forName((String)a.get("class")).getDeclaredConstructor().newInstance();((DialoguePlugin)p).init();return null;}
   case"openId":{NPC n=npc(a);int key=Rec.n(a.get("key"));return n==null?interp.open(key):interp.open(key,n);}
//// helpers
 /** GameWorld.settings: absent "settings" is the default world; a null "settings" is no settings. */
 public static core.game.world.GameWorld.Settings settings(){if(world.containsKey("settings")&&world.get("settings")==null)return null;Object n=world.get("serverName");return new core.game.world.GameWorld.Settings(n==null?"2009scape":text(n));}
