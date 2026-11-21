import java.io.PrintStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Collections;
import zombie.Lua.LuaManager;
import zombie.iso.IsoWorld;
import zombie.iso.worldgen.WorldGenChunk;
import zombie.iso.worldgen.WorldGenParams;
import zombie.iso.worldgen.WorldGenReader;
import zombie.iso.worldgen.biomes.Biome;
import se.krka.kahlua.vm.KahluaTable;

/** Oracle for the original WorldGenChunk's constructor and public queries on a booted server.
 * This deliberately does not claim coverage of generateChunks, tile object creation or blending.
 */
public final class WgChunkProbe {
    static final int[] COORDS = {-63, -1, 0, 1, 7, 8, 30, 1574};
    static final String[][] PROTECTED = {
        {}, {"vegetation_*"}, {"fencing_01_?"}, {"natural.02_3"}, {"*"}, {"?"}
    };
    static final String[] TILES = {
        "vegetation_01_0", "fencing_01_1", "natural.02_3", "other", "X"
    };

    static void run(PrintStream out) throws Exception {
        WorldGenChunk chunk = IsoWorld.instance.getWgChunk();
        if (chunk == null) throw new IllegalStateException("original WorldGenChunk not initialized");
        long seed = WorldGenParams.INSTANCE.getSeed();
        out.println("SEED " + seed);
        for (int x : COORDS) for (int y : COORDS) {
            out.println("BIOME " + x + " " + y + " " + chunk.getBiome(x, y).name());
        }
        KahluaTable table = (KahluaTable)LuaManager.env.rawget("worldgen");
        WorldGenReader reader = new WorldGenReader();
        Map<String, Biome> biomes = reader.loadBiomes(table, "biomes_map");
        List<String> filters = new ArrayList<>(biomes.keySet());
        Collections.sort(filters);
        for (String filter : filters) {
            for (int x : new int[]{-1, 0, 8, 1574}) for (int y : new int[]{-1, 0, 8, 1574}) {
                var biome = chunk.getMapBiome(x, y, filter);
                out.println("MAP " + filter + " " + x + " " + y + " " + (biome == null ? "nil" : biome.name()));
            }
        }
        Map<String, Double> priorities = reader.loadPriorities(table, "priorities");
        List<String> keys = new ArrayList<>(priorities.keySet());
        Collections.sort(keys);
        if (keys.size() > 8) keys = keys.subList(0, 8);
        keys.add("__missing__");
        for (String a : keys) for (String b : keys) {
            out.println("PRIORITY " + a + " " + b + " " + (chunk.priority(a, b) ? 1 : 0));
        }
        for (int i = 0; i < PROTECTED.length; i++) {
            List<String> patterns = List.of(PROTECTED[i]);
            for (String tile : TILES) {
                out.println("PROTECT " + i + " " + tile + " " + (chunk.isProtected(patterns, tile) ? 1 : 0));
            }
        }
    }

    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgChunkProbe::run);
    }
}
