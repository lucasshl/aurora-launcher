package dev.aurora.cosmetics.mixin;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import net.minecraft.client.entity.ClientAvatarEntity;
import net.minecraft.client.renderer.entity.player.AvatarRenderer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.world.entity.Avatar;

import dev.aurora.cosmetics.AuroraCosmetics;
import dev.aurora.cosmetics.PlayerData;

/** Copia nome e movimento do jogador para o AvatarRenderState (mesmo alvo usado pelos testes da Fabric API 26.3). */
@Mixin(AvatarRenderer.class)
public abstract class AvatarRendererMixin<AvatarlikeEntity extends Avatar & ClientAvatarEntity> {
	@Inject(method = "extractRenderState(Lnet/minecraft/world/entity/Avatar;Lnet/minecraft/client/renderer/entity/state/AvatarRenderState;F)V", at = @At("RETURN"))
	private void auroraCosmetics$extract(AvatarlikeEntity entity, AvatarRenderState state, float partialTicks, CallbackInfo ci) {
		state.setData(AuroraCosmetics.PLAYER, new PlayerData(
				entity.getName().getString(),
				entity.tickCount + partialTicks,
				(float) entity.getDeltaMovement().horizontalDistance(),
				entity.isCrouching(),
				entity.isFallFlying(),
				entity.isInvisible()
		));
	}
}
