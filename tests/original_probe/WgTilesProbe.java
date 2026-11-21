import java.io.PrintStream;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.worldgen.ChunksCache;
import zombie.iso.worldgen.WorldGenTile;
import zombie.iso.worldgen.biomes.Biome;
import zombie.iso.worldgen.biomes.Feature;
import zombie.iso.worldgen.biomes.FeatureType;
import zombie.iso.worldgen.biomes.TileGroup;
import zombie.iso.worldgen.enums.TileLookupRetValue;
import zombie.iso.worldgen.utils.probabilities.ProbaDouble;

/** Direct Java-to-Rust oracle for WorldGenTile's deterministic lookup and selection logic.
 * No sprites are constructed or changed: all lookup cases stop before applyTile.
 * A private method is invoked only to observe the real Java getBiomeTile result and its
 * shared 16x16 pending array, not a rewritten implementation of its algorithm.
 */
public final class WgTilesProbe {
    static PrintStream out;
    static final String[] CASES = {
        "no-features", "no-specific", "empty-list", "filtered", "no-groups",
        "too-big", "cant-place", "already-placed", "single", "reserved",
        "any", "existing", "zero-proba", "two-by-two"
    };

    static Feature feature(double p, int min, TileGroup... groups) {
        return new Feature(List.of(groups), min, 8, new ProbaDouble(p));
    }

    static TileGroup group(int sx, int sy, String... names) {
        return new TileGroup(sx, sy, List.of(names));
    }

    static Biome biome(Map<FeatureType, List<Feature>> features, Map<FeatureType, List<String>> placements) {
        return new Biome("ORACLE", null, true, features, null,
            null, null, null, null, null, null, 0.0f, placements, null, List.of(), null);
    }

    static Biome lookupBiome(String name) {
        Map<FeatureType, List<Feature>> features = name.equals("no-features") ? null : new EnumMap<>(FeatureType.class);
        if (features != null && !name.equals("no-specific")) {
            List<Feature> list = new ArrayList<>();
            if (!name.equals("empty-list")) {
                int min = name.equals("filtered") ? 9 : 1;
                TileGroup tile;
                if (name.equals("no-groups")) {
                    list.add(feature(1.0, min));
                } else {
                    switch (name) {
                        case "too-big", "cant-place", "already-placed", "two-by-two" ->
                            tile = group(2, 2, "a", "b", "c", "d");
                        case "reserved" -> tile = group(1, 1, "$no_tree");
                        case "any" -> tile = group(1, 1, "$any");
                        default -> tile = group(1, 1, "single_tile");
                    }
                    list.add(feature(name.equals("zero-proba") ? 0.0 : 1.0, min, tile));
                }
            }
            features.put(FeatureType.GROUND, list);
        }
        Map<FeatureType, List<String>> placements = null;
        if (name.equals("cant-place")) {
            placements = new EnumMap<>(FeatureType.class);
            placements.put(FeatureType.GROUND, List.of("*"));
        }
        return biome(features, placements);
    }

    static String pending(String[] a) {
        StringBuilder b = new StringBuilder("[");
        boolean first = true;
        for (int y = 0; y < 16; y++) for (int x = 0; x < 16; x++) {
            String s = a[x + y * 16];
            if (s != null) {
                if (!first) b.append(',');
                b.append(x).append(':').append(y).append('=').append(s);
                first = false;
            }
        }
        return b.append(']').toString();
    }

    static String tile(TileGroup group) {
        return group == null ? "nil" : group.sx() + "x" + group.sy() + ":" + String.join(",", group.tiles());
    }

    static void lookupCases(WorldGenTile wg) throws Exception {
        Method method = WorldGenTile.class.getDeclaredMethod("getBiomeTile", zombie.iso.worldgen.biomes.IBiome.class,
            FeatureType.class, ChunksCache.class, IsoChunk.class, int.class, int.class, int.class,
            int.class, int.class, int.class, EnumMap.class, Random.class, int.class);
        method.setAccessible(true);
        IsoChunk chunk = new IsoChunk((IsoCell)null);
        chunk.wx = 0;
        chunk.wy = 0;
        for (int i = 0; i < CASES.length; i++) {
            String name = CASES[i];
            int x = name.equals("too-big") ? 15 : 1;
            int y = 1;
            EnumMap<FeatureType, String[]> pending = new EnumMap<>(FeatureType.class);
            for (FeatureType type : FeatureType.values()) pending.put(type, new String[256]);
            if (name.equals("already-placed")) pending.get(FeatureType.GROUND)[(x + 1) + (y + 1) * 16] = "occupied";
            if (name.equals("existing")) pending.get(FeatureType.GROUND)[x + y * 16] = "existing_tile";
            Random rnd = new Random(9000L + i);
            TileLookupRetValue result = (TileLookupRetValue)method.invoke(wg,
                lookupBiome(name), FeatureType.GROUND, new ChunksCache(), chunk, x, y, 0,
                x, y, 0, pending, rnd, 8);
            out.println("LOOKUP " + name + " " + result.tileLookup() + " " +
                (result.tile() == null ? "nil" : result.tile()) + " " +
                pending(pending.get(FeatureType.GROUND)) + " NEXT " + Long.toHexString(rnd.nextLong()));
        }
    }

    static void selectionCases(WorldGenTile wg) {
        List<Feature> features = List.of(
            feature(0.2, 1, group(1, 1, "a", "b")),
            feature(0.3, 1, group(1, 1, "c")),
            feature(0.5, 1, group(1, 1, "d")));
        for (int mode = 0; mode < 4; mode++) {
            for (int i = 0; i < 24; i++) {
                Random rnd = new Random(1000L + 100L * mode + i);
                List<Feature> candidates = mode == 1 ? features.subList(0, 2) : mode == 2 ? List.of() : features;
                float pre = mode == 1 ? 1.0f : mode == 3 ? 0.0f : 1.0f;
                float post = mode == 1 ? 0.5f : mode == 3 ? 0.0f : 1.0f;
                Feature found = wg.findFeature(candidates, pre, post, rnd);
                out.println("FIND " + mode + " " + i + " " +
                    (found == null ? -1 : candidates.indexOf(found)) + " NEXT " + Long.toHexString(rnd.nextLong()));
            }
        }
        Map<FeatureType, List<Feature>> fm = new EnumMap<>(FeatureType.class);
        fm.put(FeatureType.GROUND, List.of(
            feature(0.75, 1, group(1, 1, "g0"), group(1, 1, "g1")),
            feature(0.25, 1, group(1, 1, "g2"), group(1, 1, "g3"))));
        fm.put(FeatureType.PLANT, List.of(
            feature(0.25, 1, group(1, 1, "p0"), group(1, 1, "p1")),
            feature(0.8, 1, group(1, 1, "p2"), group(1, 1, "p3"))));
        Biome biome = biome(fm, null);
        for (int i = 0; i < 40; i++) {
            Random groundRnd = new Random(2200L + i);
            out.println("GROUND " + i + " " + tile(wg.getGround(biome, groundRnd)) +
                " NEXT " + Long.toHexString(groundRnd.nextLong()));
            Random plantRnd = new Random(3200L + i);
            out.println("PLANT " + i + " " + tile(wg.getPlant(biome, plantRnd)) +
                " NEXT " + Long.toHexString(plantRnd.nextLong()));
        }
    }

    public static void main(String[] args) throws Exception {
        out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        WorldGenTile wg = new WorldGenTile();
        selectionCases(wg);
        lookupCases(wg);
        out.flush();
    }
}
