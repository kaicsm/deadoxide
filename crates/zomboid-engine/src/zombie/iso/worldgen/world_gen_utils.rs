//! zombie.iso.worldgen.WorldGenUtils: helpers of the procedural world generation. `media/lua/server/WorldGen/
//! Features.lua` and `Biomes.lua` list their data files with getFiles/getFilesNum/getFile and require each.
use std::path::{Path, PathBuf};

/// zombie.iso.worldgen.WorldGenUtils (one instance: WorldGenUtils.INSTANCE).
#[derive(Debug, Default)]
pub struct WorldGenUtils {
    pub files: Vec<String>,
}

/// java.nio.file.Files.walk without FOLLOW_LINKS: the start, then depth first, each directory's entries in the
/// order the file system lists them (a directory is entered when met). Errors below the start are not ported
/// (the original throws UncheckedIOException out of the stream).
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        out.push(p.clone());
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&p, out);
        }
    }
}

impl WorldGenUtils {
    /// zombie.iso.worldgen.WorldGenUtils.getFiles: the regular files under `media/<base_path>` whose path ends in
    /// "lua", relative to the game directory with '/' separators, replacing the list. A missing directory is an
    /// IOException the original logs (DebugType.General): returned here as the log line, the list left empty.
    pub fn get_files(&mut self, game_root: &Path, base_path: &str) -> Option<String> {
        self.files = Vec::new();
        let main_file = game_root.join("media").join(base_path);
        if !main_file.exists() {
            return Some(format!(
                "java.nio.file.NoSuchFileException: {}",
                main_file.to_string_lossy()
            ));
        }
        let mut paths = vec![main_file.clone()];
        walk(&main_file, &mut paths);
        for f in paths {
            // Files.isRegularFile follows links
            if !f.is_file() || !f.to_string_lossy().ends_with("lua") {
                continue;
            }
            if let Ok(rel) = f.strip_prefix(game_root) {
                self.files.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        None
    }

    /// zombie.iso.worldgen.WorldGenUtils.getFilesNum
    pub fn get_files_num(&self) -> i32 {
        self.files.len() as i32
    }

    /// zombie.iso.worldgen.WorldGenUtils.getFile: `files.get(i)` (IndexOutOfBoundsException outside the list).
    pub fn get_file(&self, i: i32) -> Option<&str> {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.files.get(i))
            .map(String::as_str)
    }
}
