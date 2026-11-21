import java.io.PrintStream;
import se.krka.kahlua.vm.KahluaTable;
import zombie.Lua.LuaManager;
import zombie.iso.worldgen.zones.WorldGenZone;
import zombie.iso.zones.ZoneGeometryType;

/** The WorldGen-specific Rocks property and Zone.contains shape/border semantics. */
public final class WgSpecialZonesProbe {
    static void run(PrintStream out) {
        KahluaTable off = LuaManager.platform.newTable();
        off.rawset("Rocks", Boolean.FALSE);
        KahluaTable on = LuaManager.platform.newTable();
        on.rawset("Rocks", Boolean.TRUE);
        WorldGenZone[] zones = {
            new WorldGenZone("default", "WorldGen", -16, -16, 0, 8, 8, null),
            new WorldGenZone("off", "WorldGen", -4, 6, 0, 8, 8, off),
            new WorldGenZone("on", "WorldGen", -4, 6, 1, 8, 8, on),
            new WorldGenZone("polygon", "WorldGen", 10, 10, 0, 8, 8, off)
        };
        zones[3].geometryType = ZoneGeometryType.Polygon;
        for (int v : new int[] {10,10, 17,10, 17,17, 10,17}) zones[3].points.add(v);
        for (int zi = 0; zi < zones.length; zi++) {
            WorldGenZone z = zones[zi];
            out.println("ZONE " + zi + " rocks=" + (z.getRocks()?1:0));
            for (int x : new int[] {-17,-16,-15,-9,-8,-5,-4,-1,0,3,4,9,10,11,15,17,18}) {
                for (int y : new int[] {-17,-16,-9,-8,5,6,7,13,14,17,18}) {
                    for (int level : new int[] {0, 1}) {
                        if (z.contains(x, y, level)) out.println("HIT " + zi + " " + x + " " + y + " " + level);
                    }
                }
            }
        }
    }
    public static void main(String[] args) throws Exception {
        ServerHarness.run(args[0], WgSpecialZonesProbe::run);
    }
}
