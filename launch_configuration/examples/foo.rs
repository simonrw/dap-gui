use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Bar {
    pub a: u32,
}

fn parse_launch_configuration(input: &str) -> eyre::Result<Bar> {
    let json_value = jsonc_parser::parse_to_serde_value(input, &Default::default())?.unwrap();
    let bar = serde_json::from_value(json_value)?;
    Ok(bar)
}

fn main() -> eyre::Result<()> {
    let input = r#"{"a": 10} // test"#;
    let bar = parse_launch_configuration(input)?;
    dbg!(bar);

    Ok(())
}
