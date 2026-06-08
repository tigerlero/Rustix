//! Asset cooking pipeline: preprocess source assets into optimized runtime formats.
//!
//! `AssetCooker` walks a source asset tree, runs each file through the
//! appropriate `Importer` + optimization pipeline, and writes `.rx*`
//! native binaries to a cooked output directory.  A `CookJob` describes
//! one file → one cooked asset.  The cooker uses the existing `DiskCache`
//! infrastructure to skip unchanged files, enabling incremental builds.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::cache::DiskCache;
use crate::dependency_graph::DependencyGraph;
use crate::importer::Importer;
use crate::mesh::{MeshAsset, export_rxmesh};
use crate::material::{MaterialAsset, export_rxmat};
use crate::texture::{TextureAsset, export_rxtex};
use crate::animation::{AnimationAsset, export_rxanim};
use crate::skeleton::{SkeletonAsset, export_rxskel};

/// A single cooking task: source path → cooked path.
#[derive(Debug, Clone)]
pub struct CookJob {
    pub source: PathBuf,
    pub output: PathBuf,
    pub kind: CookKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookKind {
    Mesh,
    Material,
    Texture,
    Animation,
    Skeleton,
    Generic,
}

impl CookKind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "gltf" | "glb" | "obj" | "fbx" => CookKind::Mesh,
            "png" | "jpg" | "jpeg" | "hdr" | "ktx2" | "dds" => CookKind::Texture,
            "mat.ron" | "mat.json" | "rxmat" => CookKind::Material,
            "anim.ron" | "rxanim" => CookKind::Animation,
            "skel.ron" | "rxskel" => CookKind::Skeleton,
            _ => CookKind::Generic,
        }
    }

    pub fn cooked_extension(&self) -> &'static str {
        match self {
            CookKind::Mesh => "rxmesh",
            CookKind::Material => "rxmat",
            CookKind::Texture => "rxtex",
            CookKind::Animation => "rxanim",
            CookKind::Skeleton => "rxskel",
            CookKind::Generic => "rxcooked",
        }
    }
}

/// Result of a single cook job.
#[derive(Debug, Clone)]
pub struct CookResult {
    pub source: PathBuf,
    pub output: PathBuf,
    pub success: bool,
    pub error: Option<String>,
    pub bytes_written: usize,
}

/// Incremental asset cooker.
///
/// ```rust
/// let cooker = AssetCooker::new("./assets/src", "./assets/cooked");
/// cooker.cook_all()?;
/// ```
pub struct AssetCooker {
    pub source_root: PathBuf,
    pub output_root: PathBuf,
    /// Cache used to skip unchanged files.
    pub cache: DiskCache,
    /// Dependency graph for transitive invalidation.
    pub graph: DependencyGraph,
    graph_path: PathBuf,
    /// Registered importers by extension.
    importers: HashMap<String, Box<dyn Fn(&[u8], Option<&str>) -> Result<Vec<u8>, String> + Send>>,
}

impl AssetCooker {
    pub fn new(source_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let source_root = source_root.into();
        let output_root = output_root.into();
        std::fs::create_dir_all(&output_root)?;
        let cache = DiskCache::new(output_root.join(".cache"))?;
        let graph_path = output_root.join(".deps.json");
        let graph = if graph_path.exists() {
            DependencyGraph::load(&graph_path).unwrap_or_default()
        } else {
            DependencyGraph::new()
        };
        Ok(Self {
            source_root,
            output_root,
            cache,
            graph,
            graph_path,
            importers: HashMap::new(),
        })
    }

    /// Register a boxed cook function for a file extension.
    ///
    /// The function receives the raw source bytes and an optional import hint
    /// (usually the file stem), and returns the cooked binary bytes.
    pub fn register<F>(&mut self, extension: impl Into<String>, cook_fn: F)
    where
        F: Fn(&[u8], Option<&str>) -> Result<Vec<u8>, String> + Send + 'static,
    {
        self.importers.insert(extension.into(), Box::new(cook_fn));
    }

    /// Scan `source_root` recursively and build a list of cook jobs.
    pub fn scan(&self) -> Vec<CookJob> {
        let mut jobs = Vec::new();
        self.scan_dir(&self.source_root, &mut jobs);
        jobs
    }

    fn scan_dir(&self, dir: &Path, jobs: &mut Vec<CookJob>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.scan_dir(&path, jobs);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let rel = path.strip_prefix(&self.source_root).unwrap_or(&path);
                let kind = CookKind::from_extension(ext);
                let mut out = self.output_root.join(rel);
                out.set_extension(kind.cooked_extension());
                jobs.push(CookJob {
                    source: path,
                    output: out,
                    kind,
                });
            }
        }
    }

    /// Cook a single job, skipping if the cache says the source is unchanged.
    pub fn cook_one(&mut self, job: &CookJob) -> CookResult {
        let source_bytes = match std::fs::read(&job.source) {
            Ok(b) => b,
            Err(e) => {
                return CookResult {
                    source: job.source.clone(),
                    output: job.output.clone(),
                    success: false,
                    error: Some(format!("read failed: {e}")),
                    bytes_written: 0,
                };
            }
        };

        let ext = job
            .source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // If the importer is registered, use it; otherwise pass-through.
        let cooked = if let Some(importer) = self.importers.get(&ext) {
            let hint = job.source.file_stem().and_then(|s| s.to_str());
            match importer(&source_bytes, hint) {
                Ok(bytes) => bytes,
                Err(e) => {
                    return CookResult {
                        source: job.source.clone(),
                        output: job.output.clone(),
                        success: false,
                        error: Some(e),
                        bytes_written: 0,
                    };
                }
            }
        } else {
            // Pass-through: source bytes are already cooked (e.g. .rxmesh)
            source_bytes.clone()
        };

        // Write cooked file.
        if let Some(parent) = job.output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let result = match std::fs::write(&job.output, &cooked) {
            Ok(()) => {
                let _ = self.cache.write(&job.source, &cooked);
                CookResult {
                    source: job.source.clone(),
                    output: job.output.clone(),
                    success: true,
                    error: None,
                    bytes_written: cooked.len(),
                }
            }
            Err(e) => CookResult {
                source: job.source.clone(),
                output: job.output.clone(),
                success: false,
                error: Some(format!("write failed: {e}")),
                bytes_written: 0,
            },
        };

        // Update dependency graph for materials so incremental builds
        // know which assets depend on which textures.
        if result.success && job.kind == CookKind::Material {
            if let Ok(mat) = crate::importer::import_ron::<MaterialAsset>(&source_bytes) {
                let deps: Vec<PathBuf> = mat.texture_dependencies()
                    .iter()
                    .map(|p| self.source_root.join(p))
                    .collect();
                self.graph.set_dependencies(&job.source, &deps);
            }
        }

        result
    }

    /// Cook every job from `scan()`, returning per-job results.
    ///
    /// Also persists the dependency graph to disk so that future incremental
    /// cooks can perform transitive invalidation.
    pub fn cook_all(&mut self) -> Vec<CookResult> {
        let jobs = self.scan();
        let results: Vec<CookResult> = jobs.iter().map(|j| self.cook_one(j)).collect();
        let _ = self.graph.save(&self.graph_path);
        results
    }

    /// Cook incrementally: only re-cook files that changed or whose
    /// dependencies changed (transitively).
    ///
    /// 1. Find directly stale jobs (cache miss or missing output).
    /// 2. Find jobs whose direct dependencies are stale.
    /// 3. Find all transitive dependents of stale sources via the graph.
    /// 4. Cook the union.
    /// 5. Persist the updated graph.
    pub fn cook_incremental(&mut self) -> Vec<CookResult> {
        let jobs = self.scan();

        // 1. Directly stale sources
        let mut stale: HashSet<PathBuf> = HashSet::new();
        for job in &jobs {
            if !self.cache.is_cached(&job.source) || !job.output.exists() {
                stale.insert(job.source.clone());
            }
        }

        // 2. Jobs whose direct dependencies are stale
        let mut changed = stale.clone();
        for job in &jobs {
            for dep in self.graph.dependencies_of(&job.source) {
                if stale.contains(dep) {
                    changed.insert(job.source.clone());
                    break;
                }
            }
        }

        // 3. Transitive dependents
        let mut cook_set = changed.clone();
        for path in &changed {
            for dependent in self.graph.transitive_dependents(path) {
                cook_set.insert(dependent);
            }
        }

        // 4. Cook
        let results: Vec<CookResult> = jobs
            .iter()
            .filter(|j| cook_set.contains(&j.source))
            .map(|j| self.cook_one(j))
            .collect();

        // 5. Persist graph
        let _ = self.graph.save(&self.graph_path);

        results
    }

    /// Remove all cooked files, clear the cache, and delete the dependency graph.
    pub fn clean(&self) -> std::io::Result<()> {
        if self.output_root.exists() {
            std::fs::remove_dir_all(&self.output_root)?;
            std::fs::create_dir_all(&self.output_root)?;
        }
        self.cache.clear()?;
        if self.graph_path.exists() {
            std::fs::remove_file(&self.graph_path)?;
        }
        Ok(())
    }
}

// ── Convenience cook functions ──

/// Cook a glTF/GLB mesh through the full optimization pipeline.
pub fn cook_mesh(source: &[u8], _hint: Option<&str>) -> Result<Vec<u8>, String> {
    use crate::mesh::import_gltf;
    use crate::mesh_opt::optimize_full;

    let mesh = import_gltf(source)?;
    let optimized = optimize_full(&mesh, 1.05);
    Ok(export_rxmesh(&optimized))
}

/// Cook a PNG texture through the texture compressor (BC7 sRGB).
pub fn cook_texture_bc7(source: &[u8], _hint: Option<&str>) -> Result<Vec<u8>, String> {
    use crate::texture::import_png;
    use crate::texture_compress::{TextureCompressor, CompressedBlockFormat};

    let tex = import_png(source)?;
    let compressed = TextureCompressor::compress(&tex, CompressedBlockFormat::BC7_UNORM_SRGB)
        .map_err(|e| format!("compress: {e}"))?;
    // Write as a minimal wrapper: width, height, format, mip_levels, block_data
    let mut out = Vec::new();
    out.extend_from_slice(&compressed.width.to_le_bytes());
    out.extend_from_slice(&compressed.height.to_le_bytes());
    out.extend_from_slice(&(compressed.format as u32).to_le_bytes());
    out.extend_from_slice(&compressed.mip_levels.to_le_bytes());
    out.extend_from_slice(&compressed.data);
    Ok(out)
}

/// Cook a material (pass-through RON → rxmat).
pub fn cook_material(source: &[u8], _hint: Option<&str>) -> Result<Vec<u8>, String> {
    use crate::importer::import_ron;
    use crate::material::MaterialAsset;

    let mat: MaterialAsset = import_ron(source)?;
    Ok(export_rxmat(&mat))
}

/// Cook an animation (pass-through rxanim).
pub fn cook_animation(source: &[u8], _hint: Option<&str>) -> Result<Vec<u8>, String> {
    use crate::animation::import_rxanim;

    let anim = import_rxanim(source)?;
    Ok(export_rxanim(&anim))
}

/// Cook a skeleton (pass-through rxskel).
pub fn cook_skeleton(source: &[u8], _hint: Option<&str>) -> Result<Vec<u8>, String> {
    use crate::skeleton::import_rxskel;

    let skel = import_rxskel(source)?;
    Ok(export_rxskel(&skel))
}
