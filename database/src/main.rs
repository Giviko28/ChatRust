use postgres::{Client, NoTls, Error};
use std::fs;

fn run_migration(client: &mut Client, migration: &str) -> Result<(), Error> {
    println!("{}", migration);
    client.batch_execute(migration)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let migrations_dir = "./migrations";
    let mut client = Client::connect("host=localhost port=7777 user=postgres password=mysecretpassword dbname=postgres", NoTls)?;

    // Read migration files and execute them
    for entry in fs::read_dir(migrations_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().unwrap_or_default() == "sql" {
            let migration = fs::read_to_string(&path)?;
            println!("Running migration: {:?}", path.file_name().unwrap());
            run_migration(&mut client, &migration)?;
        }
    }

    println!("Migrations applied successfully.");
    Ok(())
}
