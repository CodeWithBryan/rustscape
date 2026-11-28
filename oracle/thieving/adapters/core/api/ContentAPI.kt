@file:JvmName("ContentAPIKt")
package core.api

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.update.flag.context.Animation
import oracle.W

/** Adapter: records the animation of an entity (an Int or an Animation, as the original accepts). */
fun <T> animate(entity: Entity, anim: T, forced: Boolean = false) {
    val animation = when (anim) {
        is Int -> Animation(anim)
        is Animation -> anim
        else -> throw IllegalStateException("Invalid value passed for anim")
    }
    W.emit("animate", if (entity is Player) "player" else "npc", animation.id, animation.priority.name, forced)
}

/** Adapter: records the call. */
fun playAudio(player: Player, id: Int, delay: Int = 0) { W.emit("playAudio", id, delay) }

/** Adapter: records the call; the sound it picks is not covered. */
fun playHurtAudio(player: Player, delay: Int = 0) { W.emit("playHurtAudio", delay) }

/** Adapter: records the call; what a stun does is not covered. */
fun stun(entity: Entity, ticks: Int) { W.emit("stun", ticks) }
