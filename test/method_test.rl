record Enemy { hp: Int atk: Int }

impl Enemy {
    fun attack = self: Enemy tgt: Int { 
        self.atk + tgt
    }
}

fun main = {
    val damage = (Enemy { hp = 500, atk = 50 }, 10) attack
    damage
}