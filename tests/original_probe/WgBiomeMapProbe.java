import java.awt.image.BufferedImage;
import java.io.File;
import java.io.PrintStream;
import java.util.Locale;
import javax.imageio.ImageIO;
import zombie.ZomboidFileSystem;
import zombie.iso.IsoWorld;
import zombie.iso.worldgen.maps.BiomeMap;

/**
 * Differential oracle for BiomeMap's automatic PNG loading.
 *
 * Scenarios cover active-map search order, band extraction, cached null retention until
 * eviction from CacheMap(16), and the rule that a corrupt first PNG does not fall through
 * to a later world map. Files are written into the probe scratch folder and injected into
 * ZomboidFileSystem.instance.activeFileMap, so no installation files are touched.
 */
public final class WgBiomeMapProbe {
    static File fixtureRoot;

    static String relativePath(String map, int cellX, int cellY) {
        return String.format("media/maps/%s/maps/biomemap_%d_%d.png", map, cellX, cellY);
    }

    static File fixtureFile(String map, int cellX, int cellY) {
        return new File(new File(fixtureRoot, map), String.format("biomemap_%d_%d.png", cellX, cellY));
    }

    static void register(String map, int cellX, int cellY, File file) {
        ZomboidFileSystem.instance.activeFileMap.put(relativePath(map, cellX, cellY).toLowerCase(Locale.ENGLISH), file.getAbsolutePath());
    }

    static void writeRaster(File file, int baseRed, int baseGreen, int blockRed, int blockGreen) throws Exception {
        File parent = file.getParentFile();
        if (parent != null && !parent.mkdirs() && !parent.isDirectory()) {
            throw new java.io.IOException("cannot create " + parent);
        }
        BufferedImage image = new BufferedImage(256, 256, BufferedImage.TYPE_INT_ARGB);
        int base = (255 << 24) | (baseRed << 16) | (baseGreen << 8);
        int block = (255 << 24) | (blockRed << 16) | (blockGreen << 8);
        for (int y = 0; y < 256; y++) {
            for (int x = 0; x < 256; x++) {
                image.setRGB(x, y, base);
            }
        }
        for (int y = 0; y < 8; y++) {
            for (int x = 8; x < 16; x++) {
                image.setRGB(x, y, block);
            }
        }
        ImageIO.write(image, "png", file);
    }

    static void writeCorrupt(File file) throws Exception {
        File parent = file.getParentFile();
        if (parent != null && !parent.mkdirs() && !parent.isDirectory()) {
            throw new java.io.IOException("cannot create " + parent);
        }
        try (java.io.FileOutputStream out = new java.io.FileOutputStream(file)) {
            out.write(new byte[]{'n', 'o', 't', '-', 'p', 'n', 'g'});
        }
    }

    static String summary(int[] values) {
        if (values == null) return "nil";
        if (values.length == 0) return "0 0 0 0 0 0";
        long sum = 0L;
        int hash = 0;
        for (int value : values) {
            sum += value;
            hash = hash * 31 + value;
        }
        return values.length + " " + values[0] + " " + values[values.length / 2] + " " + values[values.length - 1] + " " + sum + " " + hash;
    }

    static String corruptResult(BiomeMap map, BiomeMap.Type type) {
        try {
            return summary(map.getCell(0, 0, type));
        } catch (ArrayIndexOutOfBoundsException e) {
            // ImageData(String) silently returns width/height -1 for an invalid PNG;
            // the exception occurs only when BiomeRaster.getSamples is called.
            return "THROW " + e.getClass().getName() + ": " + e.getMessage();
        }
    }

    static void run(PrintStream out) throws Exception {
        if (IsoWorld.instance == null || IsoWorld.instance.currentCell == null) {
            throw new IllegalStateException("original server world is not initialized");
        }
        if (!fixtureRoot.mkdirs() && !fixtureRoot.isDirectory()) {
            throw new java.io.IOException("cannot create " + fixtureRoot);
        }

        File first = fixtureFile("oracle_a", 1, 2);
        writeRaster(first, 17, 201, 18, 202);
        register("oracle_a", 1, 2, first);
        File second = fixtureFile("oracle_b", 1, 2);
        writeRaster(second, 91, 45, 92, 46);
        register("oracle_b", 1, 2, second);
        IsoWorld.instance.setMap("oracle_missing;oracle_a;oracle_b");
        BiomeMap firstMap = new BiomeMap();
        out.println("CASE FIRST");
        out.println("CELL BIOME " + summary(firstMap.getCell(1, 2, BiomeMap.Type.BIOME)));
        out.println("CELL ZONE " + summary(firstMap.getCell(1, 2, BiomeMap.Type.ZONE)));
        out.println("ZONES BIOME " + summary(firstMap.getZones(33, 64, BiomeMap.Type.BIOME)));
        out.println("ZONES ZONE " + summary(firstMap.getZones(33, 64, BiomeMap.Type.ZONE)));

        File active = fixtureFile("oracle_override", 0, 0);
        writeRaster(active, 55, 66, 77, 88);
        register("oracle_override", 0, 0, active);
        IsoWorld.instance.setMap("oracle_override");
        BiomeMap activeMap = new BiomeMap();
        out.println("CASE ACTIVE");
        out.println("CELL BIOME " + summary(activeMap.getCell(0, 0, BiomeMap.Type.BIOME)));
        out.println("CELL ZONE " + summary(activeMap.getCell(0, 0, BiomeMap.Type.ZONE)));

        File cacheFile = fixtureFile("oracle_cache", 9, 9);
        cacheFile.delete();
        register("oracle_cache", 9, 9, cacheFile);
        IsoWorld.instance.setMap("oracle_cache");
        BiomeMap cacheMap = new BiomeMap();
        out.println("CASE CACHE");
        out.println("MISS0 " + summary(cacheMap.getCell(9, 9, BiomeMap.Type.BIOME)));
        writeRaster(cacheFile, 70, 80, 90, 100);
        out.println("MISS1 " + summary(cacheMap.getCell(9, 9, BiomeMap.Type.BIOME)));
        for (int i = 0; i < 17; i++) {
            cacheMap.getCell(100 + i, i, BiomeMap.Type.BIOME);
        }
        out.println("MISS2 " + summary(cacheMap.getCell(9, 9, BiomeMap.Type.BIOME)));

        File corrupt = fixtureFile("oracle_corrupt", 0, 0);
        writeCorrupt(corrupt);
        register("oracle_corrupt", 0, 0, corrupt);
        File fallback = fixtureFile("oracle_fallback", 0, 0);
        writeRaster(fallback, 13, 14, 15, 16);
        register("oracle_fallback", 0, 0, fallback);
        IsoWorld.instance.setMap("oracle_corrupt;oracle_fallback");
        BiomeMap corruptMap = new BiomeMap();
        out.println("CASE CORRUPT");
        out.println("CELL BIOME " + corruptResult(corruptMap, BiomeMap.Type.BIOME));
        out.println("CELL ZONE " + corruptResult(corruptMap, BiomeMap.Type.ZONE));

        out.println("CASE NEGATIVE");
        out.println("WEST " + summary(firstMap.getZones(-1, 64, BiomeMap.Type.BIOME)));
        out.println("NORTH " + summary(firstMap.getZones(33, -1, BiomeMap.Type.ZONE)));
    }

    public static void main(String[] args) throws Exception {
        fixtureRoot = new File(args[0], "biomemap-fixtures");
        ServerHarness.run(args[0], WgBiomeMapProbe::run);
    }
}
