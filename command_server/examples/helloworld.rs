use command_server;
use std::sync::mpsc::channel;
use std::thread;

fn main() {
    // Create a channel for sending commands to the library
    let (sender, receiver) = channel();

    // Start the library's server in a separate thread
    thread::spawn(move || {
        if let Err(err) = command_server::field_commands_forever(sender) {
            eprintln!("Error in library: {:?}", err);
        }
    });

    // Main thread can now handle received commands or perform other tasks
    // For demonstration, let's just print received commands
    for received_cmd in receiver {
        println!("Received command: {:?}", received_cmd);
        // Implement your logic to handle the received commands here
    }

    // Optionally, you can perform cleanup or other tasks before exiting
}
