package core.net.packet.`in`
import core.game.node.entity.player.Player
object Packet {
 data class IfAction(val player:Player,val iface:Int,val child:Int,val slot:Int,val itemId:Int,val opcode:Int,val optIndex:Int)
 data class CloseIface(val player:Player)
}
