use std::{fmt::Debug, fs::create_dir, io::{ErrorKind, Write}, time::{SystemTime, UNIX_EPOCH}};

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use sqlx::{migrate::MigrateDatabase, FromRow, Pool, Sqlite, SqlitePool};
use chrono::{DateTime, Local, TimeDelta};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

/**
 * [x] cli tool
 * [x] db setup
 * [x] db install
 * [x] start shift
 * [x] stop shift
 * [x] create task log
 * [ ] accumulated time delta
 * [ ] week tracker
 * [ ] 9/80 tracker
 * [ ] backup function
 */

#[derive(Clone, FromRow, Debug)]
struct Shift {
    id: i64,
    time_in: i64,
    time_out: Option<i64>,
    time_diff: Option<i64>,
}

// #[derive(Clone, FromRow, Debug)]
// struct Log {
//     id: i64,
//     shift_id: i64,
//     task: String,
//     time: i64,
//     created_at: i64,
// }

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

async fn is_shift_active(db: &Pool<Sqlite>) -> bool {
    let open_shift_count = sqlx::query_scalar::<_, i64>("
        SELECT COUNT(*) FROM shifts WHERE time_out IS NULL
    ")
        .fetch_one(db)
        .await
        .unwrap();
    return open_shift_count > 0;
}

async fn panic_if_shift_active(db: &Pool<Sqlite>) {
    if is_shift_active(db).await == true {
        panic!("A shift is already in progress");
    }
}
async fn panic_if_no_shift_active(db: &Pool<Sqlite>) {
    if is_shift_active(db).await == false {
        panic!("No active shifts found.");
    }
}

async fn get_active_shift(db: &Pool<Sqlite>) -> Shift {
    panic_if_no_shift_active(db).await;

    let result = sqlx::query_as::<_, Shift>("
        SELECT * FROM shifts WHERE time_out IS NULL
    ")
        .fetch_one(db)
        .await;

    return result.unwrap();
}

fn print_target_delta(t: i64) {
    let delta = TimeDelta::new(t - 32400, 0).unwrap();
    let abs_delta = delta.abs();
    let hours = abs_delta.num_hours();
    let minutes = abs_delta.num_minutes() % 60;
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let formatted = format!("({hours:0>2}:{minutes:0>2})");

    if delta.num_minutes() < 0 {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
    } else if delta.num_minutes() < 60 {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow))).unwrap();
    } else {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
    }
    writeln!(&mut stdout, "{formatted}").unwrap();
}

// Simple program to greet a person
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Log a task to the active shift
    log: Option<String>,

    /// Assign a task a time value, in minutes
    #[arg(short, long)]
    time: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Activate a shift
    Start {},

    /// Stop a currently active shift
    Stop {},

    // Edit the current or most recent shift
    Edit {},

    /// List the shift history
    List {
        /// Number of shifts to show
        shifts: Option<i64>
    },

    /// Show the current status
    Status {

    },
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
    if let Some(name) = cli.log.as_deref() {
        let shift = get_active_shift(&db).await;
        let time = cli.time.unwrap_or_default().parse::<i64>().unwrap_or_default();

        sqlx::query("
            INSERT INTO logs (shift_id, task, time) VALUES (?, ?, ?);
        ")
            .bind(shift.id)
            .bind(name)
            .bind(time)
            .execute(&db)
            .await
            .unwrap();

        print!("Task logged. Remaining: ");
        let since_the_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap();
        let now = since_the_epoch.as_secs() as i64;
        print_target_delta(now - shift.time_in);

        return;
    }

    match &cli.command {
        Some(Commands::Start {  }) => {
            panic_if_shift_active(&db).await;

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
            panic_if_no_shift_active(&db).await;

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
                Err(error) => panic!("error: {}", error)
            }
        }
        Some(Commands::Edit {  }) => {
            println!("edit")
        }
        Some(Commands::List { shifts }) => {
            println!("{:?}", shifts)
        }
        Some(Commands::Status {  }) | None => {
            println!("status")
        }
    }
}
