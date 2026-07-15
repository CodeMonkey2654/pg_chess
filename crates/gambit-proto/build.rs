fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("protoc binary");
    std::env::set_var("PROTOC", protoc);

    let server = std::env::var("CARGO_FEATURE_SERVER").is_ok();
    let client = std::env::var("CARGO_FEATURE_CLIENT").is_ok();

    tonic_build::configure()
        .build_server(server)
        .build_client(client)
        .compile_protos(
            &[
                "proto/gambit/v1/common.proto",
                "proto/gambit/v1/studio.proto",
                "proto/gambit/v1/ingest.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
