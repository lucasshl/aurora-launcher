package dev.aurora.cosmetics;

/**
 * @param name      nick do jogador (chave para buscar os cosméticos)
 * @param age       ticks + partialTick, usado nas animações
 * @param speed     velocidade horizontal (blocos/tick)
 * @param crouching agachado
 * @param flying    planando de élitra
 * @param invisible invisível (poção) — não desenha nada
 */
public record PlayerData(String name, float age, float speed, boolean crouching, boolean flying, boolean invisible) {
}
