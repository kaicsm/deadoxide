//! zombie.iso.worldgen.WorldGenParams: the world seed and generation bounds a save keeps in `map_worldgen.bin`.
use crate::java;
use crate::java_nio::{read_string, ByteBuffer};

/// zombie.iso.worldgen.WorldGenParams.FILE_MAGIC
const FILE_MAGIC: [u8; 4] = *b"WGEN";

/// zombie.iso.worldgen.WorldGenParams, the saved fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldGenParams {
    pub seed_string: String,
    /// `seedString.hashCode()`
    pub seed: i32,
    pub min_x_cell: i32,
    pub min_y_cell: i32,
    pub max_x_cell: i32,
    pub max_y_cell: i32,
}

impl Default for WorldGenParams {
    /// The field initialisers.
    fn default() -> Self {
        WorldGenParams {
            seed_string: String::new(),
            seed: 0,
            min_x_cell: -250,
            min_y_cell: -250,
            max_x_cell: 250,
            max_y_cell: 250,
        }
    }
}

impl WorldGenParams {
    /// zombie.iso.worldgen.WorldGenParams.setSeedString
    pub fn set_seed_string(&mut self, seed_string: &str) {
        self.seed_string = seed_string.to_string();
        self.seed = java::string_hash_code(seed_string);
    }

    /// zombie.iso.worldgen.WorldGenParams.getSeed (the int seed widened to long).
    pub fn get_seed(&self) -> i64 {
        i64::from(self.seed)
    }

    /// zombie.iso.worldgen.WorldGenParams.getRandom(int, int) (WorldGenParams.java:48-50):
    /// `return this.getRandom(wx, wy, 0L)`.
    pub fn get_random(&self, wx: i32, wy: i32) -> java::Random {
        self.get_random_with_offset(wx, wy, 0)
    }

    /// zombie.iso.worldgen.WorldGenParams.getRandom(int, int, long) (WorldGenParams.java:52-58):
    /// `Random tmpRnd = new Random(this.seed + offset)` — the int seed widens to long and the addition
    /// wraps — then `wxRnd = tmpRnd.nextLong()` and `wyRnd = tmpRnd.nextLong()`, and
    /// `tmpRnd.setSeed(wx * wxRnd ^ wy * wyRnd ^ this.seed)`: wx and wy widen to long for the wrapping
    /// products, the xor chain runs left to right, and the int seed widens for the final xor.
    pub fn get_random_with_offset(&self, wx: i32, wy: i32, offset: i64) -> java::Random {
        let mut tmp_rnd = java::Random::new(i64::from(self.seed).wrapping_add(offset));
        let wx_rnd = tmp_rnd.next_long();
        let wy_rnd = tmp_rnd.next_long();
        tmp_rnd.set_seed(
            (i64::from(wx).wrapping_mul(wx_rnd) ^ i64::from(wy).wrapping_mul(wy_rnd))
                ^ i64::from(self.seed),
        );
        tmp_rnd
    }

    /// zombie.iso.worldgen.WorldGenParams.load over the bytes of `map_worldgen.bin`: magic, world version, seed text,
    /// cell bounds. The original logs a broken file and keeps the fields it had set; this returns them the same way
    /// along with the error.
    pub fn load(data: &[u8]) -> (Self, Option<String>) {
        let mut p = WorldGenParams::default();
        let r = p.load_bytes(data);
        (p, r.err())
    }

    /// zombie.iso.worldgen.WorldGenParams.load as the singleton runs it on a server or single player (`GameClient.client`
    /// false): `None` is a missing file (`NOT_PRESENT`, nothing changes), any file is `DONE` with the fields it set
    /// before a failure; the failure is the second value.
    pub fn load_file(&mut self, data: Option<&[u8]>) -> (LoadResult, Option<String>) {
        match data {
            None => (LoadResult::NotPresent, None),
            Some(d) => (LoadResult::Done, self.load_bytes(d).err()),
        }
    }

    fn load_bytes(&mut self, data: &[u8]) -> Result<(), String> {
        let p = self;
        let mut b = ByteBuffer::wrap(data);
        (|| -> Result<(), String> {
            if b.get_bytes(4)? != FILE_MAGIC {
                return Err("java.io.IOException: does not appear to be map_worldgen.bin".into());
            }
            let _world_version = b.get_int()?;
            let seed = read_string(&mut b)?;
            p.set_seed_string(&seed);
            p.min_x_cell = b.get_int()?;
            p.min_y_cell = b.get_int()?;
            p.max_x_cell = b.get_int()?;
            p.max_y_cell = b.get_int()?;
            Ok(())
        })()
    }
}

/// zombie.iso.worldgen.WorldGenParams.Result (`CLIENT` is a network client's, never read from a file).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadResult {
    NotPresent,
    Done,
}
