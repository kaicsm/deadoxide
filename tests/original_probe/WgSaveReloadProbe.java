import java.io.PrintStream;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Arrays;
import java.util.zip.CRC32;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoObject;
import zombie.iso.IsoWorld;
import zombie.iso.sprite.IsoSpriteManager;
import zombie.iso.worldgen.blending.BlendDirection;
import zombie.iso.worldgen.utils.SquareCoord;

/** IsoChunk.Save -> LoadFromDiskOrBuffer on an isolated 8x8 native v249 chunk.
 *  The original game's complete native byte stream is emitted as HEX, not a
 *  derived JSON representation. Compare it byte-for-byte against Rust.
 */
public final class WgSaveReloadProbe {
    static void dump(PrintStream out, String stage, IsoChunk c) {
        out.println(stage + " " + c.wx + " " + c.wy
            + " LEVELS " + c.getMinLevel() + " " + c.getMaxLevel()
            + " BLEND " + (c.isBlendingDoneFull()?1:0) + " " + (c.isBlendingDonePartial()?1:0)
            + " " + (c.isBlendingDone(0)?1:0) + " " + (c.isBlendingDone(1)?1:0)
            + " " + (c.isBlendingDone(2)?1:0) + " " + (c.isBlendingDone(3)?1:0)
            + " ATTACH " + (c.isAttachmentsDoneFull()?1:0)
            + " " + (c.isAttachmentsDone(0)?1:0) + " " + (c.isAttachmentsDone(1)?1:0)
            + " " + (c.isAttachmentsDone(2)?1:0) + " " + (c.isAttachmentsDone(3)?1:0)
            + " " + (c.isAttachmentsDone(4)?1:0)
            + " PARTIAL " + c.attachmentsPartialSize());
        for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) {
            IsoGridSquare sq = c.getGridSquare(x,y,0);
            out.println("S " + x + " " + y + (sq == null ? " null" : WgAdvancedFixture.objects(sq)));
        }
    }
    static byte[] save(IsoChunk chunk) throws Exception {
        ByteBuffer b = chunk.Save(ByteBuffer.allocate(32768), new CRC32(), true);
        return Arrays.copyOf(b.array(), b.position());
    }
    static String hex(byte[] data) {
        StringBuilder s = new StringBuilder(data.length * 2);
        for (byte v : data) s.append(String.format("%02x", v & 255));
        return s.toString();
    }
    static void run(PrintStream out) throws Exception {
        WgAdvancedFixture f = new WgAdvancedFixture("blends_natural_01_0");
        IsoChunk ch = f.center();
        ch.setBlendingDonePartial(true);
        ch.setBlendingModified(0);
        ch.setBlendingModified(2);
        ch.setModifDepth(BlendDirection.NORTH, 4);
        ch.setModifDepth(BlendDirection.WEST, 3);
        ch.setAttachmentsDoneFull(false);
        ch.setAttachmentsState(2, false);
        ch.setAttachmentsPartial(new SquareCoord(ch.wx * 8 + 1, ch.wy * 8 + 2, 0));
        IsoGridSquare first = ch.getGridSquare(0, 0, 0);
        first.getObjects().get(0).setName("Grass");
        first.getObjects().add(new IsoObject(f.cell, first, IsoSpriteManager.instance.getSprite("vegetation_groundcover_01_0")));
        IsoGridSquare second = ch.getGridSquare(1, 0, 0);
        second.getObjects().add(new IsoObject(f.cell, second, IsoSpriteManager.instance.getSprite("vegetation_groundcover_01_0")));
        second.getObjects().add(new IsoObject(f.cell, second, IsoSpriteManager.instance.getSprite("vegetation_groundcover_01_0")));
        dump(out, "BEFORE", ch);
        byte[] data = save(ch);
        CRC32 crc = new CRC32();
        crc.update(data, 17, data.length - 17);
        out.println("BYTES " + data.length + " CRC " + crc.getValue());
        out.println("HEX " + hex(data));
        // A genuine disk round trip, isolated from the player's save and server cache.
        Path temp = Files.createTempDirectory("pz-wgsavereload-");
        Path mapDir = temp.resolve("map");
        Path wxDir = mapDir.resolve(Integer.toString(ch.wx));
        Path saved = wxDir.resolve(ch.wy + ".bin");
        try {
            Files.createDirectories(wxDir);
            Files.write(saved, data, StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
            byte[] onDisk = Files.readAllBytes(saved);
            out.println("DISK_EQUALS " + Arrays.equals(data, onDisk));
            IsoChunk loaded = new IsoChunk(IsoWorld.instance.currentCell);
            loaded.wx = ch.wx;
            loaded.wy = ch.wy;
            loaded.assignLoadID();
            Method m = IsoChunk.class.getDeclaredMethod("LoadFromDiskOrBuffer", ByteBuffer.class);
            m.setAccessible(true);
            m.invoke(loaded, ByteBuffer.wrap(onDisk));
            dump(out, "RELOADED", loaded);
            byte[] again = save(loaded);
            out.println("RESAVE_EQUALS " + Arrays.equals(data, again));
        } finally {
            Files.deleteIfExists(saved);
            Files.deleteIfExists(wxDir);
            Files.deleteIfExists(mapDir);
            Files.deleteIfExists(temp);
        }
    }
    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgSaveReloadProbe::run);
    }
}
