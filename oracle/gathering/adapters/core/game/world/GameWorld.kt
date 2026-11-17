package core.game.world

import core.game.system.task.Pulse
import oracle.Support

/** Recording stand-ins for the three GameWorld members the fishing sources read. */
object GameWorld {
    @JvmStatic val settings: GameSettings? get() = if (Support.emit("gameWorld.settings", Support.settingsPresent)) GameSettings() else null
    @JvmStatic val ticks: Int get() = Support.emit("gameWorld.ticks", Support.ticks)
    @JvmStatic val Pulser = PulseRunner()
}
class GameSettings {
    val isDevMode: Boolean get() = Support.emit("gameSettings.isDevMode", Support.devMode)
}
class PulseRunner {
    fun submit(pulse: Pulse) {
        Support.pulses.add(pulse)
        Support.emit("pulser.submit", null, pulse.delay)
    }
}