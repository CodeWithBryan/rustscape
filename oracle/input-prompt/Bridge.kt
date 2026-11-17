package oracle
import core.game.node.entity.player.Player
import core.api.*
import core.net.packet.`in`.RunScript
sealed class Packet {data class InputPromptResponse(val player:Player,val response:Any):Packet()}
object Bridge {
 @JvmStatic fun receive(pkt:Packet.InputPromptResponse){
 /* ORIGINAL_INPUT_BRANCH */
 }
 @JvmStatic fun process(p:Player,value:Any,script:Any){RunScript.processInput(p,value,script as ((Any)->Boolean))}
 @JvmStatic fun send(p:Player,kind:Int,prompt:String,script:Any){sendInputDialogue(p,InputType.values()[kind],prompt,script as ((Any)->Unit))}
 @JvmStatic fun transfer(p:Player,slot:Int,withdraw:Boolean,after:Any?){content.global.handlers.iface.bank.BankUtils.transferX(p,slot,withdraw,after as (() -> Unit)?)}
}
