//! Setup recommendation facts and classes.

/// One binary GiB.
const GIB: u64 = 1024 * 1024 * 1024;

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
    pub(crate) const fn current_platform() -> Self {
        Self::new(std::env::consts::OS, std::env::consts::ARCH, None, None)
    }
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
mod tests {
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use super::{GIB, HardwareClass, HardwareFacts, classify_hardware};

    fn facts(
        os: &'static str,
        arch: &'static str,
        total_ram_gib: Option<u64>,
        nvidia_vram_gib: Option<u64>,
    ) -> HardwareFacts {
        HardwareFacts::new(
            os,
            arch,
            total_ram_gib.map(|gib| gib * GIB),
            nvidia_vram_gib.map(|gib| gib * GIB),
        )
    }

    #[test]
    fn current_platform_facts_use_compile_target_os_and_arch() {
        let facts = HardwareFacts::current_platform();

        assert_eq!(facts.os, std::env::consts::OS);
        assert_eq!(facts.arch, std::env::consts::ARCH);
        assert_eq!(facts.total_ram_bytes, None);
        assert_eq!(facts.nvidia_vram_bytes, None);
    }

    #[test]
    fn unknown_memory_returns_unknown() {
        assert_eq!(
            classify_hardware(&facts("linux", "x86_64", None, None)),
            HardwareClass::Unknown
        );
    }

    #[test]
    fn nvidia_vram_wins_over_cpu_ram() {
        assert_eq!(
            classify_hardware(&facts("linux", "x86_64", Some(4), Some(24))),
            HardwareClass::Nvidia24GbPlus
        );
    }

    #[test]
    fn nvidia_vram_boundaries() {
        assert_eq!(
            classify_hardware(&HardwareFacts::new(
                "linux",
                "x86_64",
                None,
                Some(8 * GIB - 1),
            )),
            HardwareClass::NvidiaUnder8Gb
        );
        assert_eq!(
            classify_hardware(&facts("linux", "x86_64", None, Some(8))),
            HardwareClass::Nvidia8Gb
        );
        assert_eq!(
            classify_hardware(&facts("linux", "x86_64", None, Some(16))),
            HardwareClass::Nvidia16Gb
        );
        assert_eq!(
            classify_hardware(&facts("linux", "x86_64", None, Some(24))),
            HardwareClass::Nvidia24GbPlus
        );
    }

    #[test]
    fn apple_silicon_detection_needs_macos_and_aarch64() {
        assert_eq!(
            classify_hardware(&facts("macos", "aarch64", Some(16), None)),
            HardwareClass::AppleSiliconMedium
        );
        assert_eq!(
            classify_hardware(&facts("macos", "x86_64", Some(16), None)),
            HardwareClass::CpuStandard
        );
    }

    #[test]
    fn apple_silicon_ram_boundaries() {
        assert_eq!(
            classify_hardware(&HardwareFacts::new(
                "macos",
                "aarch64",
                Some(16 * GIB - 1),
                None,
            )),
            HardwareClass::AppleSiliconSmall
        );
        assert_eq!(
            classify_hardware(&facts("macos", "aarch64", Some(16), None)),
            HardwareClass::AppleSiliconMedium
        );
        assert_eq!(
            classify_hardware(&facts("macos", "aarch64", Some(24), None)),
            HardwareClass::AppleSiliconLarge
        );
    }

    #[test]
    fn non_apple_arm_does_not_classify_as_apple_silicon() {
        assert_eq!(
            classify_hardware(&facts("linux", "aarch64", Some(16), None)),
            HardwareClass::CpuStandard
        );
    }

    #[test]
    fn cpu_ram_boundaries() {
        assert_eq!(
            classify_hardware(&HardwareFacts::new(
                "linux",
                "x86_64",
                Some(16 * GIB - 1),
                None,
            )),
            HardwareClass::CpuSmall
        );
        assert_eq!(
            classify_hardware(&facts("linux", "x86_64", Some(16), None)),
            HardwareClass::CpuStandard
        );
    }

    #[test]
    fn hardware_class_tags_stay_stable() {
        let cases = [
            (HardwareClass::Unknown, "unknown"),
            (HardwareClass::CpuSmall, "cpu_small"),
            (HardwareClass::CpuStandard, "cpu_standard"),
            (HardwareClass::AppleSiliconSmall, "apple_silicon_small"),
            (HardwareClass::AppleSiliconMedium, "apple_silicon_medium"),
            (HardwareClass::AppleSiliconLarge, "apple_silicon_large"),
            (HardwareClass::NvidiaUnder8Gb, "nvidia_under_8gb"),
            (HardwareClass::Nvidia8Gb, "nvidia_8gb"),
            (HardwareClass::Nvidia16Gb, "nvidia_16gb"),
            (HardwareClass::Nvidia24GbPlus, "nvidia_24gb_plus"),
        ];

        for (class, tag) in cases {
            assert_eq!(class.as_str(), tag);
        }
    }
}
