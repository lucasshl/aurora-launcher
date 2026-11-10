package dev.aurora.cosmetics;

import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import net.fabricmc.loader.api.FabricLoader;

/**
 * Descobre quais cosméticos cada jogador está usando.
 * 1) config/aurora-cosmetics.json (escrito pelo launcher: o seu próprio jogador, sem depender de internet)
 * 2) servidor de cosméticos: GET {server}/api/cosmetics3d/{nick}, com cache de 5 minutos
 */
public final class CosmeticSync {
	private record Entry(List<String> items, long expiresAt) {
	}

	private static final Map<String, Entry> REMOTE = new ConcurrentHashMap<>();
	private static final Map<String, Boolean> IN_FLIGHT = new ConcurrentHashMap<>();
	private static volatile Map<String, List<String>> local = Map.of();
	private static volatile String server = "";
	private static volatile long configMtime = -1;
	private static long lastConfigCheck = 0;
	private static final HttpClient HTTP = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(5)).build();

	private CosmeticSync() {
	}

	private static Path configPath() {
		return FabricLoader.getInstance().getConfigDir().resolve("aurora-cosmetics.json");
	}

	public static void init() {
		reloadConfig();
	}

	/** Relê o arquivo se o launcher o alterou (checado no máximo a cada 3 s). */
	private static void reloadConfig() {
		try {
			Path p = configPath();
			if (!Files.exists(p)) {
				return;
			}
			long mtime = Files.getLastModifiedTime(p).toMillis();
			if (mtime == configMtime) {
				return;
			}
			configMtime = mtime;
			JsonObject root = JsonParser.parseString(Files.readString(p, StandardCharsets.UTF_8)).getAsJsonObject();
			server = root.has("server") && !root.get("server").isJsonNull() ? root.get("server").getAsString().replaceAll("/+$", "") : "";
			Map<String, List<String>> map = new ConcurrentHashMap<>();
			if (root.has("players")) {
				for (Map.Entry<String, JsonElement> e : root.getAsJsonObject("players").entrySet()) {
					map.put(e.getKey().toLowerCase(Locale.ROOT), toList(e.getValue()));
				}
			}
			local = map;
			REMOTE.clear();
		} catch (Exception e) {
			AuroraCosmetics.LOG.warn("Não foi possível ler aurora-cosmetics.json: {}", e.toString());
		}
	}

	private static List<String> toList(JsonElement el) {
		List<String> out = new ArrayList<>();
		if (el != null && el.isJsonArray()) {
			for (JsonElement i : el.getAsJsonArray()) {
				out.add(i.getAsString());
			}
		}
		return out;
	}

	public static List<String> cosmeticsFor(String name) {
		long now = System.currentTimeMillis();
		if (now - lastConfigCheck > 3000) {
			lastConfigCheck = now;
			reloadConfig();
		}
		String key = name.toLowerCase(Locale.ROOT);
		List<String> mine = local.get(key);
		if (mine != null) {
			return mine;
		}
		Entry e = REMOTE.get(key);
		if ((e == null || e.expiresAt() < now) && !server.isEmpty() && IN_FLIGHT.putIfAbsent(key, Boolean.TRUE) == null) {
			fetch(key);
		}
		return e == null ? List.of() : e.items();
	}

	private static void fetch(String key) {
		String url = server + "/api/cosmetics3d/" + URLEncoder.encode(key, StandardCharsets.UTF_8);
		HttpRequest req = HttpRequest.newBuilder(URI.create(url)).timeout(Duration.ofSeconds(8)).GET().build();
		HTTP.sendAsync(req, HttpResponse.BodyHandlers.ofString()).whenComplete((resp, err) -> {
			try {
				List<String> items = List.of();
				long ttl = 2 * 60_000;
				if (err == null && resp.statusCode() == 200) {
					items = toList(JsonParser.parseString(resp.body()).getAsJsonObject().get("items"));
					ttl = 5 * 60_000;
				}
				REMOTE.put(key, new Entry(List.copyOf(items), System.currentTimeMillis() + ttl));
			} catch (Exception ex) {
				REMOTE.put(key, new Entry(List.of(), System.currentTimeMillis() + 2 * 60_000));
			} finally {
				IN_FLIGHT.remove(key);
			}
		});
	}
}
