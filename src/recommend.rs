//! Setup recommendation facts and classes.

use std::{fs, process::Command};

/// One binary GiB.
const GIB: u64 = 1024 * 1024 * 1024;
/// One binary MiB.
const MIB: u64 = 1024 * 1024;

/// Hardware facts used for setup recommendation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HardwareFacts {
    /// Compile target OS.
    os: &'static str,
    /// Compile target arch.
    arch: &'static str,
    /// Total RAM bytes, when known.
    total_ram_bytes: Option<u64>,
    /// NVIDIA VRAM bytes, when known.
    nvidia_vram_bytes: Option<u64>,
}

impl HardwareFacts {
    /// Build facts from injected values.
    pub(crate) const fn new(
        os: &'static str,
        arch: &'static str,
        total_ram_bytes: Option<u64>,
        nvidia_vram_bytes: Option<u64>,
    ) -> Self {
        Self {
            os,
            arch,
            total_ram_bytes,
            nvidia_vram_bytes,
        }
    }

    /// Build facts for compile target.
    #[cfg(test)]
    pub(crate) const fn current_platform() -> Self {
        Self::new(std::env::consts::OS, std::env::consts::ARCH, None, None)
    }
}

/// Collect facts from current host.
pub(crate) fn collect_hardware_facts() -> HardwareFacts {
    HardwareFacts::new(
        std::env::consts::OS,
        std::env::consts::ARCH,
        collect_total_ram_bytes(),
        collect_nvidia_vram_bytes(),
    )
}

/// Collect total RAM by host OS.
fn collect_total_ram_bytes() -> Option<u64> {
    match std::env::consts::OS {
        "linux" => collect_linux_total_ram_bytes(),
        "macos" => collect_macos_total_ram_bytes(),
        _ => None,
    }
}

/// Read Linux RAM from `/proc/meminfo`.
fn collect_linux_total_ram_bytes() -> Option<u64> {
    let meminfo = fs::read("/proc/meminfo").ok()?;
    parse_linux_mem_total_bytes(&meminfo)
}

/// Read macOS RAM from `sysctl`.
fn collect_macos_total_ram_bytes() -> Option<u64> {
    let output = Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_macos_memsize_bytes(&output.stdout)
}

/// Read NVIDIA VRAM from `nvidia-smi`.
fn collect_nvidia_vram_bytes() -> Option<u64> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_nvidia_smi_vram_bytes(&output.stdout)
}

/// Parse Linux `MemTotal` KiB.
fn parse_linux_mem_total_bytes(meminfo: &[u8]) -> Option<u64> {
    for line in String::from_utf8_lossy(meminfo).lines() {
        let Some(rest) = line.strip_prefix("MemTotal:") else {
            continue;
        };
        let mut fields = rest.split_whitespace();
        let kib = fields.next()?.parse::<u64>().ok()?;

        if fields.next()? != "kB" || fields.next().is_some() {
            return None;
        }

        return kib.checked_mul(1024);
    }

    None
}

/// Parse macOS `hw.memsize` bytes.
fn parse_macos_memsize_bytes(output: &[u8]) -> Option<u64> {
    std::str::from_utf8(output).ok()?.trim().parse::<u64>().ok()
}

/// Parse NVIDIA VRAM MiB rows.
fn parse_nvidia_smi_vram_bytes(output: &[u8]) -> Option<u64> {
    std::str::from_utf8(output)
        .ok()?
        .lines()
        .filter_map(|line| {
            let mib = line.trim().parse::<u64>().ok()?;
            mib.checked_mul(MIB)
        })
        .max()
}

/// Hardware class for model recommendation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HardwareClass {
    /// Hardware facts missing.
    Unknown,
    /// CPU-only with small RAM.
    CpuSmall,
    /// CPU-only with standard RAM.
    CpuStandard,
    /// Apple Silicon with small unified RAM.
    AppleSiliconSmall,
    /// Apple Silicon with medium unified RAM.
    AppleSiliconMedium,
    /// Apple Silicon with large unified RAM.
    AppleSiliconLarge,
    /// NVIDIA GPU below 8 GiB VRAM.
    NvidiaUnder8Gb,
    /// NVIDIA GPU with 8 GiB class VRAM.
    Nvidia8Gb,
    /// NVIDIA GPU with 16 GiB class VRAM.
    Nvidia16Gb,
    /// NVIDIA GPU with 24 GiB or more VRAM.
    Nvidia24GbPlus,
}

impl HardwareClass {
    /// Stable `snake_case` tag.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::CpuSmall => "cpu_small",
            Self::CpuStandard => "cpu_standard",
            Self::AppleSiliconSmall => "apple_silicon_small",
            Self::AppleSiliconMedium => "apple_silicon_medium",
            Self::AppleSiliconLarge => "apple_silicon_large",
            Self::NvidiaUnder8Gb => "nvidia_under_8gb",
            Self::Nvidia8Gb => "nvidia_8gb",
            Self::Nvidia16Gb => "nvidia_16gb",
            Self::Nvidia24GbPlus => "nvidia_24gb_plus",
        }
    }
}

/// Built-in model facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModelCandidate {
    /// Ollama pull name.
    model: &'static str,
    /// Max hardware fit tier.
    fit_rank: u8,
    /// Coding preference inside same fit tier.
    coding_rank: u8,
}

/// Recommendation origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecommendationSource {
    /// Reused installed model.
    Installed,
    /// Fell back to built-in table.
    BuiltIn,
}

impl RecommendationSource {
    /// Stable `snake_case` tag.
    #[cfg(test)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::BuiltIn => "built_in",
        }
    }
}

/// Recommendation confidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecommendationConfidence {
    /// Hardware facts weak.
    Low,
    /// Hardware fit usable.
    Medium,
    /// Hardware fit strong.
    High,
}

impl RecommendationConfidence {
    /// Stable `snake_case` tag.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Pure model choice result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModelRecommendation {
    /// Chosen Ollama pull name.
    model: &'static str,
    /// Choice origin.
    source: RecommendationSource,
    /// Classified hardware bucket.
    hardware_class: HardwareClass,
    /// True when chosen model missing.
    needs_pull: bool,
    /// Confidence tag for output.
    confidence: RecommendationConfidence,
    /// Short fit reason.
    why: &'static str,
}

impl ModelRecommendation {
    /// Model tag.
    pub(crate) const fn model(self) -> &'static str {
        self.model
    }

    /// Pull needed.
    pub(crate) const fn needs_pull(self) -> bool {
        self.needs_pull
    }

    /// Confidence tag.
    pub(crate) const fn confidence_tag(self) -> &'static str {
        self.confidence.as_str()
    }

    /// Hardware class tag.
    pub(crate) const fn hardware_class_tag(self) -> &'static str {
        self.hardware_class.as_str()
    }

    /// Fit reason.
    pub(crate) const fn why(self) -> &'static str {
        self.why
    }
}

/// Curated built-in candidates.
const MODEL_CANDIDATES: [ModelCandidate; 4] = [
    ModelCandidate {
        model: "qwen2.5-coder:3b",
        fit_rank: 0,
        coding_rank: 0,
    },
    ModelCandidate {
        model: "qwen2.5-coder:7b",
        fit_rank: 1,
        coding_rank: 1,
    },
    ModelCandidate {
        model: "qwen2.5-coder:14b",
        fit_rank: 2,
        coding_rank: 2,
    },
    ModelCandidate {
        model: "qwen2.5-coder:32b",
        fit_rank: 3,
        coding_rank: 3,
    },
];

/// Recommend model from facts and installed names.
pub(crate) fn recommend_model(
    facts: &HardwareFacts,
    installed_models: &[String],
) -> ModelRecommendation {
    let hardware_class = classify_hardware(facts);
    let built_in = built_in_candidate(hardware_class);

    if let Some(installed) = best_installed_candidate(hardware_class, installed_models) {
        return ModelRecommendation {
            model: installed.model,
            source: RecommendationSource::Installed,
            hardware_class,
            needs_pull: false,
            confidence: confidence_for_class(hardware_class),
            why: why_for_class(hardware_class),
        };
    }

    ModelRecommendation {
        model: built_in.model,
        source: RecommendationSource::BuiltIn,
        hardware_class,
        needs_pull: true,
        confidence: confidence_for_class(hardware_class),
        why: why_for_class(hardware_class),
    }
}

/// Map hardware class to built-in candidate.
const fn built_in_candidate(class: HardwareClass) -> ModelCandidate {
    match class {
        HardwareClass::Unknown | HardwareClass::CpuSmall | HardwareClass::NvidiaUnder8Gb => {
            MODEL_CANDIDATES[0]
        }
        HardwareClass::CpuStandard
        | HardwareClass::AppleSiliconSmall
        | HardwareClass::Nvidia8Gb => MODEL_CANDIDATES[1],
        HardwareClass::AppleSiliconMedium
        | HardwareClass::AppleSiliconLarge
        | HardwareClass::Nvidia16Gb => MODEL_CANDIDATES[2],
        HardwareClass::Nvidia24GbPlus => MODEL_CANDIDATES[3],
    }
}

/// Pick best acceptable installed candidate.
fn best_installed_candidate(
    hardware_class: HardwareClass,
    installed_models: &[String],
) -> Option<ModelCandidate> {
    MODEL_CANDIDATES
        .iter()
        .copied()
        .filter(|candidate| candidate_acceptable_for_class(*candidate, hardware_class))
        .filter(|candidate| {
            installed_models
                .iter()
                .any(|model| model == candidate.model)
        })
        .max_by_key(|candidate| (candidate.fit_rank, candidate.coding_rank, candidate.model))
}

/// Check if candidate fits class.
const fn candidate_acceptable_for_class(
    candidate: ModelCandidate,
    hardware_class: HardwareClass,
) -> bool {
    if matches!(hardware_class, HardwareClass::Unknown) {
        return candidate.fit_rank == 0;
    }

    candidate.fit_rank <= hardware_class_rank(hardware_class)
}

/// Collapse class into fit rank.
const fn hardware_class_rank(class: HardwareClass) -> u8 {
    match class {
        HardwareClass::Unknown => 0,
        HardwareClass::CpuSmall => 0,
        HardwareClass::CpuStandard => 1,
        HardwareClass::AppleSiliconSmall => 1,
        HardwareClass::AppleSiliconMedium => 2,
        HardwareClass::AppleSiliconLarge => 2,
        HardwareClass::NvidiaUnder8Gb => 0,
        HardwareClass::Nvidia8Gb => 1,
        HardwareClass::Nvidia16Gb => 2,
        HardwareClass::Nvidia24GbPlus => 3,
    }
}

/// Map class to confidence.
const fn confidence_for_class(class: HardwareClass) -> RecommendationConfidence {
    match class {
        HardwareClass::Unknown => RecommendationConfidence::Low,
        HardwareClass::CpuSmall | HardwareClass::CpuStandard | HardwareClass::NvidiaUnder8Gb => {
            RecommendationConfidence::Medium
        }
        HardwareClass::AppleSiliconSmall
        | HardwareClass::AppleSiliconMedium
        | HardwareClass::AppleSiliconLarge
        | HardwareClass::Nvidia8Gb
        | HardwareClass::Nvidia16Gb
        | HardwareClass::Nvidia24GbPlus => RecommendationConfidence::High,
    }
}

/// Short fit reason.
const fn why_for_class(class: HardwareClass) -> &'static str {
    match class {
        HardwareClass::Unknown => "safe small fallback for unknown hardware",
        HardwareClass::CpuSmall => "small model fits lower-RAM CPU systems",
        HardwareClass::CpuStandard => "mid-size model fits standard CPU RAM",
        HardwareClass::AppleSiliconSmall => "mid-size model fits smaller Apple unified RAM",
        HardwareClass::AppleSiliconMedium => "larger model fits medium Apple unified RAM",
        HardwareClass::AppleSiliconLarge => "larger model fits large Apple unified RAM",
        HardwareClass::NvidiaUnder8Gb => "small model fits limited NVIDIA VRAM",
        HardwareClass::Nvidia8Gb => "mid-size model fits 8 GB NVIDIA VRAM",
        HardwareClass::Nvidia16Gb => "larger model fits 16 GB NVIDIA VRAM",
        HardwareClass::Nvidia24GbPlus => "largest model fits 24 GB plus NVIDIA VRAM",
    }
}

/// Classify hardware facts.
pub(crate) fn classify_hardware(facts: &HardwareFacts) -> HardwareClass {
    if let Some(vram_bytes) = facts.nvidia_vram_bytes {
        classify_nvidia_vram(vram_bytes)
    } else if facts.os == "macos" && facts.arch == "aarch64" {
        classify_apple_ram(facts.total_ram_bytes)
    } else {
        classify_cpu_ram(facts.total_ram_bytes)
    }
}

/// Classify NVIDIA VRAM.
const fn classify_nvidia_vram(vram_bytes: u64) -> HardwareClass {
    if vram_bytes < 8 * GIB {
        HardwareClass::NvidiaUnder8Gb
    } else if vram_bytes < 16 * GIB {
        HardwareClass::Nvidia8Gb
    } else if vram_bytes < 24 * GIB {
        HardwareClass::Nvidia16Gb
    } else {
        HardwareClass::Nvidia24GbPlus
    }
}

/// Classify Apple Silicon unified RAM.
const fn classify_apple_ram(total_ram_bytes: Option<u64>) -> HardwareClass {
    match total_ram_bytes {
        Some(bytes) if bytes < 16 * GIB => HardwareClass::AppleSiliconSmall,
        Some(bytes) if bytes < 24 * GIB => HardwareClass::AppleSiliconMedium,
        Some(_) => HardwareClass::AppleSiliconLarge,
        None => HardwareClass::Unknown,
    }
}

/// Classify CPU RAM.
const fn classify_cpu_ram(total_ram_bytes: Option<u64>) -> HardwareClass {
    match total_ram_bytes {
        Some(bytes) if bytes < 16 * GIB => HardwareClass::CpuSmall,
        Some(_) => HardwareClass::CpuStandard,
        None => HardwareClass::Unknown,
    }
}

#[cfg(test)]
mod tests;
