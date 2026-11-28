@file:JvmName("ContentAPIKt")
package core.api

import core.game.node.entity.Entity
import core.game.world.update.flag.context.Animation

/** Adapter with the original's body: the animator's animation, forced or not. */
fun <T> animate(entity: Entity, anim: T, forced: Boolean = false) {
    val animation = when (anim) {
        is Int -> Animation(anim)
        is Animation -> anim
        else -> throw IllegalStateException("Invalid value passed for anim")
    }
    val animator = (entity as core.game.node.entity.player.Player).animator
    if (forced) {
        animator.forceAnimation(animation)
    } else {
        animator.animate(animation)
    }
}

/** Adapter with the original's body. */
fun lock(entity: Entity, duration: Int) {
    entity.lock(duration)
}
