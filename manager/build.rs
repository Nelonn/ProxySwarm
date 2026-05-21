fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure prost-build to generate only message types (no tonic service code)
    let mut config = prost_build::Config::new();
    config.out_dir("src/pb");
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");

    let proto_files = vec![
        "../proto/account.proto",
        "../proto/node/common.proto",
        "../proto/node/vless.proto",
        "../proto/node/hysteria2.proto",
        "../proto/node/trusttunnel.proto",
        "../proto/node/naiveproxy.proto",
        "../proto/node/wireguard.proto",
        "../proto/node/socks5.proto",
        "../proto/node/shadowsocks.proto",
        "../proto/node/reverseproxy.proto",
        "../proto/node/tproxy.proto",
        "../proto/node/trojan.proto",
        "../proto/node/service.proto",
        "../proto/registry/registry.proto",
    ];

    let existing_files: Vec<&str> = proto_files
        .iter()
        .filter(|f| std::path::Path::new(f).exists())
        .map(|f| *f)
        .collect();

    if !existing_files.is_empty() {
        config.compile_protos(
            &existing_files,
            &["../proto", "../proto/node", "../proto/registry"],
        )?;
    }

    Ok(())
}
