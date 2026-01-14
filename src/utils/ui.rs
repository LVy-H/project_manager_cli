use colored::*;

pub fn colorize(msg: &str, color: &str) -> String {
    match color {
        "green" => msg.green().to_string(),
        "yellow" => msg.yellow().to_string(),
        "red" => msg.red().to_string(),
        "blue" => msg.blue().to_string(),
        _ => msg.to_string(),
    }
}

pub fn print_info(msg: &str) {
    println!("{}", msg.blue());
}

pub fn print_success(msg: &str) {
    println!("{}", msg.green());
}

pub fn print_warning(msg: &str) {
    println!("{}", msg.yellow());
}

pub fn print_error(msg: &str) {
    eprintln!("{} {}", "Error:".red().bold(), msg);
}

pub fn print_dim(msg: &str) {
    println!("{}", msg.dimmed());
}
