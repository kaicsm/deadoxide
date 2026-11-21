import java.io.PrintStream;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import se.krka.kahlua.vm.KahluaTable;
import zombie.Lua.LuaManager;
import zombie.iso.worldgen.WorldGenReader;
import zombie.iso.worldgen.WorldGenSimplexGenerator;
import zombie.iso.worldgen.biomes.Biome;
import zombie.iso.worldgen.biomes.BiomeNoise;
import zombie.iso.worldgen.biomes.BiomeRegistry;
import zombie.iso.worldgen.biomes.BiomeType;
import zombie.iso.worldgen.biomes.Feature;
import zombie.iso.worldgen.biomes.FeatureType;
import zombie.iso.worldgen.biomes.Grass;
import zombie.iso.worldgen.biomes.IBiome;
import zombie.iso.worldgen.biomes.TileGroup;
import zombie.iso.worldgen.utils.probabilities.ProbaString;

/** Oracle probe (tools/oracle.sh wgbiomes): the ORIGINAL biome registry the game's own Lua defines. The
 * setup is LootProbe's (LuaGameProbe.init: the client's startup headless, then LoadDirBase of shared and
 * server - the dedicated server's Lua, where media/lua/server/WorldGen/*.lua fills the `worldgen` table),
 * then WorldGenReader over LuaManager.env's worldgen table exactly as WorldGenChunk's constructor reads it
 * (the _override key fallbacks, loadBiomes of both tables, loadSelection of the six parameters,
 * loadPriorities). Printed, in this order (floats as raw bits f%08x, doubles d%016x, strings escaped,
 * lists [a,b], EnumSets in ordinal order; the maps in their own java.util.HashMap iteration order):
 *   KEYS biomes=<k> biomes_map=<k> selection=<k>       which tables the constructor's keys resolved to
 *   BIOMES <n> / B <name> ...                          every biome of the raw loadBiomes map, every field
 *   (per biome: PARENT/GEN/ZOMBIES, GRASS, SETS, FEATURES nil|n + FEAT/F/GF, REPLACEMENTS n + R/RF/GRF,
 *    PLACEMENTS nil|5 + PL, SUBBIOMES n + SB/BB (sub-biomes nested under <name>/<TYPE>/<SUB>/<k>), PROTECTED)
 *   BIOMESGEN <n> [names]                             the generate()-filtered Collectors.toMap map
 *   BIOMESMAP <n> / B ... / BIOMESMAPGEN <n> [names]  the same for biomes_map
 *   SEL <param> <n> / S <param> <NAME> [lo,hi]        the six selection threshold lists
 *   PRIORITIES <n> / P <tile> d<bits>                 the tile priorities
 *   GETBIOME <seed> <x> <y> <name>                    BiomeRegistry.get over the filtered biomes, the
 *                                                     stage-1 simplex (WorldGenSimplexGenerator), 6 seeds
 *                                                     x 8x8 coordinates
 *   GETMAPBIOME <filter> <seed> <x> <y> <name>        the filtered get over biomes_map, each of its names
 *                                                     as the filter, 2 seeds x 4x4 coordinates
 * The feature probability prints getValue()'s float bits (what every consumer reads), `P s` marking the
 * sandbox-option kind (ProbaString). Compared with `pz-dump wgbiomes`.
 * Args: <scratch cache dir> ; run from the game root. */
public class WorldGenBiomesProbe {
    static PrintStream out;

    static final long[] SEEDS = {1L, 42L, -1L, 2147483647L, Long.MIN_VALUE, 0x1234567890ABCDEFL};
    static final int[] CO = {0, 1, 7, 8, 63, 300, 2047, -1};

    static String esc(String s) {
        StringBuilder b = new StringBuilder();
        LuaLoadEnvProbe.esc(b, s);
        return b.toString();
    }

    static String d(double d) {
        return String.format("d%016x", Double.doubleToRawLongBits(d));
    }

    static String f(float f) {
        return String.format("f%08x", Float.floatToRawIntBits(f));
    }

    static String names(List<String> list) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < list.size(); i++) b.append(i == 0 ? "" : ",").append(esc(list.get(i)));
        return b.append(']').toString();
    }

    static String doubles(List<Double> list) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < list.size(); i++) b.append(i == 0 ? "" : ",").append(d(list.get(i)));
        return b.append(']').toString();
    }

    static String sets(EnumSet<?> set) {
        StringBuilder b = new StringBuilder("[");
        boolean first = true;
        for (Object e : set) {
            b.append(first ? "" : ",").append(((Enum<?>) e).name());
            first = false;
        }
        return b.append(']').toString();
    }

    static String p(Feature feat) {
        String v = "P " + (feat.probability() instanceof ProbaString ? "s" : "") + f(feat.probability().getValue());
        return v;
    }

    static void featureLines(String fPrefix, String gPrefix, List<Feature> features) {
        for (int i = 0; i < features.size(); i++) {
            Feature feat = features.get(i);
            out.println(fPrefix + " " + (i + 1) + " MIN " + feat.minSize() + " MAX " + feat.maxSize() + " " + p(feat));
            List<TileGroup> groups = feat.tileGroups();
            for (int j = 0; j < groups.size(); j++) {
                TileGroup g = groups.get(j);
                out.println(gPrefix + " " + (i + 1) + " " + (j + 1) + " " + g.sx() + "x" + g.sy() + " " + names(g.tiles()));
            }
        }
    }

    static void biomeBlock(String id, IBiome b, int depth) {
        out.println("B " + id + " PARENT " + (b.parent() == null ? "nil" : esc(b.parent())) + " GEN " + (b.generate() ? 1 : 0) + " ZOMBIES " + f(b.zombies()));
        if (b.grass() == null) {
            out.println("B " + id + " GRASS nil");
        } else {
            Grass g = b.grass();
            out.println("B " + id + " GRASS fern=" + f(g.fernChance()) + " div=" + f(g.noGrassDiv()) + " noGrass=" + doubles(g.noGrassStages()) + " grass=" + doubles(g.grassStages()));
        }
        out.println("B " + id + " SETS landscape=" + sets(b.landscape()) + " plant=" + sets(b.plant()) + " bush=" + sets(b.bush()) + " temperature=" + sets(b.temperature()) + " hygrometry=" + sets(b.hygrometry()) + " ore=" + sets(b.oreLevel()));
        if (b.getFeatures() == null) {
            out.println("B " + id + " FEATURES nil");
        } else {
            int present = 0;
            for (FeatureType t : FeatureType.values()) if (b.getFeatures().get(t) != null) present++;
            out.println("B " + id + " FEATURES " + present);
            for (FeatureType t : FeatureType.values()) {
                List<Feature> list = b.getFeatures().get(t);
                if (list == null) continue;
                out.println("B " + id + " FEAT " + t + " " + list.size());
                featureLines("F " + id + " " + t, "GF " + id + " " + t, list);
            }
        }
        if (b.getReplacements() == null) {
            out.println("B " + id + " REPLACEMENTS nil");
        } else {
            out.println("B " + id + " REPLACEMENTS " + b.getReplacements().size());
            for (Map.Entry<String, List<Feature>> e : b.getReplacements().entrySet()) {
                out.println("R " + id + " " + esc(e.getKey()) + " " + e.getValue().size());
                featureLines("RF " + id + " " + esc(e.getKey()), "GRF " + id + " " + esc(e.getKey()), e.getValue());
            }
        }
        if (b.placements() == null) {
            out.println("B " + id + " PLACEMENTS nil");
        } else {
            out.println("B " + id + " PLACEMENTS 5");
            for (FeatureType t : FeatureType.values()) {
                out.println("PL " + id + " " + t + " " + names(b.placements().get(t)));
            }
        }
        out.println("B " + id + " SUBBIOMES " + b.subBiomes().size());
        if (depth < 8) {
            for (FeatureType t : FeatureType.values()) {
                Map<FeatureType, List<IBiome>> inner = b.subBiomes().get(t);
                if (inner == null) continue;
                for (FeatureType st : FeatureType.values()) {
                    List<IBiome> list = inner.get(st);
                    if (list == null) continue;
                    out.println("SB " + id + " " + t + " " + st + " " + list.size());
                    for (int k = 0; k < list.size(); k++) {
                        String path = id + "/" + t + "/" + st + "/" + (k + 1);
                        out.println("BB " + path + " " + esc(list.get(k).name()));
                        biomeBlock(path, list.get(k), depth + 1);
                    }
                }
            }
        }
        out.println("B " + id + " PROTECTED " + names(b.protectedList()));
    }

    /** The raw map, every biome block, then the generate()-filtered Collectors.toMap map (what
     * WorldGenChunk's constructor keeps) and its iteration order; returns the filtered map. */
    static Map<String, Biome> dumpMap(String header, Map<String, Biome> raw) {
        out.println(header + " " + raw.size());
        for (Map.Entry<String, Biome> e : raw.entrySet()) biomeBlock(e.getKey(), e.getValue(), 0);
        Map<String, Biome> kept = raw.entrySet().stream().filter(e -> e.getValue().generate()).collect(Collectors.toMap(Map.Entry::getKey, Map.Entry::getValue));
        StringBuilder names = new StringBuilder("[");
        boolean first = true;
        for (String n : kept.keySet()) {
            names.append(first ? "" : ",").append(esc(n));
            first = false;
        }
        out.println(header + "GEN " + kept.size() + " " + names.append(']'));
        return kept;
    }

    static <T extends Enum<T>> void selectionLines(String key, Map<T, List<Double>> sel) {
        out.println("SEL " + key + " " + sel.size());
        List<T> ordered = new ArrayList<>(sel.keySet());
        ordered.sort((a, b) -> Integer.compare(a.ordinal(), b.ordinal()));
        for (T v : ordered) {
            out.println("S " + key + " " + v + " " + doubles(sel.get(v)));
        }
    }

    public static void main(String[] a) throws Exception {
        out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        LuaGameProbe.init(new String[]{a[0], "shared", "server"});
        KahluaTable worldgen = (KahluaTable)LuaManager.env.rawget("worldgen");
        if (worldgen == null) {
            System.err.println("ORACLE: worldgen table missing from the Lua environment");
            Runtime.getRuntime().halt(4);
        }
        WorldGenReader r = new WorldGenReader();
        // WorldGenChunk.<init>'s table keys
        String biomesKey = worldgen.rawget("biomes_override") == null ? "biomes" : "biomes_override";
        String biomesMapKey = "biomes_map";
        String selectionKey = worldgen.rawget("selection_override") == null ? "selection" : "selection_override";
        out.println("KEYS biomes=" + biomesKey + " biomes_map=" + biomesMapKey + " selection=" + selectionKey);

        Map<String, Biome> biomes = dumpMap("BIOMES", r.loadBiomes(worldgen, biomesKey));
        Map<String, Biome> biomesMap = dumpMap("BIOMESMAP", r.loadBiomes(worldgen, biomesMapKey));

        selectionLines("landscape", r.loadSelection(BiomeType.Landscape.NONE, worldgen, selectionKey));
        selectionLines("plant", r.loadSelection(BiomeType.Plant.NONE, worldgen, selectionKey));
        selectionLines("bush", r.loadSelection(BiomeType.Bush.NONE, worldgen, selectionKey));
        selectionLines("temperature", r.loadSelection(BiomeType.Temperature.NONE, worldgen, selectionKey));
        selectionLines("hygrometry", r.loadSelection(BiomeType.Hygrometry.NONE, worldgen, selectionKey));
        selectionLines("ore_level", r.loadSelection(BiomeType.OreLevel.NONE, worldgen, selectionKey));

        Map<String, Double> priorities = r.loadPriorities(worldgen, "priorities");
        out.println("PRIORITIES " + priorities.size());
        for (Map.Entry<String, Double> e : priorities.entrySet()) {
            out.println("P " + esc(e.getKey()) + " " + d(e.getValue()));
        }

        Map<BiomeType.Landscape, List<Double>> landscape = r.loadSelection(BiomeType.Landscape.NONE, worldgen, selectionKey);
        Map<BiomeType.Plant, List<Double>> plant = r.loadSelection(BiomeType.Plant.NONE, worldgen, selectionKey);
        Map<BiomeType.Bush, List<Double>> bush = r.loadSelection(BiomeType.Bush.NONE, worldgen, selectionKey);
        Map<BiomeType.Temperature, List<Double>> temperature = r.loadSelection(BiomeType.Temperature.NONE, worldgen, selectionKey);
        Map<BiomeType.Hygrometry, List<Double>> hygrometry = r.loadSelection(BiomeType.Hygrometry.NONE, worldgen, selectionKey);
        Map<BiomeType.OreLevel, List<Double>> oreLevel = r.loadSelection(BiomeType.OreLevel.NONE, worldgen, selectionKey);

        for (long seed : SEEDS) {
            WorldGenSimplexGenerator g = new WorldGenSimplexGenerator(seed);
            for (int x : CO) {
                for (int y : CO) {
                    BiomeNoise n = g.noise(x, y);
                    IBiome pick = BiomeRegistry.instance.get(biomes, n, g.selector(x, y),
                        landscape, plant, bush, temperature, hygrometry, oreLevel);
                    out.println("GETBIOME " + seed + " " + x + " " + y + " " + pick.name());
                }
            }
        }
        List<String> filters = new ArrayList<>(biomesMap.keySet());
        for (String filter : filters) {
            for (int si = 0; si < 2; si++) {
                long seed = SEEDS[si];
                WorldGenSimplexGenerator g = new WorldGenSimplexGenerator(seed);
                for (int ci = 0; ci < CO.length; ci += 2) {
                    for (int cj = 0; cj < CO.length; cj += 2) {
                        int x = CO[ci], y = CO[cj];
                        BiomeNoise n = g.noise(x, y);
                        IBiome pick = BiomeRegistry.instance.get(biomesMap, filter, n, g.selector(x, y),
                            bush, oreLevel);
                        out.println("GETMAPBIOME " + esc(filter) + " " + seed + " " + x + " " + y + " " + (pick == null ? "null" : pick.name()));
                    }
                }
            }
        }
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
