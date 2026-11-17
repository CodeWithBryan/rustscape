package oracle
import core.game.node.entity.player.Player
import core.net.packet.`in`.Packet
import core.game.world.GameWorld
import core.game.system.task.Pulse
import core.game.node.entity.player.info.login.LoginConfiguration
import core.game.interaction.InterfaceListeners
import core.game.event.ButtonClickEvent
object Processor {
/* ORIGINAL_METHODS */
@JvmStatic fun action(packet:Packet.IfAction){processIfAction(packet)}
@JvmStatic fun close(packet:Packet.CloseIface){processCloseIface(packet)}
}
