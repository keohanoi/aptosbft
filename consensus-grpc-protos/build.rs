use std::io::Result;

fn main() -> Result<()> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &["../protos/consensus_app.proto"],
            &["../protos"],
        )?;
    Ok(())
}
