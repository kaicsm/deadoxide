import java.io.PrintStream;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoWorld;
import zombie.iso.objects.IsoTree;
import zombie.iso.sprite.IsoSpriteManager;
import zombie.iso.worldgen.blending.BlendDirection;

/** Full Blending.applyBlending over a loaded, isolated 3x3 server-map fixture. */
public final class WgBlendingProbe {
    static void bitState(PrintStream out, String name, IsoChunk chunk) {
        out.println(name + " FULL " + (chunk.isBlendingDoneFull() ? 1 : 0)
            + " PARTIAL " + (chunk.isBlendingDonePartial() ? 1 : 0)
            + " DIR " + (chunk.isBlendingDone(0) ? 1 : 0)
            + " " + (chunk.isBlendingDone(1) ? 1 : 0)
            + " " + (chunk.isBlendingDone(2) ? 1 : 0)
            + " " + (chunk.isBlendingDone(3) ? 1 : 0));
    }

    static void dump(PrintStream out, WgAdvancedFixture f, String label) {
        IsoChunk chunk = f.center();
        bitState(out, label, chunk);
        for (BlendDirection dir : BlendDirection.values()) {
            IsoChunk neighbor = f.neighbour(dir.x, dir.y);
            out.println("NEIGHBOR " + dir + " " + (neighbor.isBlendingDone(dir.opposite().index) ? 1 : 0));
        }
        for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) {
            IsoGridSquare sq = chunk.getGridSquare(x, y, 0);
            out.println("S " + x + " " + y + WgAdvancedFixture.objects(sq));
        }
    }

    static void run(PrintStream out) throws Exception {
        WgAdvancedFixture f = new WgAdvancedFixture("blends_natural_01_0");
        IsoChunk center = f.center();
        // One procedural neighbour is sufficient to trigger a real ground/tree transition.
        f.neighbour(-1, 0).setBlendingDoneFull(true);
        IsoGridSquare sq = center.getGridSquare(0, 0, 0);
        sq.getObjects().add(new IsoTree(sq, IsoSpriteManager.instance.getSprite("vegetation_trees_01_0")));
        sq = center.getGridSquare(0, 4, 0);
        sq.getObjects().add(new IsoTree(sq, IsoSpriteManager.instance.getSprite("vegetation_trees_01_0")));
        dump(out, f, "BEFORE");
        IsoWorld.instance.getBlending().applyBlending(center);
        dump(out, f, "AFTER");
        // A second call must be idempotent after the four directional handshakes.
        IsoWorld.instance.getBlending().applyBlending(center);
        dump(out, f, "REPEAT");
    }

    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgBlendingProbe::run);
    }
}
