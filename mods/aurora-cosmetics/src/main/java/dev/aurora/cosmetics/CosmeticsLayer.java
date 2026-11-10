package dev.aurora.cosmetics;

import java.util.List;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.math.Axis;

import net.minecraft.client.model.player.PlayerModel;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.RenderLayerParent;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.rendertype.RenderTypes;

import dev.aurora.cosmetics.CosmeticRegistry.Cube;
import dev.aurora.cosmetics.CosmeticRegistry.Item;
import dev.aurora.cosmetics.CosmeticRegistry.Part;

/** Desenha os cosméticos 3D presos aos ossos do modelo do jogador (cabeça, tronco ou raiz). */
public final class CosmeticsLayer extends RenderLayer<AvatarRenderState, PlayerModel> {
	private static final float DEG = (float) (Math.PI / 180.0);

	public CosmeticsLayer(RenderLayerParent<AvatarRenderState, PlayerModel> parent) {
		super(parent);
	}

	@Override
	public void submit(PoseStack poseStack, SubmitNodeCollector collector, int light, AvatarRenderState state, float yRot, float xRot) {
		PlayerData data = state.getData(AuroraCosmetics.PLAYER);
		if (data == null || data.invisible()) {
			return;
		}
		List<String> ids = CosmeticSync.cosmeticsFor(data.name());
		if (ids.isEmpty()) {
			return;
		}

		// Luz do ambiente (bloco ou céu) para os cosméticos não brilharem no escuro
		int block = (light >> 4) & 0xF;
		int sky = (light >> 20) & 0xF;
		final float brightness = 0.35f + 0.65f * (Math.max(block, sky) / 15f);
		PlayerModel model = this.getParentModel();

		for (String id : ids) {
			Item item = CosmeticRegistry.get(id);
			if (item == null) {
				continue;
			}
			poseStack.pushPose();
			switch (item.bone()) {
				case "head" -> model.head.translateAndRotate(poseStack);
				case "body" -> model.body.translateAndRotate(poseStack);
				default -> {
				}
			}
			for (Part part : item.parts()) {
				poseStack.pushPose();
				applyPart(poseStack, part, data);
				final List<Cube> cubes = part.cubes();
				collector.submitCustomGeometry(poseStack, RenderTypes.debugFilledBox(), (pose, consumer) -> {
					for (Cube c : cubes) {
						drawCube(pose, consumer, c, brightness);
					}
				});
				poseStack.popPose();
			}
			poseStack.popPose();
		}
	}

	/** Pivô → rotação fixa → animação → volta do pivô. As mesmas fórmulas existem no preview do launcher. */
	private static void applyPart(PoseStack poseStack, Part part, PlayerData d) {
		float t = d.age();
		poseStack.translate(part.px(), part.py(), part.pz());
		if (part.rz() != 0) poseStack.rotate(Axis.ZP, part.rz() * DEG);
		if (part.ry() != 0) poseStack.rotate(Axis.YP, part.ry() * DEG);
		if (part.rx() != 0) poseStack.rotate(Axis.XP, part.rx() * DEG);

		switch (part.anim()) {
			case "flapL", "flapR" -> {
				float speed = d.flying() ? 0.55f : 0.18f + Math.min(d.speed() * 2f, 0.3f);
				float amp = d.flying() ? 10f : 22f;
				float base = d.flying() ? 55f : (d.crouching() ? 5f : 0f);
				float angle = base + (float) Math.sin(t * speed) * amp;
				poseStack.rotate(Axis.YP, (part.anim().equals("flapL") ? -angle : angle) * DEG);
			}
			case "spin" -> poseStack.rotate(Axis.YP, (t * 4f % 360f) * DEG);
			case "bob" -> poseStack.translate(0f, (float) Math.sin(t * 0.15f) * 0.04f, 0f);
			case "float" -> {
				poseStack.translate(0f, (float) Math.sin(t * 0.1f) * 0.075f, 0f);
				poseStack.rotate(Axis.YP, (float) Math.sin(t * 0.05f) * 25f * DEG);
			}
			default -> {
			}
		}
		poseStack.translate(-part.px(), -part.py(), -part.pz());
	}

	private static int shade(int argb, float f) {
		int r = Math.min(255, (int) (((argb >> 16) & 0xFF) * f));
		int g = Math.min(255, (int) (((argb >> 8) & 0xFF) * f));
		int b = Math.min(255, (int) ((argb & 0xFF) * f));
		return 0xFF000000 | (r << 16) | (g << 8) | b;
	}

	/** Mesmo formato de quads usado pelo TestRenderUtils da Fabric API (RenderTypes.debugFilledBox). */
	private static void drawCube(PoseStack.Pose pose, VertexConsumer v, Cube c, float light) {
		float x0 = c.x0(), y0 = c.y0(), z0 = c.z0(), x1 = c.x1(), y1 = c.y1(), z1 = c.z1();
		// No espaço do modelo y cresce para baixo: y0 é o topo
		int top = shade(c.argb(), light);
		int side = shade(c.argb(), light * 0.82f);
		int front = shade(c.argb(), light * 0.9f);
		int bottom = shade(c.argb(), light * 0.6f);
		// Frente (z0)
		v.addVertex(pose, x0, y0, z0).setColor(front);
		v.addVertex(pose, x1, y0, z0).setColor(front);
		v.addVertex(pose, x1, y1, z0).setColor(front);
		v.addVertex(pose, x0, y1, z0).setColor(front);
		// Trás (z1)
		v.addVertex(pose, x1, y0, z1).setColor(front);
		v.addVertex(pose, x0, y0, z1).setColor(front);
		v.addVertex(pose, x0, y1, z1).setColor(front);
		v.addVertex(pose, x1, y1, z1).setColor(front);
		// Lado x0
		v.addVertex(pose, x0, y0, z1).setColor(side);
		v.addVertex(pose, x0, y0, z0).setColor(side);
		v.addVertex(pose, x0, y1, z0).setColor(side);
		v.addVertex(pose, x0, y1, z1).setColor(side);
		// Lado x1
		v.addVertex(pose, x1, y0, z0).setColor(side);
		v.addVertex(pose, x1, y0, z1).setColor(side);
		v.addVertex(pose, x1, y1, z1).setColor(side);
		v.addVertex(pose, x1, y1, z0).setColor(side);
		// Topo (y0)
		v.addVertex(pose, x0, y0, z0).setColor(top);
		v.addVertex(pose, x0, y0, z1).setColor(top);
		v.addVertex(pose, x1, y0, z1).setColor(top);
		v.addVertex(pose, x1, y0, z0).setColor(top);
		// Base (y1)
		v.addVertex(pose, x0, y1, z1).setColor(bottom);
		v.addVertex(pose, x0, y1, z0).setColor(bottom);
		v.addVertex(pose, x1, y1, z0).setColor(bottom);
		v.addVertex(pose, x1, y1, z1).setColor(bottom);
	}
}
