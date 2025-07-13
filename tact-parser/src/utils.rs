use crate::jenkins3::hashlittle2;

/// Perform a [`HashPath`][0] with [`hashlittle2`][] (aka: jenkins3).
///
/// This normalises `path` using the same rules as [`SStrHash`][1], and then
/// merges the two `u32`s of [`hashlittle2`][] into a `u64`, with `pc` as the
/// high bytes.
///
/// [0]: https://wowdev.wiki/TACT#hashpath
/// [1]: https://wowdev.wiki/SStrHash
pub fn jenkins3_hashpath(path: &str) -> u64 {
    let normalised = path.to_ascii_uppercase().replace('/', "\\");
    let mut pc = 0;
    let mut pb = 0;
    hashlittle2(normalised.as_bytes(), &mut pc, &mut pb);

    (u64::from(pc) << 32) | u64::from(pb)
}
