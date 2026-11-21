import java.util.HashMap;
import java.util.Map;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoChunkMap;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoObject;
import zombie.iso.IsoWorld;
import zombie.iso.sprite.IsoSprite;
import zombie.iso.sprite.IsoSpriteManager;
import zombie.network.ServerMap;

/** Owned 3x3 probe chunks registered in the original server's lookup tables. */
final class WgAdvancedFixture {
    static final int CX = 1034;
    static final int CY = 1458;
    final IsoChunk[][] chunks = new IsoChunk[3][3];
    final IsoCell cell;

    WgAdvancedFixture(String floorSprite) {
        cell = IsoWorld.instance.currentCell;
        if (cell == null || cell.chunkMap[0] == null) {
            throw new IllegalStateException("world/first chunk map not initialized");
        }
        IsoSprite sprite = IsoSpriteManager.instance.getSprite(floorSprite);
        for (int dx = -1; dx <= 1; dx++) {
            for (int dy = -1; dy <= 1; dy++) {
                int wx = CX + dx;
                int wy = CY + dy;
                IsoChunk chunk = new IsoChunk(cell);
                chunk.wx = wx;
                chunk.wy = wy;
                chunk.assignLoadID();
                chunk.setMinMaxLevel(0, 0);
                for (int x = 0; x < 8; x++) {
                    for (int y = 0; y < 8; y++) {
                        IsoGridSquare sq = IsoGridSquare.getNew(cell, null, wx * 8 + x, wy * 8 + y, 0);
                        chunk.setSquare(x, y, 0, sq);
                        sq.getObjects().add(new IsoObject(cell, sq, sprite));
                    }
                }
                if (chunk.getGridSquare(0, 0, 0).getFloor() == null) {
                    throw new IllegalStateException("fixture sprite is not a floor: " + floorSprite);
                }
                chunk.loaded = true;
                chunks[dx + 1][dy + 1] = chunk;
            }
        }
        register();
    }

    IsoChunk center() { return chunks[1][1]; }
    IsoChunk neighbour(int dx, int dy) { return chunks[dx + 1][dy + 1]; }

    void register() {
        ServerMap map = ServerMap.instance;
        IsoChunkMap chunkMap = cell.chunkMap[0];
        chunkMap.ignore = false;
        chunkMap.setInitialPos(CX, CY);
        int w = IsoChunkMap.chunkGridWidth;
        IsoChunk[] mapChunks = chunkMap.getChunks();
        int minWX = chunkMap.getWorldXMin();
        int minWY = chunkMap.getWorldYMin();
        for (int dx = -1; dx <= 1; dx++) {
            for (int dy = -1; dy <= 1; dy++) {
                IsoChunk chunk = neighbour(dx, dy);
                int x = chunk.wx - minWX;
                int y = chunk.wy - minWY;
                if (x < 0 || y < 0 || x >= w || y >= w) {
                    throw new IllegalStateException("chunk map too small for 3x3 fixture");
                }
                mapChunks[y * w + x] = chunk;
                int scWX = Math.floorDiv(chunk.wx, 8);
                int scWY = Math.floorDiv(chunk.wy, 8);
                int sx = scWX - map.getMinX();
                int sy = scWY - map.getMinY();
                if (map.isInvalidCell(sx, sy)) throw new IllegalStateException("fixture outside server map");
                int index = sy * (map.getMaxX() - map.getMinX() + 1) + sx;
                ServerMap.ServerCell sc = map.cellMap[index];
                if (sc == null) {
                    sc = new ServerMap.ServerCell();
                    sc.wx = scWX;
                    sc.wy = scWY;
                    map.cellMap[index] = sc;
                }
                sc.isLoaded = true;
                sc.chunks[Math.floorMod(chunk.wx, 8)][Math.floorMod(chunk.wy, 8)] = chunk;
            }
        }
        if (map.getChunk(CX, CY) != center()) throw new IllegalStateException("server-map fixture registration failed");
        if (cell.getChunk(CX, CY) != center()) throw new IllegalStateException("cell chunk-map fixture registration failed");
    }

    void setFloor(IsoChunk chunk, int x, int y, String name) {
        IsoGridSquare square = chunk.getGridSquare(x, y, 0);
        IsoSprite sprite = IsoSpriteManager.instance.getSprite(name);
        IsoObject floor = square.getFloor();
        if (floor == null) throw new IllegalStateException("missing fixture floor");
        floor.setSprite(sprite);
    }

    static String objects(IsoGridSquare square) {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < square.getObjects().size(); i++) {
            IsoObject obj = square.getObjects().get(i);
            String name = obj.getSprite() == null ? "null" : String.valueOf(obj.getSprite().getName());
            s.append(" | ").append(obj.getClass().getSimpleName()).append(' ').append(name);
            if (obj.getAttachedAnimSprite() != null) {
                for (var attached : obj.getAttachedAnimSprite()) {
                    s.append(" + ").append(attached.getParentSprite().getName());
                }
            }
        }
        return s.toString();
    }
}
