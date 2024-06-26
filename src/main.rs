use std::{fmt::Debug, fs::{create_dir_all, File}, io::{BufRead, BufReader, ErrorKind, Write}, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use sqlx::{migrate::MigrateDatabase, FromRow, Pool, Sqlite, SqlitePool};
use chrono::{DateTime, Local, TimeDelta, Utc};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

/**
 * [x] cli tool
 * [x] db setup
 * [x] db install
 * [x] start shift
 * [x] stop shift
 * [x] create task log
 * [x] get status
 * [x] show standup data
 * [x] import old data
 * [x] add start/end time flags
 * [x] create release version
 * [x] create install script
 * [x] handle duration column
 * [x] order logs for standup
 * --- beta ---
 * [ ] accumulated time delta
 * [ ] week tracker
 * [ ] 9/80 tracker
 * [ ] backup function
 */

const DAY_SECONDS: i64 = 32400;

#[derive(Clone, FromRow, Debug)]
struct Shift {
    id: i64,
    time_in: i64,
    time_out: Option<i64>,
    time_diff: Option<i64>,
}

#[derive(Clone, FromRow, Debug)]
struct Log {
    // id: i64,
    // shift_id: i64,
    task: String,
    time: i64,
    // created_at: i64,
}

fn format_timestamp(t: i64) -> String {
    let dt = DateTime::from_timestamp(t, 0).unwrap();
    let tz = *Local::now().offset();
    let dtz = dt.with_timezone(&tz);
    return dtz.format("%I:%M%P on %A, %b %d %Y").to_string();
}

fn format_timestamp_short(t: i64) -> String {
    let dt = DateTime::from_timestamp(t, 0).unwrap();
    let tz = *Local::now().offset();
    let dtz = dt.with_timezone(&tz);
    return dtz.format("%I:%M%P").to_string();
}

fn format_timediff(t: i64) -> String {
    let delta = TimeDelta::new(t, 0).unwrap();
    let hours = delta.num_hours();
    let minutes = delta.num_minutes() % 60;
    return format!("{hours:0>2}:{minutes:0>2}");
}

fn get_time_from_flag(t: Option<String>) -> i64 {
    if t.is_none() {
        return Utc::now().timestamp();
    }

    let time = t.unwrap();
    let human_time_str = format!("{time} -0600");
    let timestamp = DateTime::parse_from_str(&human_time_str, "%H:%M %m/%d/%Y %z");

    return timestamp.unwrap().timestamp();
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

async fn get_balance(_db: &Pool<Sqlite>) -> i64 {
    return 0; // TODO
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

async fn get_completed_shift_list(db: &Pool<Sqlite>, n: i64) -> Vec<Shift> {
    let results = sqlx::query_as::<_, Shift>("
        SELECT * FROM shifts WHERE time_out NOT NULL ORDER BY time_in DESC LIMIT (?)
    ")
        .bind(n)
        .fetch_all(db)
        .await
        .unwrap();

    return results;
}

async fn get_shift_logs(db: &Pool<Sqlite>, id: i64) -> Vec<Log> {
    let results = sqlx::query_as::<_, Log>("
        SELECT * FROM logs WHERE shift_id = (?)
    ")
        .bind(id)
        .fetch_all(db)
        .await
        .unwrap();

    return results;
}

fn print_delta(t: i64, d: i64, n: bool) {
    let delta = TimeDelta::new(t - d, 0).unwrap();
    let abs_delta = delta.abs();
    let hours = abs_delta.num_hours();
    let minutes = abs_delta.num_minutes() % 60;
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let formatted = format!("({hours:0>2}:{minutes:0>2})");

    if delta.num_minutes() < 0 {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red))).unwrap();
    } else if delta.num_minutes() < 60 && n {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow))).unwrap();
    } else {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
    }
    writeln!(&mut stdout, "{formatted}").unwrap();
    WriteColor::reset(&mut stdout).unwrap();

}

fn print_target_delta(t: i64) { print_delta(t, DAY_SECONDS, true) }
fn print_zero_delta(t: i64) { print_delta(t, 0, false) }

fn print_active_target_delta(t: i64) {
    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap();
    let now = since_the_epoch.as_secs() as i64;

    print_target_delta(now - t);
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Activate a shift
    Start {
        /// Override the start time with the format "%H:%M %m/%d/%Y"
        #[arg(short, long)]
        time: Option<String>,
    },

    /// Stop a currently active shift
    Stop {
        /// Override the start time with the format "%H:%M %m/%d/%Y"
        #[arg(short, long)]
        time: Option<String>,
    },

    /// Log a task to the active shift
    Log {
        /// Task to be logged
        log: Option<String>,

        /// Assign a task a time value, in minutes
        #[arg(short, long)]
        time: Option<String>,
    },

    /// TODO: Edit the current or most recent shift
    Edit {},

    /// TODO: List the shift history
    List {
        /// Number of shifts to show
        #[arg(default_value_t = 3)]
        shifts: i64
    },

    // Could make this for planned work for the day as well
    /// Show info for daily standup
    Stand {},

    /// Show the current status
    Status {},

    /// Import data from a text file, with a custom format
    Import {
        path: PathBuf,
    },
}

async fn get_database() -> Pool<Sqlite>{
    let proj_dirs = ProjectDirs::from("", "",  "track-cli").unwrap();
    let data_dir = proj_dirs.data_dir().to_str().unwrap();
    match create_dir_all(data_dir) {
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
    if let Err(error) = sqlx::migrate!()
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

    match &cli.command {
        Some(Commands::Start { time }) => {
            panic_if_shift_active(&db).await;

            let update_time = get_time_from_flag(time.clone());
            let result = sqlx::query_as::<_, Shift>("
                INSERT INTO shifts (time_in) VALUES (?) RETURNING *;
            ")
                .bind(update_time)
                .fetch_one(&db)
                .await
                .unwrap();
            let timestamp = format_timestamp(result.time_in);
            println!("Shift started at {}", timestamp);
        }
        Some(Commands::Stop { time }) => {
            panic_if_no_shift_active(&db).await;

            let update_time = get_time_from_flag(time.clone());
            let result = sqlx::query_as::<_, Shift>("
                UPDATE shifts
                SET time_out = (?),
                    time_diff = (? - time_in)
                WHERE
                    id = (SELECT MAX(id) FROM shifts WHERE time_out IS NULL)
                RETURNING *;
            ")
                .bind(update_time)
                .bind(update_time)
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
        
        Some(Commands::Log { log, time }) => {
            let name = log.clone().unwrap();
            let shift = get_active_shift(&db).await;
            let time = time.clone().unwrap_or_default().parse::<i64>().unwrap_or_default();

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
            print_active_target_delta(shift.time_in);
        },
        Some(Commands::Edit {  }) => {
            println!("edit")
        }
        Some(Commands::List { shifts }) => {
            println!("{:?}", shifts)
        }
        Some(Commands::Stand { }) => {
            let shift_list = get_completed_shift_list(&db, 1).await;
            let last_shift = shift_list.get(0).unwrap();
            let logs = get_shift_logs(&db, last_shift.id).await;
            
            println!("-- {} --", format_timestamp(last_shift.time_in));
            for log in logs {
                println!("{} - {}", format_timediff(log.time), log.task);
            }

            let active_shift = get_active_shift(&db).await;
            let active_logs = get_shift_logs(&db, active_shift.id).await;

            println!("-- {} --", format_timestamp(active_shift.time_in));
            for log in active_logs {
                println!("{} - {}", format_timediff(log.time), log.task);
            }
        }
        Some(Commands::Status {  }) | None => {
            match is_shift_active(&db).await {
                true => {
                    let shift = get_active_shift(&db).await;
                    print!(
                        "In: \t\t{}\nExpected Out: \t{}\nRemaining: \t",
                        format_timestamp_short(shift.time_in),
                        format_timestamp_short(shift.time_in + DAY_SECONDS),
                    );
                    print_active_target_delta(shift.time_in);
                }
                false => println!("No active shifts."),
            }

            match get_balance(&db).await {
                0 => {},
                balance => {
                    print!("Balance: \t");
                    print_zero_delta(balance);
                },
            }
        }
        Some(Commands::Import { path }) => {
            let file = File::open(path).unwrap();
            let reader = BufReader::new(file);

            let tx = db.begin().await.unwrap();

            for line in reader.lines() {
                let line = line.unwrap();
                let mut columns = line.split(" | ");

                let date = columns.next().unwrap();
                if date == "" {
                    continue;
                }
                
                let times = columns.next().unwrap();
                let mut times_split = times.split(" - ");
                let time_in_str = times_split.next().unwrap();
                let time_out_str = times_split.next().unwrap();

                let timestamp_in_str = format!("{time_in_str} {date} -0600");
                let timestamp_in = DateTime::parse_from_str(&timestamp_in_str, "%H%M %m/%d/%Y %z").unwrap();
                let timestamp_out_str = format!("{time_out_str} {date} -0600");
                let timestamp_out = DateTime::parse_from_str(&timestamp_out_str, "%H%M %m/%d/%Y %z").unwrap();
                let unix_in = timestamp_in.timestamp();
                let unix_out = timestamp_out.timestamp();

                let result = sqlx::query_as::<_, Shift>("
                    INSERT INTO shifts (time_in, time_out, time_diff)
                    VALUES (?, ?, ?) RETURNING *;
                ")
                    .bind(unix_in)
                    .bind(unix_out)
                    .bind(unix_out - unix_in)
                    .fetch_one(&db)
                    .await
                    .unwrap();

                let _duration = columns.next().unwrap();

                let tasks = columns.next().unwrap();
                let tasks_split = tasks.split(", ");
                for task in tasks_split {
                    sqlx::query("
                        INSERT INTO logs (shift_id, task, time) VALUES (?, ?, ?);
                    ")
                        .bind(result.id)
                        .bind(task)
                        .bind(0)
                        .execute(&db)
                        .await
                        .unwrap();
                }
            }

            tx.commit().await.unwrap();
        }
    }
}
