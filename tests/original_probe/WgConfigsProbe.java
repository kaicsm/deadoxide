import java.io.PrintStream;
import java.util.List;
import java.util.Map;
import se.krka.kahlua.vm.KahluaTable;
import zombie.Lua.LuaManager;
import zombie.iso.worldgen.WorldGenReader;
import zombie.iso.worldgen.biomes.IBiome;
import zombie.iso.worldgen.biomes.TileGroup;
import zombie.iso.worldgen.maps.BiomeMapEntry;
import zombie.iso.worldgen.veins.OreVeinConfig;
import zombie.iso.worldgen.zones.AnimalsPathConfig;
import zombie.iso.worldgen.zombie.ZombieVoronoiEntry;
import zombie.iso.worldgen.zombie.ZombieVoronoiReader;

/** Oracle probe (tools/oracle.sh wgconfigs): the ORIGINAL WorldGenReader loaders the wgbiomes case left out -
 * loadStaticModules (with loadPrefab and the inline loadBiome), loadVeinsConfig, loadRoadConfig,
 * loadAnimalsPath, loadBiomeMapConfig - and the zombie_voronoi entries, over the same Lua state as wgbiomes
 * (LuaGameProbe.init: the client's startup headless, then LoadDirBase of shared and server, whose
 * media/lua/server/WorldGen/*.lua fills the `worldgen` table and the animals_path_config / biome_map_config /
 * zombie_voronoi globals). Printed, in this order (floats as raw bits f%08x, doubles d%016x, strings escaped,
 * int lists [a,b]; every map in its own java.util.HashMap iteration order):
 *   KEYS static_modules=<k> veins=<k> roads=<k>            which of the worldgen sub-tables exist
 *   STATIC n / SM i ...                                    every static module (bounds, the inline biome's
 *                                                          parent/gen/zombies, the prefab dims/tiles/schematic)
 *   PF i <category> rows...                                the prefab schematics, one line per category
 *   VEINS n / V <name> ...                                 every ore vein config (arms/center/feature tiles)
 *   ROADS n / R <name> ...                                 every road config
 *   ANIMALS nil|n / A i ...                                animals_path_config (nil when the global is missing)
 *   BIOME_MAP nil|n / M pixel ...                          biome_map_config in HashMap<Integer> iteration order
 *   ZVORONOI nil|n / Z i ...                               the zombie_voronoi entries
 * The feature tile lists print every TileGroup as sx x sy [names].
 * Compared with `pz-dump wgconfigs`.
 * Args: <scratch cache dir> ; run from the game root. */
public class WgConfigsProbe {
    static PrintStream out;

    /** The private field read (PrefabStructure exposes no getters for tiles/schematic). */
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

    static String esc(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') b.append(c); else b.append(String.format("\\u%04x", (int) c));
        }
        return b.append('"').toString();
    }

    static String d(double d) {
        return String.format("d%016x", Double.doubleToRawLongBits(d));
    }

    static String f(float f) {
        return String.format("f%08x", Float.floatToRawIntBits(f));
    }

    static String names(List<String> list) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < list.size(); i++) b.append(i == 0 ? "" : ",").append(esc(list.get(i)));
        return b.append(']').toString();
    }

    static String tiles(List<TileGroup> list) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < list.size(); i++) {
            TileGroup g = list.get(i);
            if (i > 0) b.append(',');
            b.append(g.sx()).append('x').append(g.sy()).append(' ').append(names(g.tiles()));
        }
        return b.append(']').toString();
    }

    static String ints(int[] list) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < list.length; i++) b.append(i == 0 ? "" : ",").append(list[i]);
        return b.append(']').toString();
    }

    static String rows(int[][] rows) {
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < rows.length; i++) {
            if (i > 0) b.append(',');
            b.append(ints(rows[i]));
        }
        return b.append(']').toString();
    }

    public static void main(String[] a) throws Exception {
        out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        LuaGameProbe.init(new String[]{a[0], "shared", "server"});
        KahluaTable worldgen = (KahluaTable)LuaManager.env.rawget("worldgen");
        if (worldgen == null) {
            System.err.println("ORACLE: worldgen table missing from the Lua environment");
            Runtime.getRuntime().halt(4);
        }
        WorldGenReader r = new WorldGenReader();

        out.println("KEYS static_modules=" + (worldgen.rawget("static_modules") == null ? "nil" : "table")
            + " veins=" + (worldgen.rawget("veins") == null ? "nil" : "table")
            + " roads=" + (worldgen.rawget("roads") == null ? "nil" : "table")
            + " animals=" + (LuaManager.env.rawget("animals_path_config") == null ? "nil" : "table")
            + " biome_map=" + (LuaManager.env.rawget("biome_map_config") == null ? "nil" : "table")
            + " zombie_voronoi=" + (LuaManager.env.rawget("zombie_voronoi") == null ? "nil" : "table"));

        // static modules
        List<zombie.iso.worldgen.StaticModule> modules = r.loadStaticModules(worldgen, "static_modules");
        out.println("STATIC " + modules.size());
        for (int i = 0; i < modules.size(); i++) {
            zombie.iso.worldgen.StaticModule m = modules.get(i);
            out.println("SM " + (i + 1) + " " + m.xmin() + " " + m.xmax() + " " + m.ymin() + " " + m.ymax());
            if (m.biome() == null) {
                out.println("SM " + (i + 1) + " BIOME nil");
            } else {
                IBiome b = m.biome();
                out.println("SM " + (i + 1) + " BIOME parent=" + (b.parent() == null ? "nil" : esc(b.parent())) + " gen=" + (b.generate() ? 1 : 0) + " zombies=" + f(b.zombies()));
            }
            if (m.prefab() == null) {
                out.println("SM " + (i + 1) + " PREFAB nil");
            } else {
                zombie.iso.worldgen.PrefabStructure p = m.prefab();
                List<String> pTiles = (List<String>)g(p, "tiles");
                Map<String, int[][]> pSchem = (Map<String, int[][]>)g(p, "schematic");
                out.println("SM " + (i + 1) + " PREFAB " + p.getX() + "x" + p.getY() + " tiles=" + pTiles.size() + " zombies=" + f(p.getZombies()) + " cats=" + p.getCategories());
                for (int j = 0; j < pTiles.size(); j++) out.println("PT " + (i + 1) + " " + (j + 1) + " " + esc(pTiles.get(j)));
                for (Map.Entry<String, int[][]> e : pSchem.entrySet()) out.println("PF " + (i + 1) + " " + esc(e.getKey()) + " " + rows(e.getValue()));
            }
        }

        // veins
        Map<String, OreVeinConfig> veins = r.loadVeinsConfig(worldgen, "veins");
        out.println("VEINS " + veins.size());
        for (Map.Entry<String, OreVeinConfig> e : veins.entrySet()) {
            OreVeinConfig c = e.getValue();
            out.println("V " + esc(e.getKey()) + " cr=" + c.getCenterRadius() + " cp=" + f(c.getCenterProb()) + " amin=" + c.getArmsAmountMin() + " amax=" + c.getArmsAmountMax() + " dmin=" + c.getArmsDistMin() + " dmax=" + c.getArmsDistMax() + " delta=" + c.getArmsDeltaAngle() + " ap=" + f(c.getArmsProb()) + " p=" + f(c.getProbability()));
            out.println("VT " + esc(e.getKey()) + " " + tiles(c.getTiles()));
        }

        // roads
        Map<String, zombie.iso.worldgen.roads.RoadConfig> roads = r.loadRoadConfig(worldgen, "roads");
        out.println("ROADS " + roads.size());
        for (Map.Entry<String, zombie.iso.worldgen.roads.RoadConfig> e : roads.entrySet()) {
            zombie.iso.worldgen.roads.RoadConfig c = e.getValue();
            out.println("R " + esc(e.getKey()) + " proba=" + d(c.probaRoads()) + " p=" + d(c.probability()) + " filter=" + d(c.filter()));
            out.println("RT " + esc(e.getKey()) + " " + tiles(c.tiles()));
        }

        // animals paths
        KahluaTable animalsTable = (KahluaTable)LuaManager.env.rawget("animals_path_config");
        if (animalsTable == null) {
            out.println("ANIMALS nil");
        } else {
            List<AnimalsPathConfig> animals = r.loadAnimalsPath(animalsTable);
            out.println("ANIMALS " + animals.size());
            for (int i = 0; i < animals.size(); i++) {
                AnimalsPathConfig c = animals.get(i);
                out.println("A " + (i + 1) + " " + esc(c.animalType()) + " count=" + c.count() + " chance=" + f(c.chance()) + " points=" + ints(c.points()) + " radius=" + ints(c.radius()) + " extension=" + ints(c.extension()) + " extchance=" + f(c.extensionChance()) + " hash=" + c.getNameHash());
            }
        }

        // biome map
        KahluaTable biomeMapTable = (KahluaTable)LuaManager.env.rawget("biome_map_config");
        if (biomeMapTable == null) {
            out.println("BIOME_MAP nil");
        } else {
            Map<Integer, BiomeMapEntry> biomeMap = r.loadBiomeMapConfig(biomeMapTable);
            out.println("BIOME_MAP " + biomeMap.size());
            for (Map.Entry<Integer, BiomeMapEntry> e : biomeMap.entrySet()) {
                BiomeMapEntry m = e.getValue();
                out.println("M " + m.pixel() + " " + (m.biome() == null ? "nil" : esc(m.biome())) + " " + (m.ore() == null ? "nil" : esc(m.ore())) + " " + (m.zone() == null ? "nil" : esc(m.zone())));
            }
        }

        // zombie voronoi
        KahluaTable zvoronoiTable = (KahluaTable)LuaManager.env.rawget("zombie_voronoi");
        if (zvoronoiTable == null) {
            out.println("ZVORONOI nil");
        } else {
            List<ZombieVoronoiEntry> entries = new ZombieVoronoiReader().getEntries(zvoronoiTable);
            out.println("ZVORONOI " + entries.size());
            for (int i = 0; i < entries.size(); i++) {
                ZombieVoronoiEntry e = entries.get(i);
                out.println("Z " + (i + 1) + " points=" + e.numberPoints() + " closest=" + e.closestPoint() + " scale=" + d(e.scale()) + " cutoff=" + d(e.cutoff()));
            }
        }

        out.flush();
    }
}
