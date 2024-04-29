use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{SendError, Sender};
use std::thread;

// Constants for server configuration
const HOST: &str = "0.0.0.0";
const PORT: &str = "5001";

// Enum to represent different commands
#[derive(Debug)]
pub enum Command {
    Setup,
    Start,
    Teams(usize),
}

pub fn field_commands_forever(sender: Sender<Command>) -> Result<(), SendError<Command>> {
    // Bind to the host and port
    let endpoint = format!("{}:{}", HOST, PORT);
    let listener = TcpListener::bind(endpoint).unwrap();
    println!("Web server is listening at port {}", PORT);

    // Accept incoming connections
    for incoming_stream in listener.incoming() {
        let stream = incoming_stream.unwrap();

        // Spawn a new thread to handle the connection
        let sender_clone = sender.clone();
        thread::spawn(move || {
            if let Err(err) = handle_connection(stream, sender_clone) {
                eprintln!("Error handling connection: {:?}", err);
            }
        });
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, sender: Sender<Command>) -> Result<(), std::io::Error> {
    // Buffer to read the incoming request
    let mut buffer = [0; 1024];
    stream.read(&mut buffer)?;

    // Convert the request buffer to a string
    let request_str = String::from_utf8_lossy(&buffer);
    println!("Request: {}", request_str);

    // Parse the request to determine the command
    let command = parse_command(&request_str);
    match command {
        Some(cmd) => {
            // Send the command through the channel
            sender.send(cmd).unwrap();
        }
        None => {
            eprintln!("Invalid command received");
        }
    }

    let response = "HTTP/1.1 200 OK\n";

    // Send the response back to the client
    stream.write(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

// Function to parse the command from the request string
fn parse_command(request_str: &str) -> Option<Command> {
    // Implement your parsing logic here
    // For simplicity, let's assume the command is extracted from the request
    // and construct a Command enum accordingly.
    // In a real application, you would parse the request according to your protocol.
    // This is just a placeholder.
    if request_str.contains("setup") {
        Some(Command::Setup)
    } else if request_str.contains("start") {
        Some(Command::Start)
    } else if let Some(teams_count) = extract_teams_count(request_str) {
        Some(Command::Teams(teams_count))
    } else {
        None
    }
}

use serde_json::Value;

// Function to extract the number of teams from the request string
fn extract_teams_count(request_str: &str) -> Option<usize> {
    // Find the start and end positions of the JSON object within curly braces
    let start_pos = request_str.find('{')?;
    let end_pos = request_str.rfind('}')?;

    // Extract the JSON string
    let json_str = &request_str[start_pos..=end_pos];

    // Parse the JSON string
    if let Ok(json_value) = serde_json::from_str::<Value>(json_str) {
        // Check if the JSON object contains the "teams" field
        if let Some(teams_value) = json_value.get("teams") {
            // Try to parse the value of "teams" as usize
            if let Some(teams_count) = teams_value.as_u64() {
                return Some(teams_count as usize);
            }
        }
    }

    None
}
