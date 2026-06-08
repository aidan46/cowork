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

    use super::{
        GIB, HardwareClass, HardwareFacts, MIB, classify_hardware, collect_hardware_facts,
        parse_linux_mem_total_bytes, parse_macos_memsize_bytes, parse_nvidia_smi_vram_bytes,
    };

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
    fn collect_hardware_facts_use_compile_target_os_and_arch() {
        let facts = collect_hardware_facts();

        assert_eq!(facts.os, std::env::consts::OS);
        assert_eq!(facts.arch, std::env::consts::ARCH);
    }

    #[test]
    fn parse_linux_mem_total_kib_to_bytes() {
        let meminfo = b"MemTotal:       16777216 kB\nMemFree:         1024 kB\n";

        assert_eq!(parse_linux_mem_total_bytes(meminfo), Some(16 * GIB));
    }

    #[test]
    fn ignore_malformed_linux_meminfo() {
        let meminfo = b"MemTotal: nope kB\n";

        assert_eq!(parse_linux_mem_total_bytes(meminfo), None);
    }

    #[test]
    fn parse_macos_hw_memsize_bytes() {
        assert_eq!(parse_macos_memsize_bytes(b"17179869184\n"), Some(16 * GIB));
    }

    #[test]
    fn ignore_malformed_macos_output() {
        assert_eq!(parse_macos_memsize_bytes(b"nope\n"), None);
    }

    #[test]
    fn parse_one_nvidia_mib_row() {
        assert_eq!(parse_nvidia_smi_vram_bytes(b"8192\n"), Some(8 * GIB));
    }

    #[test]
    fn parse_many_nvidia_rows_choose_max() {
        assert_eq!(
            parse_nvidia_smi_vram_bytes(b"8192\n24576\n16384\n"),
            Some(24 * GIB)
        );
    }

    #[test]
    fn ignore_bad_nvidia_rows() {
        assert_eq!(
            parse_nvidia_smi_vram_bytes(b"memory.total [MiB]\n\n8192\nbad\n"),
            Some(8192 * MIB)
        );
    }

    #[test]
    fn collected_facts_flow_into_classifier() {
        let facts = collect_hardware_facts();
        let class = classify_hardware(&facts);

        if facts.nvidia_vram_bytes.is_some() {
            assert!(matches!(
                class,
                HardwareClass::NvidiaUnder8Gb
                    | HardwareClass::Nvidia8Gb
                    | HardwareClass::Nvidia16Gb
                    | HardwareClass::Nvidia24GbPlus
            ));
        } else if facts.os == "macos" && facts.arch == "aarch64" {
            assert!(matches!(
                class,
                HardwareClass::Unknown
                    | HardwareClass::AppleSiliconSmall
                    | HardwareClass::AppleSiliconMedium
                    | HardwareClass::AppleSiliconLarge
            ));
        } else {
            assert!(matches!(
                class,
                HardwareClass::Unknown | HardwareClass::CpuSmall | HardwareClass::CpuStandard
            ));
        }
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
