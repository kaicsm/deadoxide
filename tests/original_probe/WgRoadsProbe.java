import java.io.PrintStream;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Random;
import zombie.iso.IsoWorld;
import zombie.iso.worldgen.WorldGenParams;
import zombie.iso.worldgen.roads.Road;
import zombie.iso.worldgen.roads.RoadConfig;
import zombie.iso.worldgen.roads.RoadGenerator;
import zombie.iso.worldgen.roads.RoadNexus;
import zombie.iso.worldgen.utils.triangulation.Edge2D;
import zombie.iso.worldgen.utils.triangulation.Vector2D;

/** Oracle probe (tools/oracle.sh wgroads): the ORIGINAL road network generator inside the booted dedicated
 * server (ServerHarness): RoadGenerator over the game's own `worldgen.roads` config, the real map's meta
 * grid bounds and WorldGenParams seeds set through setSeedString, with getRoads over fixed chunk
 * coordinates. Printed, in this order (doubles d%016x; the config map in its java.util.HashMap iteration
 * order; every point/edge/nexus/road list SORTED by the coordinates, Double.compare per axis - the Delaunay
 * edge set and the HashSet orders behind it are identity-hash based and not reproducible):
 *   META minX maxX minY maxY                       IsoWorld.instance.metaGrid's bounds
 *   CFG n / C <name> proba p filter                the road config
 *   For each seed string of {"1", "42", "DeadOxideOracle"} and each config entry (offset 1000, 2000, ...):
 *   RG seed offset name pts edges filtered nexus   the generator's four list sizes
 *   PT x y                                         every nexus point, sorted
 *   FE ax ay bx by magSqrt                         every filtered edge, sorted
 *   NX x y remotes=[(x y),..]                      every nexus (its remotes sorted), sorted by point
 *   RD cx cy (ax ay)-(bx by) dir ca.x ca.y cb.x cb.y   getRoads over the fixed sample chunks, sorted
 *   On the RuntimeException of a too-small triangulation: RG seed offset name NEE
 * Compared with `pz-dump wgroads`.
 * Args: <scratch dir> (ServerHarness's, unused). */
public class WgRoadsProbe implements ServerHarness.Probe {
    static PrintStream out;

    /** The private field read (RoadGenerator exposes only getRoadNexus). */
    static Object g(Object o, String name) throws Exception {
        Class<?> c = o.getClass();
        while (c != null) {
            try {
                java.lang.reflect.Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    static String d(double d) {
        return String.format("d%016x", Double.doubleToRawLongBits(d));
    }

    static String esc(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') b.append(c); else b.append(String.format("\\u%04x", (int) c));
        }
        return b.append('"').toString();
    }

    static String pt(Vector2D v) {
        return "(" + d(v.x) + " " + d(v.y) + ")";
    }

    static final Comparator<Vector2D> BY_XY = Comparator.comparingDouble((Vector2D v) -> v.x).thenComparingDouble(v -> v.y);

    static final Comparator<Edge2D> BY_EDGE = Comparator.comparingDouble((Edge2D e) -> e.a.x).thenComparingDouble(e -> e.a.y).thenComparingDouble(e -> e.b.x).thenComparingDouble(e -> e.b.y);

    static String hexName(String seed) {
        return esc(seed);
    }

    /** A nexus point with its index into roadDelaunayPoints, so the sorted print order can address the
     * matching RoadNexus (the nexus list is the points list, same order). */
    record IndexedPoint(Vector2D point, int index) {}

    static final Comparator<IndexedPoint> BY_IXY = Comparator.comparingDouble((IndexedPoint p) -> p.point().x).thenComparingDouble(p -> p.point().y);

    public void run(PrintStream out) throws Exception {
        WgRoadsProbe.out = out;
        int minX = IsoWorld.instance.metaGrid.minX;
        int maxX = IsoWorld.instance.metaGrid.maxX;
        int minY = IsoWorld.instance.metaGrid.minY;
        int maxY = IsoWorld.instance.metaGrid.maxY;
        out.println("META " + minX + " " + maxX + " " + minY + " " + maxY);

        zombie.iso.worldgen.WorldGenReader r = new zombie.iso.worldgen.WorldGenReader();
        se.krka.kahlua.vm.KahluaTable worldgen = (se.krka.kahlua.vm.KahluaTable)zombie.Lua.LuaManager.env.rawget("worldgen");
        if (worldgen == null) {
            System.err.println("ORACLE: worldgen table missing from the Lua environment");
            Runtime.getRuntime().halt(4);
        }
        Map<String, RoadConfig> config = r.loadRoadConfig(worldgen, "roads");
        out.println("CFG " + config.size());
        for (Map.Entry<String, RoadConfig> e : config.entrySet()) {
            out.println("C " + esc(e.getKey()) + " proba=" + d(e.getValue().probaRoads()) + " p=" + d(e.getValue().probability()) + " filter=" + d(e.getValue().filter()));
        }

        int[] sampleChunks = {0, 30, 1574, -1, 1236};
        for (String seed : new String[]{"1", "42", "DeadOxideOracle"}) {
            WorldGenParams.INSTANCE.setSeedString(seed);
            int offset = 1000;
            for (Map.Entry<String, RoadConfig> e : config.entrySet()) {
                RoadGenerator gen;
                try {
                    gen = new RoadGenerator(WorldGenParams.INSTANCE.getSeed(), e.getValue(), offset);
                } catch (Throwable t) {
                    out.println("RG " + esc(seed) + " " + offset + " " + esc(e.getKey()) + " NEE " + esc(String.valueOf(t.getMessage())));
                    offset += 1000;
                    continue;
                }
                List<Vector2D> allPts = (List<Vector2D>)g(gen, "roadDelaunayPoints");
                List<Edge2D> allEdges = (List<Edge2D>)g(gen, "roadDelaunayEdges");
                List<Edge2D> allFiltered = (List<Edge2D>)g(gen, "roadDelaunayEdgesFiltered");
                List<IndexedPoint> pts = new ArrayList<>();
                for (int pi = 0; pi < allPts.size(); pi++) pts.add(new IndexedPoint(allPts.get(pi), pi));
                pts.sort(BY_IXY);
                List<Edge2D> fe = new ArrayList<>(allFiltered);
                fe.sort(BY_EDGE);
                List<RoadNexus> nexus = gen.getRoadNexus();
                out.println("RG " + esc(seed) + " " + offset + " " + esc(e.getKey()) + " pts=" + allPts.size() + " edges=" + allEdges.size() + " filtered=" + allFiltered.size() + " nexus=" + gen.getRoadNexus().size());
                for (IndexedPoint p : pts) out.println("PT " + pt(p.point()));
                for (Edge2D edge : fe) out.println("FE " + pt(edge.a) + " " + pt(edge.b) + " " + d(edge.magSqrt()));
                for (IndexedPoint p : pts) {
                    RoadNexus n = nexus.get(p.index());
                    List<org.joml.Vector2i> remotes = new ArrayList<>(n.getDelaunayRemotes());
                    remotes.sort(Comparator.comparingInt((org.joml.Vector2i v) -> v.x).thenComparingInt(v -> v.y));
                    StringBuilder sb = new StringBuilder("[");
                    for (int i = 0; i < remotes.size(); i++) sb.append(i == 0 ? "" : ",").append("(").append(remotes.get(i).x).append(" ").append(remotes.get(i).y).append(")");
                    out.println("NX " + n.getDelaunayPoint().x + " " + n.getDelaunayPoint().y + " remotes=" + sb + "]");
                }
                for (int cx : sampleChunks) {
                    for (int cy : sampleChunks) {
                        List<Road> roads = new ArrayList<>(gen.getRoads(cx, cy));
                        roads.sort(Comparator.comparingInt((Road rd) -> rd.getA().x).thenComparingInt(rd -> rd.getA().y).thenComparingInt(rd -> rd.getB().x).thenComparingInt(rd -> rd.getB().y));
                        for (Road rd : roads) {
                            out.println("RD " + cx + " " + cy + " (" + rd.getA().x + " " + rd.getA().y + ")-(" + rd.getB().x + " " + rd.getB().y + ") " + rd.getDirection() + " " + rd.getCA().x() + " " + rd.getCA().y() + " " + rd.getCB().x() + " " + rd.getCB().y());
                        }
                    }
                }
                offset += 1000;
            }
        }
        out.flush();
    }

    public static void main(String[] a) throws Exception {
        ServerHarness.run(a[0], new WgRoadsProbe());
    }
}
