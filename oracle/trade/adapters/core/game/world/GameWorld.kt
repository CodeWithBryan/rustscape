package core.game.world

/** The three GameWorld members the trade reads. */
object GameWorld {
    @JvmStatic var ticks = 0
    @JvmStatic var settings: GameSettings? = null
    @JvmStatic var Pulser = PulseRunner()
}