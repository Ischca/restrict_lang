record Enemy { hp: Int, atk: Int }

val base = Enemy { hp = 100, atk = 10 }
val boss = base.clone { hp = 500 } freeze

impl Enemy {
    fun attack = self: Enemy tgt: Player { tgt.damage self.atk }
}

fun main = {
    boss luke.attack
}