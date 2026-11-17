package core.game.world
object GameWorld {
 @JvmField var slots=arrayOf(Settings(0),Settings(1))
 @JvmField var active=0
 @JvmStatic val settings:Settings?
  get(){oracle.WriterOracle.hit("world");return if(active<0)null else slots[active]}
 class Settings(val ref:Int) {
  @JvmField var pvp=false
  @JvmField var wild=false
  val isPvp:Boolean get(){oracle.WriterOracle.hit("pvp",ref);return pvp}
  val wild_pvp_enabled:Boolean get(){oracle.WriterOracle.hit("wild",ref);return wild}
 }
}
