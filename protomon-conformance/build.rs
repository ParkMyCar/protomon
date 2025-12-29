fn main() -> Result<(), Box<dyn std::error::Error>> {
    protomon_build::compile_protos(
        &[
            "protos/scalars.proto",
            "protos/repeated.proto",
            "protos/nested.proto",
            "protos/edge_cases.proto",
        ],
        &["protos/"],
    )?;
    Ok(())
}
