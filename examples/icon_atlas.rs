fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "artifacts/icons.svg".into());
    std::fs::write(&path, openpdn::icons::atlas_svg())?;
    println!(
        "{} hand-crafted vector icons: {path}",
        openpdn::icons::Icon::ALL.len()
    );
    Ok(())
}
