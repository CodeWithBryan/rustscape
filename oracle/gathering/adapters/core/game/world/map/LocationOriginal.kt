package core.game.world.map

import kotlin.math.*

// The two expressions below are spliced from the pinned Location.kt at generation time.
fun Location.originalWithinMaxnormDistance(other: Location, dist: Int): Boolean = /* WITHIN_MAXNORM_DISTANCE */
fun Location.originalTransform(dir: Direction, steps: Int): Location = /* TRANSFORM_DIRECTION */