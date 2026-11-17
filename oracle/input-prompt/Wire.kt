package oracle
import core.game.node.entity.player.Player
import core.net.packet.IoBuffer
import core.tools.StringUtils
interface Decoder {fun decode(player:Player,buffer:IoBuffer):Packet}
/* ORIGINAL_DECODERS */
object Wire {
 @JvmStatic fun decode(opcode:Int,player:Player,buffer:IoBuffer):Any {
  val decoder:Decoder=when(opcode){244->Decoder244();65->Decoder65();23->Decoder23();else->throw IllegalArgumentException()}
  return (decoder.decode(player,buffer) as Packet.InputPromptResponse).response
 }
}
