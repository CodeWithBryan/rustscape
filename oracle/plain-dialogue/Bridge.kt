package oracle
import oracle.Oracle.Player
import core.api.splitLines
import core.game.interaction.InterfaceListeners
object Packet {data class ContinueOption(val player:Player,val iface:Int,val child:Int,val slot:Int,val opcode:Int)}
object Bridge {
 @JvmStatic fun split(message:String,limit:Int):Array<String> = splitLines(message,limit)
 @JvmStatic fun send(player:Player,message:String) { sendDialogue(player,message) }
 @JvmStatic fun option(pkt:Packet.ContinueOption){
 /* ORIGINAL_CONTINUE */
 }
}
/* ORIGINAL_CONTENT_SEND */
