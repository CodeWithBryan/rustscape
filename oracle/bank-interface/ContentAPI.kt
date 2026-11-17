package core.api
import core.game.node.entity.player.Player
import oracle.Oracle
interface ContentInterface
fun <T> getAttribute(p:Player,key:String,fallback:T):T = p.attribute(key,fallback) as T
fun <T> setAttribute(p:Player,key:String,value:T){p.attributes[key]=value;Oracle.emit("attribute",p.id,key,value)}
fun setVarc(p:Player,id:Int,value:Int){Oracle.emit("varc",p.id,id,value)}
fun openDialogue(p:Player,dialogue:Any){Oracle.emit("dialogue",p.id,if(dialogue is content.global.dialogue.BankHelpDialogue)0 else 1)}
fun sendMessage(p:Player,text:String){Oracle.emit("message",p.id,text)}
fun openBankAccount(p:Player){Oracle.emit("openBank",p.id)}
