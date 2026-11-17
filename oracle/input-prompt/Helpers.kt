package core.api
import core.game.node.entity.player.Player
fun sendDialogue(player:Player,message:String){oracle.Oracle.emit("dialogue",player.id,oracle.Oracle.units(message))}
fun removeAttribute(player:Player,key:String){player.removeAttribute(key)}
/* ORIGINAL_PROMPTS */
