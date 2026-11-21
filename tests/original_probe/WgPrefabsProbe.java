import java.io.PrintStream;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoObject;
import zombie.iso.IsoWorld;
import zombie.iso.sprite.IsoSpriteManager;
import zombie.iso.worldgen.ChunksCache;
import zombie.iso.worldgen.PrefabStructure;
import zombie.iso.worldgen.StaticModule;
import zombie.iso.worldgen.WorldGenChunk;
import zombie.iso.worldgen.WorldGenParams;
import zombie.iso.worldgen.utils.ChunkCoord;

/** Public generateChunks with an injected deterministic static-module prefab. */
public final class WgPrefabsProbe {
    static final int BASE_X = 1034;
    static final int BASE_Y = 1458;

    @SuppressWarnings("unchecked")
    static void injectPrefab(WorldGenChunk generator) throws Exception {
        List<String> tiles = List.of(
            "blends_natural_01_0",
            "vegetation_groundcover_01_0",
            "blends_natural_01_64"
        );
        Map<String, int[][]> schematic = new HashMap<>();
        schematic.put("Floor", new int[][]{{1, 3}, {3, 1}});
        schematic.put("FloorFurniture", new int[][]{{0, 0}, {0, 2}});
        schematic.put("FloorOverlay", new int[][]{{2, 0}, {0, 2}});
        schematic.put("Furniture", new int[][]{{0, 2}, {0, 0}});
        PrefabStructure prefab = new PrefabStructure(new int[]{2, 2}, tiles, schematic, 0.0f);
        int xmin = BASE_X * 8;
        int ymin = BASE_Y * 8;
        StaticModule module = new StaticModule(null, prefab, xmin, xmin + 15, ymin, ymin + 15);
        Field field = WorldGenChunk.class.getDeclaredField("staticModules");
        field.setAccessible(true);
        List<StaticModule> modules = (List<StaticModule>)field.get(generator);
        modules.add(0, module);
    }

    static void run(PrintStream out) throws Exception {
        IsoCell cell = IsoWorld.instance.currentCell;
        WorldGenChunk generator = IsoWorld.instance.getWgChunk();
        if (cell == null || generator == null) throw new IllegalStateException("worldgen not initialized");
        cell.setWorldX(0);
        cell.setWorldY(0);
        injectPrefab(generator);
        ChunksCache cache = new ChunksCache();
        for (int x = BASE_X; x < BASE_X + 2; x++) {
            for (int y = BASE_Y; y < BASE_Y + 2; y++) {
                IsoChunk chunk = new IsoChunk(cell);
                chunk.wx = x;
                chunk.wy = y;
                chunk.assignLoadID();
                cache.put(new ChunkCoord(x, y), chunk);
            }
        }
        IsoChunk partial = cache.get(new ChunkCoord(BASE_X, BASE_Y));
        IsoGridSquare sq = IsoGridSquare.getNew(cell, null, BASE_X * 8 + 2, BASE_Y * 8 + 3, 0);
        partial.setSquare(2, 3, 0, sq);
        sq.getObjects().add(new IsoObject(cell, sq, IsoSpriteManager.instance.getSprite("blends_natural_01_0")));
        generator.generateChunks(cache);
        out.println("PREFABS " + BASE_X + " " + BASE_Y + " SEED " + WorldGenParams.INSTANCE.getSeed());
        for (int x = BASE_X; x < BASE_X + 2; x++) {
            for (int y = BASE_Y; y < BASE_Y + 2; y++) {
                WgGenerateProbe.dumpChunk(out, cache.get(new ChunkCoord(x, y)));
            }
        }
    }

    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgPrefabsProbe::run);
    }
}
