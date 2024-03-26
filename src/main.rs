use clap::{Parser, Subcommand};
use sqlx::{migrate::MigrateDatabase, FromRow, Pool, Row, Sqlite, SqlitePool};
use chrono::{DateTime, Local};

const DB_URL: &str = "sqlite://sqlite.db";

#[derive(Clone, FromRow, Debug)]
struct Shift {
    id: i64,
    time_in: i64,
    time_out: Option<i64>
}

fn format_timestamp(t: i64) -> String {
    let dt = DateTime::from_timestamp(t, 0).unwrap();
    let tz = *Local::now().offset();
    let dtz = dt.with_timezone(&tz);
    return dtz.format("%I:%M%P on %A, %b %d %Y").to_string();
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
    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        println!("Creating database {}", DB_URL);
        match Sqlite::create_database(DB_URL).await {
            Ok(_) => println!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    }

    let db = SqlitePool::connect(DB_URL).await.unwrap();
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let migrations = std::path::Path::new(&crate_dir).join("./migrations");
    let migration_results = sqlx::migrate::Migrator::new(migrations)
        .await
        .unwrap()
        .run(&db)
        .await;

    if let Err(error) = migration_results {
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
            let result = sqlx::query("
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
                Ok(row) => {
                    println!("{:?}", row.columns());
                    println!("{:?}", row.get::<i64, &str>("now"));
                    println!("{:?}", row.get::<String, usize>(2));
                    // let timestamp = format_timestamp(row.time_in);
                    // println!("Shift stopped at {}, hours worked: {}", timestamp, timestamp);
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
