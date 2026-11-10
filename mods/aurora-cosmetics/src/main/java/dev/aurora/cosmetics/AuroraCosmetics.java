package dev.aurora.cosmetics;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import net.minecraft.client.renderer.entity.player.AvatarRenderer;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.LivingEntityRenderLayerRegistrationCallback;
import net.fabricmc.fabric.api.client.rendering.v1.RenderStateDataKey;

/** Ponto de entrada: carrega os modelos, a configuração e registra a camada de cosméticos no jogador. */
public final class AuroraCosmetics implements ClientModInitializer {
	public static final String MOD_ID = "aurora_cosmetics";
	public static final Logger LOG = LoggerFactory.getLogger("AuroraCosmetics");

	/** Dados do jogador copiados para o estado de renderização (nome, tempo, movimento). */
	public static final RenderStateDataKey<PlayerData> PLAYER = RenderStateDataKey.create(() -> "aurora_cosmetics player data");

	@Override
	@SuppressWarnings({"rawtypes", "unchecked"})
	public void onInitializeClient() {
		CosmeticRegistry.load();
		CosmeticSync.init();

		LivingEntityRenderLayerRegistrationCallback.EVENT.register((entityType, entityRenderer, registrationHelper, context) -> {
			if (entityRenderer instanceof AvatarRenderer) {
				registrationHelper.register(new CosmeticsLayer((AvatarRenderer) entityRenderer));
			}
		});
		LOG.info("Aurora Cosmetics pronto: {} cosméticos 3D", CosmeticRegistry.size());
	}
}
