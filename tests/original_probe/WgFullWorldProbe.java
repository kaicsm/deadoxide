import java.io.PrintStream;
import zombie.iso.CellLoader;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoWorld;
import zombie.iso.worldgen.ChunksCache;
import zombie.iso.worldgen.WorldGenChunk;
import zombie.iso.worldgen.WorldGenParams;
import zombie.iso.worldgen.WorldGenUtils;
import zombie.iso.worldgen.utils.ChunkCoord;

/**
 * IsoChunk.LoadBrandNew -> WorldGenChunk.generateChunks -> IsoChunk.update (WorldGen stages).
 * Every requested chunk independently re-reads its actual 2x2 lotpack group, and only that
 * requested chunk is retained. The 3x3 survivors are registered in the original server's
 * chunk lookups before the blending/attachments update phase.
 */
public final class WgFullWorldProbe {
    static int bit(boolean v) { return v ? 1 : 0; }

    static IsoChunk brandNew(IsoCell cell, WorldGenChunk generator, int wx, int wy) {
        int minX = WorldGenUtils.INSTANCE.getCornerOfGeneration(wx);
        int minY = WorldGenUtils.INSTANCE.getCornerOfGeneration(wy);
        ChunksCache cache = new ChunksCache();
        for (int x = minX; x < minX + 2; x++) {
            for (int y = minY; y < minY + 2; y++) {
                IsoChunk ch = new IsoChunk(cell);
                ch.wx = x;
                ch.wy = y;
                ch.assignLoadID();
                CellLoader.LoadCellBinaryChunk(cell, x, y, ch);
                cache.put(new ChunkCoord(x, y), ch);
            }
        }
        generator.generateChunks(cache);
        IsoChunk kept = cache.get(new ChunkCoord(wx, wy));
        if (kept == null) throw new IllegalStateException("missing generated chunk");
        kept.loaded = true;
        return kept;
    }

    static void dump(PrintStream out, String stage, WgAdvancedFixture f) {
        out.println("STAGE " + stage + " SEED " + WorldGenParams.INSTANCE.getSeed());
        for (int dx = -1; dx <= 1; dx++) {
            for (int dy = -1; dy <= 1; dy++) {
                IsoChunk ch = f.neighbour(dx, dy);
                out.println("CHUNK " + ch.wx + " " + ch.wy
                    + " LEVELS " + ch.getMinLevel() + " " + ch.getMaxLevel()
                    + " BLEND " + bit(ch.isBlendingDoneFull()) + " " + bit(ch.isBlendingDonePartial())
                    + " FLAGS " + bit(ch.isBlendingDone(0)) + " " + bit(ch.isBlendingDone(1))
                    + " " + bit(ch.isBlendingDone(2)) + " " + bit(ch.isBlendingDone(3))
                    + " ATTACH " + bit(ch.isAttachmentsDoneFull())
                    + " " + bit(ch.isAttachmentsDone(0)) + " " + bit(ch.isAttachmentsDone(1))
                    + " " + bit(ch.isAttachmentsDone(2)) + " " + bit(ch.isAttachmentsDone(3))
                    + " " + bit(ch.isAttachmentsDone(4)));
                for (int x = 0; x < 8; x++) {
                    for (int y = 0; y < 8; y++) {
                        IsoGridSquare sq = ch.getGridSquare(x, y, 0);
                        if (sq == null) {
                            out.println("S " + ch.wx + " " + ch.wy + " " + x + " " + y + " null");
                        } else {
                            out.println("S " + ch.wx + " " + ch.wy + " " + x + " " + y
                                + " ROOM " + sq.getRoomID() + WgAdvancedFixture.objects(sq));
                        }
                    }
                }
            }
        }
    }

    static void run(PrintStream out) throws Exception {
        IsoCell cell = IsoWorld.instance.currentCell;
        WorldGenChunk generator = IsoWorld.instance.getWgChunk();
        cell.setWorldX(0);
        cell.setWorldY(0);
        // The same deterministic prefab used by the already-measured wgprefabs oracle.
        WgPrefabsProbe.injectPrefab(generator);
        IsoChunk[][] generated = new IsoChunk[3][3];
        for (int dx = -1; dx <= 1; dx++) {
            for (int dy = -1; dy <= 1; dy++) {
                int wx = WgAdvancedFixture.CX + dx;
                int wy = WgAdvancedFixture.CY + dy;
                generated[dx + 1][dy + 1] = brandNew(cell, generator, wx, wy);
            }
        }
        // Register only after generation, so a fixture floor cannot influence CellLoader
        // or any world lookup during a 2x2 LoadBrandNew.
        WgAdvancedFixture fixture = new WgAdvancedFixture("blends_natural_01_0");
        for (int dx = -1; dx <= 1; dx++) {
            for (int dy = -1; dy <= 1; dy++) {
                fixture.chunks[dx + 1][dy + 1] = generated[dx + 1][dy + 1];
            }
        }
        fixture.register();
        dump(out, "GENERATED", fixture);
        // IsoChunk.update: the two WorldGen stages only (no physics/zombie/gameplay changes).
        for (int dx = -1; dx <= 1; dx++) {
            for (int dy = -1; dy <= 1; dy++) {
                IsoChunk ch = fixture.neighbour(dx, dy);
                if (!ch.isBlendingDoneFull() && !(
                        ch.isBlendingDone(0) && ch.isBlendingDone(1)
                        && ch.isBlendingDone(2) && ch.isBlendingDone(3))) {
                    IsoWorld.instance.getBlending().applyBlending(ch);
                }
                if (!ch.isAttachmentsDoneFull()) {
                    IsoWorld.instance.getAttachmentsHandler().applyAttachments(ch);
                }
            }
        }
        dump(out, "POSTPROCESSED", fixture);
        // Repeating IsoChunk.update must not introduce new objects or alter the handshakes.
        for (int dx = -1; dx <= 1; dx++) {
            for (int dy = -1; dy <= 1; dy++) {
                IsoChunk ch = fixture.neighbour(dx, dy);
                if (!ch.isBlendingDoneFull() && !(
                        ch.isBlendingDone(0) && ch.isBlendingDone(1)
                        && ch.isBlendingDone(2) && ch.isBlendingDone(3))) {
                    IsoWorld.instance.getBlending().applyBlending(ch);
                }
                if (!ch.isAttachmentsDoneFull()) {
                    IsoWorld.instance.getAttachmentsHandler().applyAttachments(ch);
                }
            }
        }
        dump(out, "REPEATED", fixture);
    }

    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgFullWorldProbe::run);
    }
}
