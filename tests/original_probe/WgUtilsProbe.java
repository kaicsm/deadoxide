import java.io.PrintStream;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Random;
import zombie.iso.worldgen.enums.TileLookup;
import zombie.iso.worldgen.enums.TileReplacement;
import zombie.iso.worldgen.utils.AreaSquareVar;
import zombie.iso.worldgen.utils.CacheMap;
import zombie.iso.worldgen.utils.CellCoord;
import zombie.iso.worldgen.utils.ChunkCoord;
import zombie.iso.worldgen.utils.Direction;
import zombie.iso.worldgen.utils.SquareCoord;
import zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator;
import zombie.iso.worldgen.utils.triangulation.Edge2D;
import zombie.iso.worldgen.utils.triangulation.EdgeDistancePack;
import zombie.iso.worldgen.utils.triangulation.Triangle2D;
import zombie.iso.worldgen.utils.triangulation.Vector2D;

/** Oracle probe (tools/oracle.sh wgutils): the ORIGINAL worldgen utility classes - the coordinate records,
 * Direction, CacheMap, AreaSquareVar, and the Delaunay triangulation family - no game state, a plain JVM.
 * Printed, in this order (floats as raw bits f%08x, doubles d%016x, strings escaped):
 *   HASH record hash codes over a fixed coordinate list
 *   SQSAVE SquareCoord.save/load roundtrip over a ByteBuffer
 *   DIR i idx x y name opposite prev next            every Direction (name = the "n"/"s"... field)
 *   DIR cardinals=[..] diagonals=[..] rose=[..]      the name fields of the three lists
 *   CACHE put k -> size v                            five puts into a CacheMap(3), then gets
 *   ASV defaults / ASV set                           AreaSquareVar fields then toString
 *   ENUM TileLookup [..] / ENUM TileReplacement [..] values() names in order
 *   V x y sub/add/mult/mag/magSqrt/dot/cross         the Vector2D algebra over fixed vectors
 *   VSTR s                                          Vector2D.toString
 *   EDP a b cmp                                     EdgeDistancePack.compareTo (-1/0/1) incl. -0.0/+0.0/NaN
 *   DSET i TRI n EDGE m ...                         Delaunay triangulations of five point sets: the
 *                                                    triangles and edges SORTED (Double.compare on the
 *                                                    coordinates, identity-hash set orders are not
 *                                                    reproducible), each edge with magSqrt bits;
 *                                                    DSET with too few points prints NEE; DSHUF prints
 *                                                    the edge set again after shuffle(int[]) +
 *                                                    re-triangulation
 *   H2D seed x y                                    HashUtil-free hash note: none (HashUtil lives with the
 *                                                    zombie package and is covered by wgvoronoi)
 * Compared with `pz-dump wgutils`. No args. */
public class WgUtilsProbe {
    static PrintStream out;

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

    static int cmp(double a, double b) {
        return Double.compare(a, b);
    }

    static String pt(Vector2D v) {
        return "(" + d(v.x) + " " + d(v.y) + ")";
    }

    static void triangulationCase(int id, List<Vector2D> points) {
        out.println("DSET " + id + " PTS " + points.size());
        if (points.size() < 3) {
            DelaunayTriangulator t0 = new DelaunayTriangulator(new ArrayList<>(points));
            try {
                t0.triangulate();
                out.println("DSET " + id + " UNEXPECTED");
            } catch (Exception e) {
                out.println("DSET " + id + " NEE " + esc(e.getMessage()));
            }
            return;
        }
        DelaunayTriangulator t = new DelaunayTriangulator(new ArrayList<>(points));
        try {
            t.triangulate();
        } catch (Exception e) {
            out.println("DSET " + id + " ERR " + esc(e.toString()));
            return;
        }
        List<Triangle2D> tris = new ArrayList<>(t.getTriangles());
        tris.sort(Comparator.comparingDouble((Triangle2D tr) -> tr.a.x).thenComparingDouble(tr -> tr.a.y).thenComparingDouble(tr -> tr.b.x).thenComparingDouble(tr -> tr.b.y).thenComparingDouble(tr -> tr.c.x).thenComparingDouble(tr -> tr.c.y));
        out.println("DSET " + id + " TRI " + tris.size());
        for (Triangle2D tr : tris) out.println("DTRI " + id + " " + pt(tr.a) + " " + pt(tr.b) + " " + pt(tr.c));
        List<Edge2D> edges = new ArrayList<>(t.getEdges());
        edges.sort(Comparator.comparingDouble((Edge2D e) -> e.a.x).thenComparingDouble(e -> e.a.y).thenComparingDouble(e -> e.b.x).thenComparingDouble(e -> e.b.y));
        out.println("DSET " + id + " EDGE " + edges.size());
        for (Edge2D e : edges) out.println("DEDGE " + id + " " + pt(e.a) + " " + pt(e.b) + " " + d(e.magSqrt()));
        if (id == 1) {
            DelaunayTriangulator t2 = new DelaunayTriangulator(new ArrayList<>(points));
            t2.shuffle(new int[]{3, 1, 4, 0, 2, 6, 5});
            try {
                t2.triangulate();
                List<Edge2D> edges2 = new ArrayList<>(t2.getEdges());
                edges2.sort(Comparator.comparingDouble((Edge2D e) -> e.a.x).thenComparingDouble(e -> e.a.y).thenComparingDouble(e -> e.b.x).thenComparingDouble(e -> e.b.y));
                out.println("DSHUF " + id + " EDGE " + edges2.size());
                for (Edge2D e : edges2) out.println("DEDGE " + id + " " + pt(e.a) + " " + pt(e.b) + " " + d(e.magSqrt()));
            } catch (Exception e) {
                out.println("DSHUF " + id + " ERR " + esc(e.toString()));
            }
        }
    }

    public static void main(String[] args) throws Exception {
        out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);

        // the record hash codes over a fixed coordinate list
        out.println("HASH start");
        int[][] coords = {{0, 0}, {1, 0}, {0, 1}, {-1, -1}, {255, 255}, {123456, -654321}, {Integer.MAX_VALUE, Integer.MIN_VALUE}};
        for (int[] c : coords) out.println("HASH CC " + c[0] + " " + c[1] + " " + new CellCoord(c[0], c[1]).hashCode() + " " + new ChunkCoord(c[0], c[1]).hashCode());
        for (int[] c : coords) out.println("HASH SC " + c[0] + " " + c[1] + " " + new SquareCoord(c[0], c[1], (c[0] + c[1]) & 3).hashCode());

        // SquareCoord save/load roundtrip
        ByteBuffer bb = ByteBuffer.allocate(64);
        SquareCoord sc = new SquareCoord(17, -34, 8);
        sc.save(bb);
        bb.flip();
        SquareCoord sc2 = SquareCoord.load(bb);
        out.println("SQSAVE " + sc2.x() + " " + sc2.y() + " " + sc2.z() + " eq=" + sc.equals(sc2) + " rem=" + bb.remaining());

        // Direction
        out.println("DIR start");
        for (Direction dir : Direction.values()) {
            out.println("DIR " + dir.index + " " + dir.x + " " + dir.y + " " + dir.name + " " + dir.opposite().name + " " + dir.prev().name + " " + dir.next().name);
        }
        StringBuilder card = new StringBuilder("[");
        for (Direction dir : Direction.cardinals()) card.append(card.length() == 1 ? "" : ",").append(dir.name);
        StringBuilder diag = new StringBuilder("[");
        for (Direction dir : Direction.diagonals()) diag.append(diag.length() == 1 ? "" : ",").append(dir.name);
        StringBuilder rose = new StringBuilder("[");
        for (Direction dir : Direction.rose()) rose.append(rose.length() == 1 ? "" : ",").append(dir.name);
        out.println("DIR cardinals=" + card + " diagonals=" + diag + " rose=" + rose);

        // CacheMap(3): five puts, then gets of an old and the new key
        CacheMap<String, Integer> cm = new CacheMap<>(3);
        for (int i = 1; i <= 5; i++) {
            cm.put("k" + i, i);
            out.println("CACHE put k" + i + " size=" + cm.size());
        }
        out.println("CACHE get k1=" + cm.get("k1") + " k5=" + cm.get("k5") + " has2=" + cm.containsKey("k2"));

        // AreaSquareVar
        AreaSquareVar asv = new AreaSquareVar();
        out.println("ASV defaults " + asv.getXmin() + " " + asv.getXmax() + " " + asv.getYmin() + " " + asv.getYmax());
        asv.setXmin(-5);
        asv.setXmax(30);
        asv.setYmin(-9);
        asv.setYmax(41);
        out.println("ASV set " + asv.toString());

        // the enums
        StringBuilder tl = new StringBuilder("[");
        for (TileLookup v : TileLookup.values()) tl.append(tl.length() == 1 ? "" : ",").append(v.toString());
        out.println("ENUM TileLookup " + tl);
        StringBuilder tr = new StringBuilder("[");
        for (TileReplacement v : TileReplacement.values()) tr.append(tr.length() == 1 ? "" : ",").append(v.toString());
        out.println("ENUM TileReplacement " + tr);

        // Vector2D algebra
        Vector2D a = new Vector2D(3.5, -2.25);
        Vector2D b = new Vector2D(-1.0, 7.75);
        out.println("V " + d(a.x) + " " + d(a.y) + " sub " + pt(a.sub(b)) + " add " + pt(a.add(b)) + " mult " + pt(a.mult(2.5)));
        out.println("V mag " + d(a.mag()) + " magSqrt " + d(a.magSqrt()) + " dot " + d(a.dot(b)) + " cross " + d(a.cross(b)));
        Vector2D z = new Vector2D(0.0, -0.0);
        out.println("V zero mag " + d(z.mag()) + " magSqrt " + d(z.magSqrt()));
        out.println("VSTR " + esc(a.toString()) + " " + esc(new Vector2D(1.0, 2.0).toString()) + " " + esc(new Vector2D(-0.5, 1.0e-3).toString()));

        // EdgeDistancePack.compareTo
        double[] ds = {1.0, -0.0, 0.0, Double.NaN, Double.POSITIVE_INFINITY, -1.0};
        for (int i = 0; i < ds.length; i++) {
            for (int j = 0; j < ds.length; j++) {
                EdgeDistancePack p1 = new EdgeDistancePack(null, ds[i]);
                EdgeDistancePack p2 = new EdgeDistancePack(null, ds[j]);
                out.println("EDP " + i + " " + j + " " + Integer.signum(p1.compareTo(p2)));
            }
        }

        // Delaunay triangulations of fixed point sets
        List<Vector2D> set1 = new ArrayList<>();
        set1.add(new Vector2D(0.0, 0.0));
        set1.add(new Vector2D(10.0, 0.0));
        set1.add(new Vector2D(0.0, 10.0));
        set1.add(new Vector2D(10.0, 10.0));
        set1.add(new Vector2D(5.0, 5.0));
        set1.add(new Vector2D(5.0, 0.5));
        set1.add(new Vector2D(2.5, 7.5));
        triangulationCase(1, set1);

        Random rnd = new Random(42);
        List<Vector2D> set2 = new ArrayList<>();
        for (int i = 0; i < 12; i++) set2.add(new Vector2D(rnd.nextDouble() * 100.0, rnd.nextDouble() * 100.0));
        triangulationCase(2, set2);

        List<Vector2D> set3 = new ArrayList<>();
        for (int x = 0; x < 5; x++) for (int y = 0; y < 5; y++) set3.add(new Vector2D(x * 20.0, y * 20.0));
        triangulationCase(3, set3);

        List<Vector2D> set4 = new ArrayList<>();
        set4.add(new Vector2D(1.0, 1.0));
        set4.add(new Vector2D(1.0, 1.0));
        set4.add(new Vector2D(9.0, 1.0));
        set4.add(new Vector2D(5.0, 9.0));
        set4.add(new Vector2D(-3.0, 4.0));
        triangulationCase(4, set4);

        List<Vector2D> set5 = new ArrayList<>();
        set5.add(new Vector2D(1.0, 1.0));
        set5.add(new Vector2D(2.0, 2.0));
        triangulationCase(5, set5);

        out.flush();
    }
}
