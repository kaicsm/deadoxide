import java.awt.image.BufferedImage;
import java.io.File;
import java.io.PrintStream;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import javax.imageio.ImageIO;
import se.krka.kahlua.vm.KahluaTable;
import zombie.ZomboidFileSystem;
import zombie.Lua.LuaManager;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoObject;
import zombie.iso.IsoWorld;
import zombie.iso.enums.ChunkGenerationStatus;
import zombie.iso.sprite.IsoSprite;
import zombie.iso.sprite.IsoSpriteManager;
import zombie.iso.worldgen.ChunksCache;
import zombie.iso.worldgen.WorldGenChunk;
import zombie.iso.worldgen.WorldGenParams;
import zombie.iso.worldgen.WorldGenReader;
import zombie.iso.worldgen.blending.BlendDirection;
import zombie.iso.worldgen.maps.BiomeMap;
import zombie.iso.worldgen.maps.BiomeMapEntry;
import zombie.iso.worldgen.utils.ChunkCoord;

/**
 * Compare WorldGenChunk.genMapChunk through the public generateChunks path.
 * Real tile definitions and the original WorldGen Lua are used, with a temporary PNG
 * indexed through ZomboidFileSystem.activeFileMap. No installation media is modified.
 */
public final class WgMapChunkProbe {
    static final int BASE_X = 3072;
    static final int BASE_Y = 4096;
    static final String MAP_NAME = "oracle_mapchunk";
    static final String FLOOR_SPRITE = "blends_natural_01_0";
    static final String PLANT_SPRITE = "vegetation_groundcover_01_0";
    static File rasterFile;

    static int[] selectPixels(WorldGenChunk generator, Map<Integer, BiomeMapEntry> entries) {
        List<Integer> available = new ArrayList<>();
        for (var entry : entries.entrySet()) {
            int pixel = entry.getKey();
            String biome = entry.getValue().biome();
            if (pixel >= 0 && pixel <= 255 && biome != null && !biome.equals("$random")) {
                available.add(pixel);
            }
        }
        Collections.sort(available);
        if (available.isEmpty()) {
            throw new IllegalStateException("biome_map_config contains no mapped biome pixels");
        }
        int first = -1;
        int second = -1;
        // Select only entries for which the real WorldGenChunk has a mapped biome.
        for (int pixel : available) {
            String name = entries.get(pixel).biome();
            if (generator.getMapBiome(BASE_X * 8, BASE_Y * 8, name) != null) {
                if (first < 0) first = pixel;
                else if (second < 0) { second = pixel; break; }
            }
        }
        if (first < 0) throw new IllegalStateException("no configured pixel resolves to a mapped biome");
        if (second < 0) second = first;
        int unknown = -1;
        for (int i = 255; i >= 0; i--) {
            if (!entries.containsKey(i)) { unknown = i; break; }
        }
        if (unknown < 0) throw new IllegalStateException("no free PNG palette index for missing biome");
        return new int[]{first, second, unknown};
    }

    static int red(int localX, int localY, int[] pixel) {
        if (localX < 8 && localY < 8) return pixel[0];
        if (localX >= 8 && localY < 8) return pixel[1];
        if (localX < 8) return pixel[2];
        return ((localX + localY) & 1) == 0 ? pixel[0] : pixel[1];
    }

    static int green(int localX, int localY) {
        return 13 + (localX + 2 * localY) % 200;
    }

    static void writeRaster(int[] pixel) throws Exception {
        if (!rasterFile.getParentFile().mkdirs() && !rasterFile.getParentFile().isDirectory()) {
            throw new java.io.IOException("cannot create raster directory");
        }
        BufferedImage image = new BufferedImage(256, 256, BufferedImage.TYPE_INT_ARGB);
        for (int y = 0; y < 256; y++) {
            for (int x = 0; x < 256; x++) {
                int lx = x & 15;
                int ly = y & 15;
                int color = (255 << 24) | (red(lx, ly, pixel) << 16) | (green(lx, ly) << 8);
                image.setRGB(x, y, color);
            }
        }
        if (!ImageIO.write(image, "png", rasterFile)) throw new java.io.IOException("PNG encoder unavailable");
    }

    static void dumpChunk(PrintStream out, IsoChunk chunk) {
        out.println("CHUNK " + chunk.wx + " " + chunk.wy +
            " LEVELS " + chunk.getMinLevel() + " " + chunk.getMaxLevel() +
            " WORLDGEN " + (chunk.isModded().contains(ChunkGenerationStatus.WORLDGEN) ? 1 : 0) +
            " BLEND " + (chunk.isBlendingDoneFull() ? 1 : 0) + " " + (chunk.isBlendingDonePartial() ? 1 : 0) +
            " DEPTH " + chunk.getModifDepth(BlendDirection.NORTH) +
            " " + chunk.getModifDepth(BlendDirection.SOUTH) +
            " " + chunk.getModifDepth(BlendDirection.WEST) +
            " " + chunk.getModifDepth(BlendDirection.EAST) +
            " ATTACH " + (chunk.isAttachmentsDoneFull() ? 1 : 0) +
            " " + (chunk.isAttachmentsDone(0) ? 1 : 0) +
            " " + (chunk.isAttachmentsDone(1) ? 1 : 0) +
            " " + (chunk.isAttachmentsDone(2) ? 1 : 0) +
            " " + (chunk.isAttachmentsDone(3) ? 1 : 0) +
            " " + (chunk.isAttachmentsDone(4) ? 1 : 0) +
            " PARTIAL " + chunk.attachmentsPartialSize() +
            " ZOMBIES " + chunk.proceduralZombieSquares.size());
        for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) {
            IsoGridSquare square = chunk.getGridSquare(x, y, 0);
            StringBuilder line = new StringBuilder("S ").append(chunk.wx).append(' ')
                .append(chunk.wy).append(' ').append(x).append(' ').append(y);
            if (square == null) {
                line.append(" null");
            } else {
                line.append(" ROOM ").append(square.getRoomID())
                    .append(" OBJECTS ").append(square.getObjects().size());
                for (int i = 0; i < square.getObjects().size(); i++) {
                    IsoObject object = square.getObjects().get(i);
                    String sprite = object.getSprite() == null ? "null" :
                        String.valueOf(object.getSprite().getName());
                    line.append(" | ").append(object.getClass().getSimpleName()).append(' ').append(sprite);
                }
            }
            out.println(line);
        }
        for (IsoGridSquare square : chunk.proceduralZombieSquares) {
            out.println("Z " + chunk.wx + " " + chunk.wy + " " + square.x + " " + square.y + " " + square.z);
        }
    }

    static void run(PrintStream out) throws Exception {
        IsoCell cell = IsoWorld.instance.currentCell;
        WorldGenChunk generator = IsoWorld.instance.getWgChunk();
        if (cell == null || generator == null) throw new IllegalStateException("server worldgen is not initialized");
        KahluaTable table = (KahluaTable)LuaManager.env.rawget("biome_map_config");
        if (table == null) throw new IllegalStateException("biome_map_config missing");
        Map<Integer, BiomeMapEntry> entries = new WorldGenReader().loadBiomeMapConfig(table);
        int[] pixels = selectPixels(generator, entries);
        writeRaster(pixels);
        String relative = String.format("media/maps/%s/maps/biomemap_%d_%d.png",
            MAP_NAME, BASE_X / 32, BASE_Y / 32);
        ZomboidFileSystem.instance.activeFileMap.put(relative.toLowerCase(Locale.ENGLISH), rasterFile.getAbsolutePath());
        IsoWorld.instance.setMap(MAP_NAME);
        BiomeMap map = new BiomeMap();
        IsoWorld.instance.setBiomeMap(map);
        cell.setWorldX(0);
        cell.setWorldY(0);

        ChunksCache cache = new ChunksCache();
        IsoSprite floorSprite = IsoSpriteManager.instance.getSprite(FLOOR_SPRITE);
        IsoSprite plantSprite = IsoSpriteManager.instance.getSprite(PLANT_SPRITE);
        for (int wx = BASE_X; wx < BASE_X + 2; wx++) {
            for (int wy = BASE_Y; wy < BASE_Y + 2; wy++) {
                IsoChunk chunk = new IsoChunk(cell);
                chunk.wx = wx;
                chunk.wy = wy;
                chunk.assignLoadID();
                chunk.setMinMaxLevel(0, 0);
                for (int x = 0; x < 8; x++) {
                    for (int y = 0; y < 8; y++) {
                        IsoGridSquare square = IsoGridSquare.getNew(cell, null, wx * 8 + x, wy * 8 + y, 0);
                        chunk.setSquare(x, y, 0, square);
                        square.getObjects().add(new IsoObject(cell, square, floorSprite));
                        if (x == 1 && y == 1) {
                            square.getObjects().add(new IsoObject(cell, square, plantSprite));
                        }
                        if (square.getFloor() == null) {
                            throw new IllegalStateException("fixture floor lacks solidfloor: " + FLOOR_SPRITE);
                        }
                    }
                }
                if (chunk.hasEmptySquaresOnLevelZero()) throw new IllegalStateException("fixture selected random chunk");
                cache.put(new ChunkCoord(wx, wy), chunk);
            }
        }
        int[] sampled = map.getZones(BASE_X, BASE_Y, BiomeMap.Type.BIOME);
        if (sampled == null || sampled.length != 64 || sampled[0] != pixels[0]) {
            throw new IllegalStateException("the original Java BiomeMap did not load the fixture PNG");
        }
        generator.generateChunks(cache);
        out.println("MAPGEN " + BASE_X + " " + BASE_Y + " SEED " + WorldGenParams.INSTANCE.getSeed());
        out.println("PIXELS " + pixels[0] + " " + pixels[1] + " " + pixels[2] +
            " BIOMES " + entries.get(pixels[0]).biome() + " " + entries.get(pixels[1]).biome());
        for (int wx = BASE_X; wx < BASE_X + 2; wx++) {
            for (int wy = BASE_Y; wy < BASE_Y + 2; wy++) {
                dumpChunk(out, cache.get(new ChunkCoord(wx, wy)));
            }
        }
    }

    public static void main(String[] args) throws Exception {
        rasterFile = new File(args[0], "mapchunk-fixtures/biomemap_" + BASE_X / 32 + "_" + BASE_Y / 32 + ".png");
        ServerHarness.run(args[0], WgMapChunkProbe::run);
    }
}
