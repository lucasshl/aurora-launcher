package dev.aurora.cosmetics;

import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/** Modelos 3D em voxel lidos de assets/aurora_cosmetics/cosmetics.json (gerado por tools/gerar_cosmeticos.py). */
public final class CosmeticRegistry {
	/** Cubo em unidades de bloco (pixel / 16), cor ARGB. */
	public record Cube(float x0, float y0, float z0, float x1, float y1, float z1, int argb) {
	}

	public record Part(float px, float py, float pz, float rx, float ry, float rz, String anim, List<Cube> cubes) {
	}

	public record Item(String id, String slot, String bone, List<Part> parts) {
	}

	private static final Map<String, Item> ITEMS = new HashMap<>();

	private CosmeticRegistry() {
	}

	public static Item get(String id) {
		return ITEMS.get(id);
	}

	public static int size() {
		return ITEMS.size();
	}

	public static void load() {
		ITEMS.clear();
		try (InputStream in = CosmeticRegistry.class.getResourceAsStream("/assets/aurora_cosmetics/cosmetics.json")) {
			if (in == null) {
				AuroraCosmetics.LOG.error("cosmetics.json não encontrado dentro do mod");
				return;
			}
			JsonObject root = JsonParser.parseReader(new InputStreamReader(in, StandardCharsets.UTF_8)).getAsJsonObject();
			for (JsonElement el : root.getAsJsonArray("items")) {
				Item item = parseItem(el.getAsJsonObject());
				ITEMS.put(item.id(), item);
			}
		} catch (Exception e) {
			AuroraCosmetics.LOG.error("Falha ao carregar cosméticos", e);
		}
	}

	private static Item parseItem(JsonObject o) {
		List<Part> parts = new ArrayList<>();
		for (JsonElement pe : o.getAsJsonArray("parts")) {
			JsonObject p = pe.getAsJsonObject();
			float[] pivot = vec(p.getAsJsonArray("pivot"));
			float[] rot = p.has("rot") ? vec(p.getAsJsonArray("rot")) : new float[3];
			String anim = p.has("anim") ? p.get("anim").getAsString() : "none";
			List<Cube> cubes = new ArrayList<>();
			for (JsonElement ce : p.getAsJsonArray("cubes")) {
				JsonObject c = ce.getAsJsonObject();
				float[] f = vec(c.getAsJsonArray("f"));
				float[] t = vec(c.getAsJsonArray("t"));
				int rgb = Integer.parseInt(c.get("c").getAsString().substring(1), 16);
				cubes.add(new Cube(f[0] / 16f, f[1] / 16f, f[2] / 16f, t[0] / 16f, t[1] / 16f, t[2] / 16f, 0xFF000000 | rgb));
			}
			parts.add(new Part(pivot[0] / 16f, pivot[1] / 16f, pivot[2] / 16f, rot[0], rot[1], rot[2], anim, cubes));
		}
		return new Item(o.get("id").getAsString(), o.get("slot").getAsString(), o.get("bone").getAsString(), parts);
	}

	private static float[] vec(JsonArray a) {
		return new float[]{a.get(0).getAsFloat(), a.get(1).getAsFloat(), a.get(2).getAsFloat()};
	}
}
