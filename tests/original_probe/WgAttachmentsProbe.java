import java.io.PrintStream;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Collections;
import java.util.EnumMap;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import zombie.iso.IsoChunk;
import zombie.iso.IsoChunkMap;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoWorld;
import zombie.iso.SpriteDetails.IsoFlagType;
import zombie.iso.sprite.IsoSprite;
import zombie.iso.sprite.IsoSpriteManager;
import zombie.iso.worldgen.attachments.AttachmentsHandler;
import zombie.iso.worldgen.utils.Direction;
import zombie.iso.worldgen.utils.SquareCoord;

/** Full and partial attachment passes with an isolated nine-chunk neighbourhood. */
public final class WgAttachmentsProbe {
    static String[] pickFloors() {
        TreeMap<String, String> floors = new TreeMap<>();
        List<String> names = new ArrayList<>(IsoSpriteManager.instance.namedMap.keySet());
        Collections.sort(names);
        for (String name : names) {
            IsoSprite sprite = IsoSpriteManager.instance.namedMap.get(name);
            if (sprite == null || !sprite.getProperties().has(IsoFlagType.solidfloor)) continue;
            String material = sprite.getProperties().get("FloorMaterial");
            if (material != null && !material.equals("None") && !material.equals("null")) {
                floors.putIfAbsent(material, name);
            }
        }
        if (floors.size() < 2) throw new IllegalStateException("expected at least two distinct floor materials");
        var keys = floors.keySet().iterator();
        String materialA = keys.next();
        String materialB = keys.next();
        return new String[]{materialA, floors.get(materialA), materialB, floors.get(materialB)};
    }

    @SuppressWarnings("unchecked")
    static Map<String, Map<Direction, List<String>>> registered(AttachmentsHandler handler) throws Exception {
        Field f = AttachmentsHandler.class.getDeclaredField("attachments");
        f.setAccessible(true);
        return (Map<String, Map<Direction, List<String>>>)f.get(handler);
    }

    static void dump(PrintStream out, String label, WgAdvancedFixture fixture) {
        IsoChunk center = fixture.center();
        out.println(label + " STATE " + (center.isAttachmentsDoneFull() ? 1 : 0)
            + " " + (center.isAttachmentsDone(0) ? 1 : 0)
            + " " + (center.isAttachmentsDone(1) ? 1 : 0)
            + " " + (center.isAttachmentsDone(2) ? 1 : 0)
            + " " + (center.isAttachmentsDone(3) ? 1 : 0)
            + " " + (center.isAttachmentsDone(4) ? 1 : 0)
            + " PARTIAL " + center.attachmentsPartialSize());
        for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) {
            out.println("C " + x + " " + y + WgAdvancedFixture.objects(center.getGridSquare(x, y, 0)));
        }
        for (int y = 0; y < 8; y++) {
            out.println("E 0 " + y + WgAdvancedFixture.objects(fixture.neighbour(1, 0).getGridSquare(0, y, 0)));
        }
    }

    static void run(PrintStream out) throws Exception {
        String[] floors = pickFloors();
        out.println("MATERIALS " + floors[0] + " " + floors[1] + " " + floors[2] + " " + floors[3]);
        WgAdvancedFixture fixture = new WgAdvancedFixture(floors[1]);
        for (int y = 0; y < 8; y++) for (int x = 0; x < 8; x++) {
            fixture.setFloor(fixture.neighbour(1, 0), x, y, floors[3]);
        }
        AttachmentsHandler handler = new AttachmentsHandler();
        handler.loadAttachments();
        Map<String, Map<Direction, List<String>>> attachments = registered(handler);
        int total = 0;
        for (var dirs : attachments.values()) for (var tiles : dirs.values()) total += tiles.size();
        out.println("CATALOG " + attachments.size() + " " + total);
        // A controlled, unique overlay avoids unrelated global sprite HashMap collision orders.
        attachments.clear();
        IsoSprite overlay = IsoSpriteManager.instance.getSprite("oracle_floor_attachment_w");
        overlay.name = "oracle_floor_attachment_w";
        overlay.getProperties().set(IsoFlagType.IsFloorAttached);
        overlay.getProperties().set(IsoFlagType.FloorAttachmentW);
        Map<Direction, List<String>> choices = new EnumMap<>(Direction.class);
        choices.put(Direction.WEST, List.of("oracle_floor_attachment_w"));
        attachments.put(floors[0], choices);

        IsoChunk center = fixture.center();
        handler.resetAttachments(center);
        int missingX = fixture.neighbour(-1, -1).wx;
        int missingY = fixture.neighbour(-1, -1).wy;
        IsoChunkMap map = fixture.cell.chunkMap[0];
        int w = IsoChunkMap.chunkGridWidth;
        int slot = (missingY - map.getWorldYMin()) * w + (missingX - map.getWorldXMin());
        IsoChunk displaced = map.getChunks()[slot];
        map.getChunks()[slot] = null;
        handler.applyAttachments(center);
        dump(out, "MISSING", fixture);
        map.getChunks()[slot] = displaced;

        handler.applyAttachments(center);
        dump(out, "FULL", fixture);

        handler.resetAttachments(center);
        center.setAttachmentsPartial(new SquareCoord(center.wx * 8 + 7, center.wy * 8 + 3, 0));
        // The partial pass must remove an existing WEST overlay on the adjacent floor.
        fixture.neighbour(1, 0).getGridSquare(0, 3, 0).getFloor().addAttachedAnimSprite(overlay);
        handler.applyAttachments(center);
        dump(out, "PARTIAL", fixture);
    }

    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgAttachmentsProbe::run);
    }
}
