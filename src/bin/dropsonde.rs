use anyhow::{anyhow, Context, Result};
use supercell::matcher::Matcher;
use supercell::matcher::RhaiMatcher;

fn main() -> Result<()> {
    let mut rhai_input_path: Option<String> = None;
    let mut json_input_path: Option<String> = None;
    for arg in std::env::args_os().skip(1) {
        let arg = arg.to_string_lossy();
        if arg.ends_with(".rhai") {
            rhai_input_path = Some(arg.to_string());
        } else if arg.ends_with(".json") {
            json_input_path = Some(arg.to_string());
        }
    }

    if rhai_input_path.is_none() {
        return Err(anyhow!("No rhai input file provided"));
    }
    let rhai_input_path = rhai_input_path.unwrap();

    if json_input_path.is_none() {
        return Err(anyhow!("No json input file provided"));
    }
    let json_input_path = json_input_path.unwrap();

    let json_content = std::fs::read(json_input_path)
        .map_err(|err| anyhow::Error::new(err).context(anyhow!("reading input_json failed")))?;
    let value: serde_json::Value =
        serde_json::from_slice(&json_content).context("parsing input_json failed")?;

    let matcher = RhaiMatcher::new(&rhai_input_path).context("could not construct matcher")?;
    let result = matcher.matches(&value)?;

    let result = result.ok_or(anyhow!("no matches found"))?;

    println!("{:?} {}", result.0, result.1);

    Ok(())
}
