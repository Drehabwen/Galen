use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("rehab golden eval failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let workspace = args
        .first()
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir().map_err(|error| error.to_string())?);
    let source_path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("evals/case-datasets/ais-textbook-pilot-v1/cases.json");
    let report = galen_lib::rehab_eval::run_golden_journeys(&workspace, source_path)?;
    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("serialize report: {error}"))?;

    if let Some(output) = args.get(2) {
        let path = PathBuf::from(output);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create report directory: {error}"))?;
        }
        std::fs::write(&path, &json).map_err(|error| format!("write report: {error}"))?;
        println!("{}", path.display());
    } else {
        println!("{json}");
    }
    if !report.passed {
        return Err("negative optimization gate failed".into());
    }
    Ok(())
}
