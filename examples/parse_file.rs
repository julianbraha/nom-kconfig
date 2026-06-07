use std::{collections::HashMap, env, path::PathBuf};

use clap::Parser;
use nom_kconfig::{parse_kconfig, KconfigFile, KconfigInput};
use tracing::{error, Level};

#[derive(clap::ValueEnum, Clone, Default)]
enum Format {
    #[default]
    Debug,
    Json,
    Sexp,
}

#[derive(Parser)]
#[command(author,
    bin_name = "parse_file",
    long_about = Some("Parse a Kconfig file and print the resulting AST to stdout."),
    version
)]
struct Cli {
    #[clap(long)]
    /// Root directory to resolve Kconfig file paths from. If not provided, the current working directory will be used.
    root_dir: Option<PathBuf>,
    /// Kconfig file to parse
    file: PathBuf,
    remaining_args: Vec<PathBuf>,
    /// A comma-separated list of variables: var_a=hello
    #[clap(long = "variables", use_value_delimiter = true, value_delimiter = ',')]
    pub variables: Vec<String>,
    /// Output format for the parse tree
    #[clap(long, default_value = "debug")]
    format: Format,
}

/// to use this example, run
/// ```shell
/// cargo run --all-features --example parse_file --  --root-dir [root_dir] <kconfig_file>
/// ```
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(Level::TRACE)
        .init();

    let cli = Cli::parse();
    let path = cli.file.clone();
    let root_dir = cli.root_dir.unwrap_or_else(|| env::current_dir().unwrap());

    if !cli.remaining_args.is_empty() {
        error!("Please use '--root-dir' to specify the root directory of your Kconfig project");
        return Err(format!("Please run the command:  cargo run --all-features --example parse_file -- --root-dir '{}' '{}'", cli.remaining_args[0].display(), cli.file.display()).into());
    }

    let root_dir = root_dir.canonicalize()?;

    let variables = cli
        .variables
        .into_iter()
        .filter_map(|var| {
            let mut parts = var.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(name), Some(value)) => Some((name.to_string(), value.to_string())),
                _ => {
                    error!(
                        "Invalid variable format: '{}'. Expected format is 'name=value'",
                        var
                    );
                    None
                }
            }
        })
        .collect::<HashMap<String, String>>();

    let kconfig_file =
        KconfigFile::new_with_vars(root_dir, path.clone(), &variables, &HashMap::default());

    let input = kconfig_file.read_to_string().unwrap();
    let parsing_result = parse_kconfig(KconfigInput::new_extra(&input, kconfig_file));
    if let Err(e) = parsing_result {
        panic!("{}", e);
    }
    let kconfig = parsing_result.unwrap().1;
    match cli.format {
        Format::Debug => println!("{:#?}", kconfig),
        Format::Json => println!("{}", serde_json::to_string_pretty(&kconfig)?),
        Format::Sexp => println!("{}", json_to_sexp(&serde_json::to_value(&kconfig)?)),
    }

    Ok(())
}

fn json_to_sexp(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "()".to_string(),
        serde_json::Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("{:?}", s),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_to_sexp).collect();
            format!("({})", items.join(" "))
        }
        serde_json::Value::Object(obj) => {
            let pairs: Vec<String> = obj
                .iter()
                .map(|(k, val)| format!("({} {})", k, json_to_sexp(val)))
                .collect();
            format!("({})", pairs.join(" "))
        }
    }
}
