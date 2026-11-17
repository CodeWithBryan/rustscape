package core.game.world
import core.game.system.task.Pulse
object GameWorld {
 val pulses=ArrayList<Pulse>()
 object Pulser {fun submit(pulse:Pulse){oracle.Oracle.emit("submit");pulses.add(pulse)}}
}
