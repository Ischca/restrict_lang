impl Enemy {
    fun attack = self: Enemy tgt: Player { tgt.damage self.atk }
}