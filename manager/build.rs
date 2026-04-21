fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure prost-build to generate only message types (no tonic service code)
    let mut config = prost_build::Config::new();
    config.out_dir("src/pb");

    let proto_files = vec![
        "../proto/common.proto",
        "../proto/vless.proto",
        "../proto/hysteria2.proto",
        "../proto/trusttunnel.proto",
        "../proto/naiveproxy.proto",
        "../proto/wireguard.proto",
        "../proto/socks5.proto",
        "../proto/service.proto",
    ];

    let existing_files: Vec<&str> = proto_files
        .iter()
        .filter(|f| std::path::Path::new(f).exists())
        .map(|f| *f)
        .collect();

    if !existing_files.is_empty() {
        config.compile_protos(&existing_files, &["../proto"])?;
    }

    Ok(())
}
