use std::{fmt::Debug, fs::create_dir, io::ErrorKind, io::Write};

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use sqlx::{migrate::MigrateDatabase, FromRow, Pool, Sqlite, SqlitePool};
use chrono::{DateTime, Local, TimeDelta};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(Clone, FromRow, Debug)]
struct Shift {
    time_in: i64,
    time_out: Option<i64>,
    time_diff: Option<i64>,
}

fn format_timestamp(t: i64) -> String {
    let dt = DateTime::from_timestamp(t, 0).unwrap();
    let tz = *Local::now().offset();
    let dtz = dt.with_timezone(&tz);
    return dtz.format("%I:%M%P on %A, %b %d %Y").to_string();
}

fn format_timediff(t: i64) -> String {
    let delta = TimeDelta::new(t, 0).unwrap();
    let hours = delta.num_hours();
    let minutes = delta.num_minutes() % 60;
    return format!("{hours:0>2}:{minutes:0>2}");
}

fn print_target_delta(t: i64) {
    let delta = TimeDelta::new(t - 32400, 0).unwrap();
    let is_neg = delta.num_minutes() < 0;
    let abs_delta = delta.abs();
    let hours = abs_delta.num_hours();
    let minutes = abs_delta.num_minutes() % 60;
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let formatted = format!("({hours:0>2}:{minutes:0>2})");

    if is_neg {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
    } else {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
    }
    writeln!(&mut stdout, "{formatted}").unwrap();
}

// Simple program to greet a person
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {

    /// Something
    // #[arg(short, long, default_value_t = 1)]
    // count: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Activate a shift
    Start {},

    /// Stop a currently active shift
    Stop {},

    /// Log an Event to the active shift
    Log {
        /// Event message
        event: String,

        /// Elapsed Event time, in minutes
        #[arg(short, long)]
        time: Option<i32>,
    },

    // Edit the current or most recent shift
    Edit {},

    /// List the shift history
    List {
        /// Number of shifts to show
        shifts: Option<i32>
    },

    /// Show the current status
    Status {

    }
}

async fn get_database() -> Pool<Sqlite>{
    let proj_dirs = ProjectDirs::from("", "",  "track-cli").unwrap();
    let data_dir = proj_dirs.data_dir().to_str().unwrap();
    match create_dir(data_dir) {
        Ok(_) => (),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => (),
        Err(err) => panic!("Data directory could not be created: {err}"),
    }
    let db_url = &format!("sqlite://{data_dir}/sqlite.db");

    if !Sqlite::database_exists(db_url).await.unwrap_or(false) {
        println!("Creating database {}", db_url);
        Sqlite::create_database(db_url).await.unwrap();
    }

    let db = SqlitePool::connect(db_url).await.unwrap();
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let migrations = std::path::Path::new(&crate_dir).join("./migrations");
    if let Err(error) = sqlx::migrate::Migrator::new(migrations)
        .await
        .unwrap()
        .run(&db)
        .await {
        panic!("error: {}", error);
    }

    return db
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let db = get_database().await;

    // let user_results = sqlx::query_as::<_, Shift>("
    //     SELECT id, time_in, time_out FROM shifts
    // ")
    //     .fetch_all(&db)
    //     .await
    //     .unwrap();
    // for user in user_results {
    //     println!("[{}] name: {}", user.id, &user.time_in);
    // }
    
    match &cli.command {
        Some(Commands::Start {  }) => {
            let open_shift_count = sqlx::query_scalar::<_, i64>("
                SELECT COUNT(*) FROM shifts WHERE time_out IS NULL
            ")
                .fetch_one(&db)
                .await
                .unwrap();
            if open_shift_count > 0 {
                eprintln!("A shift is already in progress");
                return;
            }

            let result = sqlx::query_as::<_, Shift>("
                INSERT INTO shifts (id) VALUES (NULL) RETURNING *;
            ")
                .fetch_one(&db)
                .await
                .unwrap();
            let timestamp = format_timestamp(result.time_in);
            println!("Shift started at {}", timestamp);
        }
        Some(Commands::Stop {  }) => {
            let result = sqlx::query_as::<_, Shift>("
                UPDATE shifts
                SET time_out = (unixepoch()),
                    time_diff = (unixepoch() - time_in)
                WHERE
                    id = (SELECT MAX(id) FROM shifts WHERE time_out IS NULL)
                RETURNING *;
            ")
                .fetch_one(&db)
                .await;

            match result {
                Ok(shift) => {
                    let timestamp = format_timestamp(shift.time_out.unwrap());
                    let hours_worked = format_timediff(shift.time_diff.unwrap());

                    println!("Shift stopped at {timestamp}");
                    print!("hours worked: {hours_worked} ");
                    print_target_delta(shift.time_diff.unwrap());
                },
                Err(error) => {
                    if error.to_string().starts_with("no rows") {
                        eprintln!("No active shifts found.")
                    } else {
                        panic!("error: {}", error)
                    }
                },
            }
        }
        Some(Commands::Log { event, time }) => {
            println!("{:}, {:?}", event, time)
        }
        Some(Commands::Edit {  }) => {
            println!("edit")
        }
        Some(Commands::List { shifts }) => {
            println!("{:?}", shifts)
        }
        Some(Commands::Status {  }) => {
            println!("status")
        }
        None => {
            println!("default")
        }
    }
}
