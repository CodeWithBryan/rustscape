@file:JvmName("ContentAPIKt")
package core.api

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.scenery.Scenery
import oracle.W

/** Adapter: records the call; the door's walk is DoorActionHandler's, not covered here. */
fun handleAutowalkDoor(entity: Entity, door: Scenery): Boolean {
    W.emit("autowalk")
    return true
}

/** Adapter: records the message; the original splits it into lines of 86 units. */
fun sendMessage(player: Player, message: String) {
    W.emit("sendMessage", message)
}
