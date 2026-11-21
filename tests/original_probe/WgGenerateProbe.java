import java.io.PrintStream;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoObject;
import zombie.iso.IsoWorld;
import zombie.iso.enums.ChunkGenerationStatus;
import zombie.iso.sprite.IsoSpriteManager;
import zombie.iso.worldgen.ChunksCache;
import zombie.iso.worldgen.WorldGenChunk;
import zombie.iso.worldgen.blending.BlendDirection;
import zombie.iso.worldgen.utils.ChunkCoord;

/**
 * Differential oracle for WorldGenChunk.generateChunks on an owned, empty 2x2 cache.
 * The original server supplies the real WorldGenChunk, tile definitions, CellLoader
 * and Java singletons. No map/lot data or player save is modified.
 *
 * One already occupied square verifies that the random-chunk path preserves it and
 * updates the blending depths; all other squares undergo procedural generation.
 */
public final class WgGenerateProbe {
    static final int BASE_X = 1034;
    static final int BASE_Y = 1458;
    static final int FIXTURE_X = 2;
    static final int FIXTURE_Y = 3;
    static final String FIXTURE_SPRITE = "blends_natural_01_0";

    static int bit(boolean value) {
        return value ? 1 : 0;
    }

    static void dumpChunk(PrintStream out, IsoChunk chunk) {
        out.println("CHUNK " + chunk.wx + " " + chunk.wy +
            " LEVELS " + chunk.getMinLevel() + " " + chunk.getMaxLevel() +
            " WORLDGEN " + bit(chunk.isModded().contains(ChunkGenerationStatus.WORLDGEN)) +
            " BLEND " + bit(chunk.isBlendingDoneFull()) + " " + bit(chunk.isBlendingDonePartial()) +
            " DEPTH " + chunk.getModifDepth(BlendDirection.NORTH) +
            " " + chunk.getModifDepth(BlendDirection.SOUTH) +
            " " + chunk.getModifDepth(BlendDirection.WEST) +
            " " + chunk.getModifDepth(BlendDirection.EAST) +
            " ATTACH " + bit(chunk.isAttachmentsDoneFull()) +
            " " + bit(chunk.isAttachmentsDone(0)) +
            " " + bit(chunk.isAttachmentsDone(1)) +
            " " + bit(chunk.isAttachmentsDone(2)) +
            " " + bit(chunk.isAttachmentsDone(3)) +
            " " + bit(chunk.isAttachmentsDone(4)) +
            " PARTIAL " + chunk.attachmentsPartialSize() +
            " ZOMBIES " + chunk.proceduralZombieSquares.size());

        int nonempty = 0;
        for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) {
            IsoGridSquare square = chunk.getGridSquare(x, y, 0);
            StringBuilder line = new StringBuilder("S ").append(chunk.wx).append(' ')
                .append(chunk.wy).append(' ').append(x).append(' ').append(y);
            if (square == null) {
                line.append(" null");
            } else {
                line.append(" ROOM ").append(square.getRoomID())
                    .append(" OBJECTS ").append(square.getObjects().size());
                // PZArrayList deliberately does not implement iterator(); use indexed access.
                for (int i = 0; i < square.getObjects().size(); i++) {
                    IsoObject object = square.getObjects().get(i);
                    String sprite = object.getSprite() == null ? "null" :
                        String.valueOf(object.getSprite().getName());
                    line.append(" | ").append(object.getClass().getSimpleName())
                        .append(' ').append(sprite);
                    nonempty++;
                }
            }
            out.println(line);
        }
        for (IsoGridSquare square : chunk.proceduralZombieSquares) {
            out.println("Z " + chunk.wx + " " + chunk.wy + " " +
                square.x + " " + square.y + " " + square.z);
        }
        if (nonempty == 0) {
            throw new IllegalStateException("WorldGenChunk left an entire chunk empty: " + chunk.wx + "," + chunk.wy);
        }
    }

    static void run(PrintStream out) throws Exception {
        IsoCell cell = IsoWorld.instance.currentCell;
        WorldGenChunk generator = IsoWorld.instance.getWgChunk();
        if (cell == null || generator == null) {
            throw new IllegalStateException("original server worldgen is not initialized");
        }
        // The ore generator reads the current cell's origin. Fix it explicitly
        // so the Rust fixture can supply exactly the same coordinates.
        cell.setWorldX(0);
        cell.setWorldY(0);
        ChunksCache cache = new ChunksCache();
        for (int x = BASE_X; x < BASE_X + 2; x++) {
            for (int y = BASE_Y; y < BASE_Y + 2; y++) {
                IsoChunk chunk = new IsoChunk(cell);
                // Mirror IsoChunk's normal loader lifecycle before square updates.
                chunk.assignLoadID();
                chunk.wx = x;
                chunk.wy = y;
                cache.put(new ChunkCoord(x, y), chunk);
            }
        }
        IsoChunk partial = cache.get(new ChunkCoord(BASE_X, BASE_Y));
        int worldX = BASE_X * 8 + FIXTURE_X;
        int worldY = BASE_Y * 8 + FIXTURE_Y;
        IsoGridSquare square = IsoGridSquare.getNew(cell, null, worldX, worldY, 0);
        partial.setSquare(FIXTURE_X, FIXTURE_Y, 0, square);
        square.getObjects().add(new IsoObject(cell, square, IsoSpriteManager.instance.getSprite(FIXTURE_SPRITE)));

        generator.generateChunks(cache);
        out.println("GENERATE " + BASE_X + " " + BASE_Y + " SEED " +
            zombie.iso.worldgen.WorldGenParams.INSTANCE.getSeed());
        for (int x = BASE_X; x < BASE_X + 2; x++) {
            for (int y = BASE_Y; y < BASE_Y + 2; y++) {
                dumpChunk(out, cache.get(new ChunkCoord(x, y)));
            }
        }
    }

    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgGenerateProbe::run);
    }
}
