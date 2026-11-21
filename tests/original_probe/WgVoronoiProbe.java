import java.io.PrintStream;
import java.util.Arrays;
import java.util.List;
import java.util.Random;
import se.krka.kahlua.vm.KahluaTable;
import zombie.Lua.LuaManager;
import zombie.SandboxOptions;
import zombie.iso.worldgen.zombie.ZombieVoronoi;
import zombie.iso.worldgen.zombie.ZombieVoronoiEntry;

/** Oracle probe (tools/oracle.sh wgvoronoi): the ORIGINAL zombie-density Voronoi noise - ZombieVoronoi
 * .getVoronois over the game's own `zombie_voronoi` Lua table, evaluateCellNoise / evaluateCellCutoff for
 * fixed seeds and cells, the cell cache and releaseCell, and the HashUtil.hash2D values that seed every
 * section. The Lua setup is wgbiomes' (LuaGameProbe.init: the client's startup headless, then LoadDirBase of
 * shared and server). Printed, in this order (doubles d%016x):
 *   NOISE <0|1>                        SandboxOptions.instance.zombieVoronoiNoise.getValue()
 *   ENTRIES n                          the zombie_voronoi table's entry count
 *   H2D seed x y hash                  HashUtil.hash2D over fixed triples
 *   For each seed of {1, 42, -1, 2147483647, Long.MIN_VALUE, 0x1234567890ABCDEF} and each voronoi i:
 *   ZV seed i points closest scale cutoff
 *   CELL seed i wx wy cached=<0|1>     evaluateCellNoise twice - the second call must return the SAME
 *                                      array instance (the cache) - then releaseCell and a third
 *                                      evaluation (a fresh compute, equal values)
 *   CN seed i wx wy r c <32 values>    four rows of the cell noise (rows 0, 10, 21, 31), each 32 doubles
 *   CUTOFF seed i wx wy <32 values>    evaluateCellCutoff (the sandbox option decides the cutoff), row 10
 *   CUTOFFX seed i wx wy c=<bits> <32 values>   evaluateCellCutoff with the explicit 0.15, row 10
 * Compared with `pz-dump wgvoronoi`.
 * Args: <scratch cache dir> ; run from the game root. */
public class WgVoronoiProbe {
    static PrintStream out;

    static final long[] SEEDS = {1L, 42L, -1L, 2147483647L, Long.MIN_VALUE, 0x1234567890ABCDEFL};
    static final int[] ROWS = {0, 10, 21, 31};
    static final int[][] CELLS = {{0, 0}, {1, -1}, {31, 32}, {-5, 7}};

    static String d(double d) {
        return String.format("d%016x", Double.doubleToRawLongBits(d));
    }

    static String row(double[] noise, int y) {
        StringBuilder b = new StringBuilder();
        for (int x = 0; x < 32; x++) b.append(x == 0 ? "" : ",").append(d(noise[x + y * 32]));
        return b.toString();
    }

    public static void main(String[] a) throws Exception {
        out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        LuaGameProbe.init(new String[]{a[0], "shared", "server"});
        KahluaTable zvoronoiTable = (KahluaTable)LuaManager.env.rawget("zombie_voronoi");
        if (zvoronoiTable == null) {
            System.err.println("ORACLE: zombie_voronoi table missing from the Lua environment");
            Runtime.getRuntime().halt(4);
        }
        boolean noiseOption = SandboxOptions.instance.zombieVoronoiNoise.getValue();
        out.println("NOISE " + (noiseOption ? 1 : 0));

        List<ZombieVoronoi> forCount = ZombieVoronoi.getVoronois(0L);
        out.println("ENTRIES " + forCount.size());

        for (long seed : SEEDS) {
            List<ZombieVoronoi> voronois = ZombieVoronoi.getVoronois(seed);
            for (int i = 0; i < voronois.size(); i++) {
                ZombieVoronoi v = voronois.get(i);
                ZombieVoronoiEntry entry = new zombie.iso.worldgen.zombie.ZombieVoronoiReader().getEntries(zvoronoiTable).get(i);
                out.println("ZV " + seed + " " + i + " points=" + entry.numberPoints() + " closest=" + entry.closestPoint() + " scale=" + d(entry.scale()) + " cutoff=" + d(entry.cutoff()));
                for (int[] cell : CELLS) {
                    double[] first = v.evaluateCellNoise(cell[0], cell[1]);
                    double[] second = v.evaluateCellNoise(cell[0], cell[1]);
                    out.println("CELL " + seed + " " + i + " " + cell[0] + " " + cell[1] + " cached=" + (first == second ? 1 : 0));
                    for (int y : ROWS) out.println("CN " + seed + " " + i + " " + cell[0] + " " + cell[1] + " " + y + " " + row(first, y));
                    double[] cutoff = v.evaluateCellCutoff(cell[0], cell[1]);
                    out.println("CUTOFF " + seed + " " + i + " " + cell[0] + " " + cell[1] + " " + 10 + " " + row(cutoff, 10));
                    double[] cutoffX = v.evaluateCellCutoff(cell[0], cell[1], 0.15);
                    out.println("CUTOFFX " + seed + " " + i + " " + cell[0] + " " + cell[1] + " c=" + d(0.15) + " " + row(cutoffX, 10));
                    v.releaseCell(cell[0], cell[1]);
                    double[] third = v.evaluateCellNoise(cell[0], cell[1]);
                    out.println("CELL " + seed + " " + i + " " + cell[0] + " " + cell[1] + " released-eq=" + (Arrays.equals(first, third) ? 1 : 0));
                }
            }
        }

        // the section hash, directly
        out.println("H2D start");
        long[] hseeds = {0L, 1L, -1L, 1234567890123L};
        long[] hxs = {0L, 1L, -1L, 255L, 1000000L, -999999999L};
        for (long hs : hseeds) for (long hx : hxs) out.println("H2D " + hs + " " + hx + " " + (hx * 2) + " " + zombie.iso.worldgen.zombie.HashUtil.hash2D(hs, hx, hx * 2));
        for (long hx : hxs) out.println("H2D " + Long.MIN_VALUE + " " + hx + " " + (hx - 7) + " " + zombie.iso.worldgen.zombie.HashUtil.hash2D(Long.MIN_VALUE, hx, hx - 7));

        out.flush();
    }
}
