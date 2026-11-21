import zombie.iso.weather.SimplexNoise;
import zombie.iso.worldgen.WorldGenSimplexGenerator;
import zombie.iso.worldgen.biomes.BiomeNoise;

public class WorldGenSimplexProbe {
  public static void main(String[] args) {
    double[] c = {-1048576.5, -300.25, -128.0, -1.75, -0.5, 0.0, 0.25, 1.0, 1.5, 63.75, 64.0, 127.999, 128.0, 300.0, 4096.5, 1048576.0};
    for (double x : c) for (double y : c) for (double z : c)
      System.out.println("SIMPLEX3 " + x + "," + y + "," + z + "=" + SimplexNoise.noise(x, y, z));
    long[] seeds = {1L, 42L, -1L, 2147483647L, Long.MIN_VALUE, 0x1234567890ABCDEFL};
    double[] w = {-300.25, -1.75, 0.0, 0.25, 1.0, 63.75, 128.0, 300.0};
    for (long seed : seeds) {
      WorldGenSimplexGenerator g = new WorldGenSimplexGenerator(seed);
      for (double x : w) for (double y : w) {
        BiomeNoise n = g.noise(x, y);
        System.out.println("WGSG " + seed + "," + x + "," + y + " landscape=" + n.landscape() + " plant=" + n.plant() + " bush=" + n.bush() + " temperature=" + n.temperature() + " hygrometry=" + n.hygrometry() + " ore=" + n.ore());
        System.out.println("WGSEL " + seed + "," + x + "," + y + "=" + g.selector(x, y));
      }
    }
  }
}
