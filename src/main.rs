mod interactive;
use clap::{Parser, Subcommand};
use color_eyre::Report;
use markdown_query::document;
use std::ffi::OsStr;
use walkdir::WalkDir;
use xapian_rusty::{Database, Stem, TermGenerator, WritableDatabase, BRASS, DB_CREATE_OR_OPEN};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Cli {
    // TODO use https://docs.rs/clap-verbosity-flag/1.0.0/clap_verbosity_flag/
    /// Set level of verbosity
    #[clap(short, long, parse(from_occurrences))]
    verbosity: u8,

    /// Specify a PAGER to use when viewing markdown
    #[clap(long, env = "PAGER", default_value = "less")]
    pager: String,

    /// Specify an EDITOR to use when editing markdown
    #[clap(long, env = "EDITOR", default_value = "vi")]
    editor: String,

    /// Specify where to write the DB to
    #[clap(
        short,
        long,
        parse(from_os_str),
        value_name = "XAPIAN DB DIR",
        default_value = "~/.mdq-data"
    )]
    db_path: Box<OsStr>,

    #[clap(subcommand)]
    subcommand: Option<Subcommands>,
}

#[derive(Debug, Subcommand)]
#[clap(rename_all = "snake_case")]
enum Subcommands {
    /// Re-index data
    Update {
        /// Directories to search recursively for markdown content
        paths: Vec<String>,
    },

    /// Specify a starting query for interactive query mode
    Query {
        /// Query string
        query: String,
    },
}

fn setup() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    Ok(())
}

fn main() -> Result<(), Report> {
    // Parse CLI Arguments
    let cli = Cli::parse();
    let db_path: String = shellexpand::tilde(cli.db_path.to_str().unwrap()).into();

    setup()?;

    match cli.subcommand {
        Some(Subcommands::Update { ref paths }) => {
            let mut db = WritableDatabase::new(&db_path, BRASS, DB_CREATE_OR_OPEN)
                .expect("Could not open db for writing");
            let mut tg = TermGenerator::new()?;
            let mut stemmer = Stem::new("en")?;
            tg.set_stemmer(&mut stemmer)?;

            for path in paths {
                let walker = WalkDir::new(path).into_iter();
                for entry in walker.filter_entry(|e| {
                    !e.file_name()
                        .to_str()
                        .map(|s| s.starts_with('.'))
                        .unwrap_or(false)
                }) {
                    match entry {
                        Ok(path) => {
                            let path = path.path();
                            if path.extension().is_none() || path.extension().unwrap() != "md" {
                                continue;
                            }
                            if let Ok(doc) = document::Document::parse_file(path) {
                                doc.update_index(&mut db, &mut tg)?;
                                if cli.verbosity > 0 {
                                    println!("✅ {}", doc.filename);
                                }
                            } else {
                                eprintln!("❌ Failed to load file {}", path.display());
                            }
                        }

                        Err(e) => eprintln!("❌ {:?}", e),
                    }
                }

                db.commit()?;
            }
        }
        None => {
            interactive::setup_panic();
            let db = Database::new_with_path(&db_path, DB_CREATE_OR_OPEN)?;
            let iter = IntoIterator::into_iter(interactive::query(
                db,
                cli.verbosity,
                cli.pager,
                cli.editor,
            )?); // strings is moved here
            for s in iter {
                // next() moves a string out of the iter
                println!("{}", s);
            }
        }
        // TODO: user passed in a starting query, use it
        //Some(Subcommands::Query { ref query }) => {
        Some(Subcommands::Query { query: _ }) => {
            interactive::setup_panic();

            let db = Database::new_with_path(&db_path, DB_CREATE_OR_OPEN)?;
            let iter = IntoIterator::into_iter(interactive::query(
                db,
                cli.verbosity,
                cli.pager,
                cli.editor,
            )?); // strings is moved here
            for s in iter {
                // next() moves a string out of the iter
                println!("{}", s);
            }
        }
    }

    Ok(())
}
