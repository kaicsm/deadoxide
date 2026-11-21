import java.io.PrintStream;
import java.util.List;
import java.util.Map;
import java.util.Random;
import se.krka.kahlua.vm.KahluaTable;
import zombie.Lua.LuaManager;
import zombie.iso.worldgen.WorldGenParams;
import zombie.iso.worldgen.veins.OreVein;
import zombie.iso.worldgen.veins.OreVeinConfig;
import zombie.iso.worldgen.veins.Veins;

/** Oracle probe (tools/oracle.sh wgveins): the ORIGINAL ore veins - Veins.get over the game's own
 * `worldgen.veins` config (the wgbiomes Lua setup: the client's startup headless, then LoadDirBase of shared
 * and server) with WorldGenParams seeds set through setSeedString (the seed is the string's hashCode on both
 * sides), the WorldGenParams.getRandom chain, OreVein's construction draws and isValid. Printed, in this
 * order (floats as raw bits f%08x, doubles d%016x; the config map in its java.util.HashMap iteration order):
 *   CFG n / C <name> cr cp amin amax dmin dmax delta ap p   the vein config (as wgconfigs prints it)
 *   SEED s=<string> hash=<hashCode> seed=<int-as-long>     WorldGenParams.setSeedString + getSeed
 *   RND wx wy offset <6 draws>                             getRandom(wx, wy, offset)'s first draws:
 *                                                          nextInt(100), nextFloat, nextInt(256),
 *                                                          nextLong, nextFloat, nextInt(100)
 *   For each seed of {"1", "42", "oracle"} and each cell of {(0,0), (0,1), (1,0), (-1,-1), (37,-11)}:
 *   CELL seed x y n                                        Veins.get (the config values() order)
 *   OV seed x y i cx cy sx sy spx spy amount cprob aprob cr
 *   ARM seed x y i j orient dist ex ey                     every arm
 *   OVT seed x y i <tiles count>                           the vein's tile groups
 *   VAL seed x y <64 bits>                                 isValid over the 8x8 square grid at the vein's
 *                                                          start cell, each point its own Random(1000 + x +
 *                                                          y * 8), printed 0/1
 *   CACHE seed x y same=<0|1>                              the second get returns the same List instance
 * Compared with `pz-dump wgveins`.
 * Args: <scratch cache dir> ; run from the game root. */
public class WgVeinsProbe {
    static PrintStream out;

    static final String[] SEEDS = {"1", "42", "oracle"};
    static final int[][] CELLS = {{0, 0}, {0, 1}, {1, 0}, {-1, -1}, {37, -11}};

    static String d(double d) {
        return String.format("d%016x", Double.doubleToRawLongBits(d));
    }

    static String f(float x) {
        return String.format("f%08x", Float.floatToRawIntBits(x));
    }

    static String esc(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') b.append(c); else b.append(String.format("\\u%04x", (int) c));
        }
        return b.append('"').toString();
    }

    static String cfgTiles(List<zombie.iso.worldgen.biomes.TileGroup> list) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < list.size(); i++) {
            zombie.iso.worldgen.biomes.TileGroup g = list.get(i);
            if (i > 0) b.append(',');
            b.append(g.sx()).append('x').append(g.sy()).append(' ').append(g.tiles().size());
        }
        return b.append(']').toString();
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
        zombie.iso.worldgen.WorldGenReader r = new zombie.iso.worldgen.WorldGenReader();
        Map<String, OreVeinConfig> config = r.loadVeinsConfig(worldgen, "veins");
        out.println("CFG " + config.size());
        for (Map.Entry<String, OreVeinConfig> e : config.entrySet()) {
            OreVeinConfig c = e.getValue();
            out.println("C " + esc(e.getKey()) + " cr=" + c.getCenterRadius() + " cp=" + f(c.getCenterProb()) + " amin=" + c.getArmsAmountMin() + " amax=" + c.getArmsAmountMax() + " dmin=" + c.getArmsDistMin() + " dmax=" + c.getArmsDistMax() + " delta=" + c.getArmsDeltaAngle() + " ap=" + f(c.getArmsProb()) + " p=" + f(c.getProbability()) + " tiles=" + cfgTiles(c.getTiles()));
        }

        // the getRandom chain, independent of the seeds below
        for (int[] p : new int[][]{{3, -7}, {0, 0}, {-100, 100}}) {
            for (long off : new long[]{0L, 1000L, -1L}) {
                WorldGenParams.INSTANCE.setSeedString("chain");
                Random rnd = WorldGenParams.INSTANCE.getRandom(p[0], p[1], off);
                out.println("RND " + p[0] + " " + p[1] + " " + off + " " + rnd.nextInt(100) + " " + f(rnd.nextFloat()) + " " + rnd.nextInt(256) + " " + rnd.nextLong() + " " + f(rnd.nextFloat()) + " " + rnd.nextInt(100));
            }
        }

        for (String seed : SEEDS) {
            WorldGenParams.INSTANCE.setSeedString(seed);
            out.println("SEED s=" + esc(seed) + " hash=" + seed.hashCode() + " seed=" + WorldGenParams.INSTANCE.getSeed());
            Veins veins = new Veins(config);
            for (int[] cell : CELLS) {
                List<OreVein> list = veins.get(cell[0], cell[1]);
                out.println("CELL " + esc(seed) + " " + cell[0] + " " + cell[1] + " " + list.size());
                for (int i = 0; i < list.size(); i++) {
                    OreVein v = list.get(i);
                    out.println("OV " + esc(seed) + " " + cell[0] + " " + cell[1] + " " + i
                        + " " + g(v, "centerCellX") + " " + g(v, "centerCellY") + " " + g(v, "centerSquareX") + " " + g(v, "centerSquareY")
                        + " sp=" + f((Float)g2(v, "startPoint").x) + "," + f((Float)g2(v, "startPoint").y)
                        + " amount=" + g(v, "armsAmount") + " cp=" + f((Float)g(v, "centerProb")) + " ap=" + f((Float)g(v, "armsProb")) + " cr=" + g(v, "centerRadius"));
                    float[] orient = (float[])g(v, "armsOrientation");
                    float[] dist = (float[])g(v, "armsDistance");
                    zombie.iso.Vector2[] ends = (zombie.iso.Vector2[])g(v, "endPoints");
                    for (int j = 0; j < orient.length; j++) {
                        out.println("ARM " + esc(seed) + " " + cell[0] + " " + cell[1] + " " + i + " " + j + " " + f(orient[j]) + " " + f(dist[j]) + " " + f(ends[j].x) + " " + f(ends[j].y));
                    }
                    List<zombie.iso.worldgen.biomes.TileGroup> tiles = (List<zombie.iso.worldgen.biomes.TileGroup>)g(v, "tileGroups");
                    out.println("OVT " + esc(seed) + " " + cell[0] + " " + cell[1] + " " + i + " " + cfgTiles(tiles));
                    // isValid over the 8x8 grid at the vein's cell, each point its own Random
                    StringBuilder bits = new StringBuilder();
                    for (int y = 0; y < 8; y++) {
                        for (int x = 0; x < 8; x++) {
                            Random prnd = new Random(1000L + x + y * 8L);
                            bits.append(v.isValid(cell[0] * 256 + x * 32, cell[1] * 256 + y * 32, prnd) ? 1 : 0);
                        }
                    }
                    out.println("VAL " + esc(seed) + " " + cell[0] + " " + cell[1] + " " + i + " " + bits);
                }
                List<OreVein> again = veins.get(cell[0], cell[1]);
                out.println("CACHE " + esc(seed) + " " + cell[0] + " " + cell[1] + " same=" + (again == list ? 1 : 0));
            }
        }

        out.flush();
    }

    static Object g(Object o, String name) throws Exception {
        Class<?> c = o.getClass();
        while (c != null) {
            try {
                java.lang.reflect.Field fl = c.getDeclaredField(name);
                fl.setAccessible(true);
                return fl.get(o);
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    static zombie.iso.Vector2 g2(Object o, String name) throws Exception {
        return (zombie.iso.Vector2)g(o, name);
    }
}
