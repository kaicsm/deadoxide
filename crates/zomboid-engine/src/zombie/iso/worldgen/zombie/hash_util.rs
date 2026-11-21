//! zombie.iso.worldgen.zombie.HashUtil: the 2D integer hash that seeds each Voronoi section's Random.

/// zombie.iso.worldgen.zombie.HashUtil.X_PRIME.
pub const X_PRIME: i32 = 1619;
/// zombie.iso.worldgen.zombie.HashUtil.Y_PRIME.
pub const Y_PRIME: i32 = 31337;
/// zombie.iso.worldgen.zombie.HashUtil.Z_PRIME.
pub const Z_PRIME: i32 = 6971;
/// zombie.iso.worldgen.zombie.HashUtil.W_PRIME.
pub const W_PRIME: i32 = 1013;

/// HashUtil.hash2D(long, long, long): `long hash = seed ^ 1619L * x; hash ^= 31337L * y; return
/// (int)finalizeHash(hash);` - the long products (the original inlines the X_PRIME and Y_PRIME values
/// as literals, it does not read the constants) wrap, and the `int` return is the truncation of the
/// finalized hash.
pub fn hash2d(seed: i64, x: i64, y: i64) -> i32 {
    let mut hash = seed ^ 1619i64.wrapping_mul(x);
    hash ^= 31337i64.wrapping_mul(y);
    finalize_hash(hash) as i32
}

/// HashUtil.finalizeHash(long): `hash = hash * hash * hash * 60493L; return hash >> 13 ^ hash;` - the
/// three products wrap (long arithmetic), `>>` is the arithmetic shift of the (possibly negative)
/// long, and the return its xor with the shifted value.
fn finalize_hash(hash: i64) -> i64 {
    let hash = hash
        .wrapping_mul(hash)
        .wrapping_mul(hash)
        .wrapping_mul(60493);
    (hash >> 13) ^ hash
}
