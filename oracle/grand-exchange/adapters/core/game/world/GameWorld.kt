package core.game.world

/** The GameWorld members the shop and interface classes read. */
object GameWorld {
    @JvmStatic var ticks = 0
    @JvmStatic var settings: GameSettings? = null
    @JvmStatic var Pulser = PulseRunner()
}
