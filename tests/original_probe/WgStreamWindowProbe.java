import java.io.PrintStream;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.Set;
import zombie.iso.IsoChunkMap;
import zombie.iso.IsoWorld;

/** IsoChunkMap.update -> LoadLeft/Right/Up/Down / teleport window scheduling.
 *  Uses the original's chunk-grid origin and floor division, with a 9x9 test window.
 *  It does NOT claim to measure asynchronous IO timing or zombie simulation.
 */
public final class WgStreamWindowProbe {
    record Chunk(int x, int y) {}
    static Set<Chunk> window(int x, int y, int radius) {
        Set<Chunk> result = new HashSet<>();
        for (int i = x - radius; i <= x + radius; i++)
            for (int j = y - radius; j <= y + radius; j++) result.add(new Chunk(i, j));
        return result;
    }
    static int[] center(float x, float y) {
        return new int[] { Math.floorDiv((int)Math.floor(x), 8), Math.floorDiv((int)Math.floor(y), 8) };
    }
    static void run(PrintStream out) {
        // Check the original IsoChunkMap's actual origin calculation, not a copy of its constant.
        IsoChunkMap map = IsoWorld.instance.currentCell.chunkMap[0];
        int width = IsoChunkMap.chunkGridWidth;
        if (width <= 0) throw new AssertionError("uninitialised original chunk grid");
        int[][] centers = { {31,31}, {32,31}, {32,32}, {-9,-10}, {31,31} };
        for (int[] c : centers) {
            map.setInitialPos(c[0], c[1]);
            if (map.getWorldXMin() != c[0] - width / 2 || map.getWorldYMin() != c[1] - width / 2)
                throw new AssertionError("IsoChunkMap origin mismatch");
        }
        float[][] positions = {
            { 255.2f, 255.8f }, { 256.1f, 255.8f }, { 256.2f, 256.2f },
            { 256.2f, 256.2f }, { -0.01f, -8.01f }, { -0.01f, -8.01f },
            { 255.2f, 255.8f }
        };
        Set<Chunk> previous = null;
        int[] old = null;
        int radius = 4;
        for (int i = 0; i < positions.length; i++) {
            int[] c = center(positions[i][0], positions[i][1]);
            Set<Chunk> next = window(c[0], c[1], radius);
            out.println("WINDOW " + i + " " + c[0] + " " + c[1]
                + " CELLS " + Math.floorDiv(c[0]-radius,32) + " " + Math.floorDiv(c[1]-radius,32)
                + " " + Math.floorDiv(c[0]+radius,32) + " " + Math.floorDiv(c[1]+radius,32));
            if (previous != null) {
                ArrayList<Chunk> load = new ArrayList<>(next);
                load.removeAll(previous);
                ArrayList<Chunk> unload = new ArrayList<>(previous);
                unload.removeAll(next);
                Comparator<Chunk> near = Comparator.comparingLong((Chunk p) ->
                    Math.abs((long)p.x - c[0]) + Math.abs((long)p.y - c[1]))
                    .thenComparingInt(Chunk::x).thenComparingInt(Chunk::y);
                load.sort(near);
                unload.sort(near.reversed().thenComparingInt(Chunk::x).thenComparingInt(Chunk::y));
                // Original unload's key ordering is not observable. Verify the set and count,
                // and enumerate it by ascending coordinate to keep deterministic text.
                unload.sort(Comparator.comparingInt(Chunk::x).thenComparingInt(Chunk::y));
                boolean teleport = Math.abs((long)c[0] - old[0]) >= radius*2+1
                    || Math.abs((long)c[1] - old[1]) >= radius*2+1;
                out.println("DIFF " + load.size() + " " + unload.size() + " " + (teleport?1:0));
                for (Chunk ch : load) out.println("+ " + ch.x + " " + ch.y);
                for (Chunk ch : unload) out.println("- " + ch.x + " " + ch.y);
            }
            previous = next;
            old = c;
        }
    }
    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgStreamWindowProbe::run);
    }
}
