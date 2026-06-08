#![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

use super::{
    GIB, HardwareClass, HardwareFacts, MIB, RecommendationConfidence, RecommendationSource,
    classify_hardware, collect_hardware_facts, parse_linux_mem_total_bytes,
    parse_macos_memsize_bytes, parse_nvidia_smi_vram_bytes, recommend_model,
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
            Some(8 * GIB - 1)
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
            None
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

#[test]
fn unknown_hardware_recommends_safe_small_default() {
    let recommendation = recommend_model(&facts("linux", "x86_64", None, None), &[]);

    assert_eq!(recommendation.model, "qwen2.5-coder:3b");
    assert_eq!(recommendation.hardware_class, HardwareClass::Unknown);
    assert_eq!(recommendation.source, RecommendationSource::BuiltIn);
    assert!(recommendation.needs_pull);
    assert_eq!(recommendation.confidence, RecommendationConfidence::Low);
    assert_eq!(
        recommendation.why,
        "safe small fallback for unknown hardware"
    );
}

#[test]
fn cpu_small_recommends_3b() {
    let recommendation = recommend_model(
        &HardwareFacts::new("linux", "x86_64", Some(16 * GIB - 1), None),
        &[],
    );

    assert_eq!(recommendation.model, "qwen2.5-coder:3b");
    assert_eq!(recommendation.hardware_class, HardwareClass::CpuSmall);
}

#[test]
fn cpu_standard_recommends_7b() {
    let recommendation = recommend_model(&facts("linux", "x86_64", Some(16), None), &[]);

    assert_eq!(recommendation.model, "qwen2.5-coder:7b");
    assert_eq!(recommendation.hardware_class, HardwareClass::CpuStandard);
}

#[test]
fn apple_medium_or_large_recommends_14b() {
    let medium = recommend_model(&facts("macos", "aarch64", Some(16), None), &[]);
    let large = recommend_model(&facts("macos", "aarch64", Some(24), None), &[]);

    assert_eq!(medium.model, "qwen2.5-coder:14b");
    assert_eq!(large.model, "qwen2.5-coder:14b");
}

#[test]
fn nvidia_8gb_recommends_7b() {
    let recommendation = recommend_model(&facts("linux", "x86_64", None, Some(8)), &[]);

    assert_eq!(recommendation.model, "qwen2.5-coder:7b");
    assert_eq!(recommendation.hardware_class, HardwareClass::Nvidia8Gb);
}

#[test]
fn nvidia_16gb_recommends_14b() {
    let recommendation = recommend_model(&facts("linux", "x86_64", None, Some(16)), &[]);

    assert_eq!(recommendation.model, "qwen2.5-coder:14b");
    assert_eq!(recommendation.hardware_class, HardwareClass::Nvidia16Gb);
}

#[test]
fn nvidia_24gb_plus_recommends_32b() {
    let recommendation = recommend_model(&facts("linux", "x86_64", None, Some(24)), &[]);

    assert_eq!(recommendation.model, "qwen2.5-coder:32b");
    assert_eq!(recommendation.hardware_class, HardwareClass::Nvidia24GbPlus);
}

#[test]
fn installed_acceptable_model_wins_over_pull() {
    let recommendation = recommend_model(
        &facts("linux", "x86_64", None, Some(16)),
        &[
            String::from("qwen2.5-coder:7b"),
            String::from("qwen2.5-coder:14b"),
        ],
    );

    assert_eq!(recommendation.model, "qwen2.5-coder:14b");
    assert_eq!(recommendation.source, RecommendationSource::Installed);
    assert!(!recommendation.needs_pull);
}

#[test]
fn installed_too_large_model_is_ignored() {
    let recommendation = recommend_model(
        &facts("linux", "x86_64", None, Some(8)),
        &[String::from("qwen2.5-coder:14b")],
    );

    assert_eq!(recommendation.model, "qwen2.5-coder:7b");
    assert_eq!(recommendation.source, RecommendationSource::BuiltIn);
    assert!(recommendation.needs_pull);
}

#[test]
fn installed_unknown_model_is_ignored() {
    let recommendation = recommend_model(
        &facts("linux", "x86_64", None, Some(16)),
        &[String::from("llama3.1:8b")],
    );

    assert_eq!(recommendation.model, "qwen2.5-coder:14b");
    assert_eq!(recommendation.source, RecommendationSource::BuiltIn);
    assert!(recommendation.needs_pull);
}

#[test]
fn installed_selection_is_deterministic_across_input_order() {
    let facts = facts("linux", "x86_64", None, Some(24));
    let first = recommend_model(
        &facts,
        &[
            String::from("qwen2.5-coder:7b"),
            String::from("qwen2.5-coder:14b"),
        ],
    );
    let second = recommend_model(
        &facts,
        &[
            String::from("qwen2.5-coder:14b"),
            String::from("qwen2.5-coder:7b"),
        ],
    );

    assert_eq!(first, second);
    assert_eq!(first.model, "qwen2.5-coder:14b");
}

#[test]
fn needs_pull_false_only_when_chosen_model_installed() {
    let built_in_installed = recommend_model(
        &facts("linux", "x86_64", Some(16), None),
        &[String::from("qwen2.5-coder:7b")],
    );
    let missing_built_in = recommend_model(&facts("linux", "x86_64", Some(16), None), &[]);

    assert!(!built_in_installed.needs_pull);
    assert_eq!(built_in_installed.source, RecommendationSource::Installed);
    assert!(missing_built_in.needs_pull);
    assert_eq!(missing_built_in.source, RecommendationSource::BuiltIn);
}

#[test]
fn recommendation_source_and_confidence_tags_stay_stable() {
    let source_cases = [
        (RecommendationSource::Installed, "installed"),
        (RecommendationSource::BuiltIn, "built_in"),
    ];
    let confidence_cases = [
        (RecommendationConfidence::Low, "low"),
        (RecommendationConfidence::Medium, "medium"),
        (RecommendationConfidence::High, "high"),
    ];

    for (source, tag) in source_cases {
        assert_eq!(source.as_str(), tag);
    }

    for (confidence, tag) in confidence_cases {
        assert_eq!(confidence.as_str(), tag);
    }
}
