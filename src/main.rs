#[macro_use]
extern crate lazy_static;

use std::io::{Read, Write};

const CONFIG_SECTION: &str = "KATAPASS";
const CONFIG_ENGINE_PATH: &str = "ENGINE";
const CONFIG_ENGINE_ARGS: &str = "ARGS";
const CONFIG_INTERCEPT: &str = "INTERCEPT";
const CONFIG_FIELDS: [&str; 3] = [CONFIG_ENGINE_PATH, CONFIG_ENGINE_ARGS, CONFIG_INTERCEPT];

const BLACK: &str = "B";
const WHITE: &str = "W";

const UNDO_COMMAND: &str = "undo\n";
const PASS_OUTPUT: &str = "=\nplay pass\n\n";

const KATAPASS_CONSIDERING: &str = "KataPass is considering passing...\n";
const KATAPASS_PLAY: &str = "KataPass has decided to play.\n";
const KATAPASS_PASS: &str = "KataPass has decided to pass.\n";

const PASS_COMMAND_PREFIX: &str = "play ";
const PASS_COMMAND_SUFFIX: &str = " pass\n";

lazy_static! {
    static ref KATAPASS_CONFIG: std::collections::HashMap<String, String> = {
        let mut katapass_config: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut katapass_args: std::env::Args = std::env::args();
        let mut config_file: configparser::ini::Ini = configparser::ini::Ini::new();
        katapass_args.next().expect("No KataPass executable?");
        let config_file_path: &String = &katapass_args
            .next()
            .expect("No KataPass config file specified.");
        config_file
            .load(config_file_path)
            .expect(&format!("Failed to load config file: {}", config_file_path)[..]);
        for config_field in CONFIG_FIELDS {
            let value: String = config_file
                .get(CONFIG_SECTION, config_field)
                .expect(&format!("Field missing from config: {}", config_field)[..]);
            katapass_config.insert(config_field.to_string(), value);
        }
        katapass_config
    };
    static ref OPPOSITE_COLOR: std::collections::HashMap<String, String> = {
        let mut opposite_color: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        opposite_color.insert(BLACK.to_string(), WHITE.to_string());
        opposite_color.insert(WHITE.to_string(), BLACK.to_string());
        opposite_color
    };
}

/// Reads a message to KataPass stdin
fn read_katapass_stdin() -> String {
    let mut input_buffer: String = String::new();
    std::io::stdin()
        .read_line(&mut input_buffer)
        .expect("Failed to read from KataPass stdin.");
    input_buffer
}

/// Writes a message to KataPass stdout
fn write_katapass_stdout(message: String) {
    std::io::stdout()
        .write_all(message.as_bytes())
        .expect("Failed to write to KataPass stdout.");
}

/// Writes a message to KataPass stderr
fn write_katapass_stderr(message: String) {
    std::io::stderr()
        .write(message.as_bytes())
        .expect("Failed to write to KataPass stderr.");
}

/// Writes a message to engine stdin
fn write_engine_stdin(engine_stdin: &mut std::process::ChildStdin, message: String) {
    engine_stdin
        .write_all(message.as_bytes())
        .expect("Failed to write to engine stdin.");
}

/// Reads a byte from engine stdout, returns a String
fn read_engine_stdout_byte(engine_stdout: &mut std::process::ChildStdout) -> String {
    let byte_buffer: &mut [u8; 1] = &mut [0u8; 1];
    engine_stdout
        .read(byte_buffer)
        .expect("Failed to read from engine stdout.");
    String::from_utf8_lossy(byte_buffer).to_string()
}

/// Reads a byte from engine stderr, returns a String
fn read_engine_stderr_byte(engine_stderr: &mut std::process::ChildStderr) -> String {
    let byte_buffer: &mut [u8; 1] = &mut [0u8; 1];
    engine_stderr
        .read(byte_buffer)
        .expect("Failed to read from engine stderr.");
    String::from_utf8_lossy(byte_buffer).to_string()
}

/// Sends a response from the engine to the receiver
fn send_engine_response(engine_response_tx: &std::sync::mpsc::Sender<String>, response: String) {
    engine_response_tx
        .send(response)
        .expect("Failed to send engine response.");
}

/// Receives a response from the engine, returns a String
fn recv_engine_response(engine_response_rx: &std::sync::mpsc::Receiver<String>) -> String {
    engine_response_rx
        .recv()
        .expect("Failed to receive engine response.")
}

/// Parses an engine response, returns the highest winrate (f64)
fn get_winrate_from_response(response: String) -> f64 {
    let mut best_winrate: f64 = 0.0;
    for line in response.lines() {
        let tokens: Vec<&str> = line.split(" ").collect();
        let mut winrate_next: bool = false;
        for token in tokens {
            if winrate_next {
                let winrate: f64 = token.parse::<f64>().expect("Winrate data invalid.");
                if winrate > best_winrate {
                    best_winrate = winrate;
                }
                break;
            } else if token == "winrate" {
                winrate_next = true;
            }
        }
    }
    best_winrate
}

/// Returns a winrate (f64) after passing and a pass command (String)
fn process_intercept(
    input_buffer: String,
    mut engine_stdin: &mut std::process::ChildStdin,
    engine_response_rx: &std::sync::mpsc::Receiver<String>,
) -> (f64, String) {
    let mut command_vec: Vec<&str> = input_buffer.split(" ").collect();
    if command_vec.len() < 2 {
        panic!("Genmove command requires color argument.");
    }
    let color: &str = command_vec[1];
    command_vec[1] = OPPOSITE_COLOR
        .get(command_vec[1])
        .expect("Invalid color argument for genmove command.");
    let genmove_command = command_vec.join(" ");
    write_engine_stdin(&mut engine_stdin, genmove_command);
    let winrate: f64 = 1.0 - get_winrate_from_response(recv_engine_response(&engine_response_rx));
    write_engine_stdin(&mut engine_stdin, UNDO_COMMAND.to_string());
    recv_engine_response(&engine_response_rx);
    (
        winrate,
        format!("{}{}{}", PASS_COMMAND_PREFIX, color, PASS_COMMAND_SUFFIX),
    )
}
/// Returns the engine path (String) and the engine arguments (String)
fn spawn_engine_process() -> (
    std::process::Child,
    std::process::ChildStdin,
    std::process::ChildStdout,
    std::process::ChildStderr,
) {
    let mut engine_command: std::process::Command =
        std::process::Command::new(&KATAPASS_CONFIG[CONFIG_ENGINE_PATH].clone());
    engine_command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for engine_arg in KATAPASS_CONFIG[CONFIG_ENGINE_ARGS][..].split(' ') {
        engine_command.arg(engine_arg);
    }
    let mut engine_process: std::process::Child = engine_command
        .spawn()
        .expect("Failed to spawn engine process.");
    let engine_stdin: std::process::ChildStdin = engine_process
        .stdin
        .take()
        .expect("Failed to acquire engine stdin.");
    let engine_stdout: std::process::ChildStdout = engine_process
        .stdout
        .take()
        .expect("Failed to acquire engine stdout.");
    let engine_stderr: std::process::ChildStderr = engine_process
        .stderr
        .take()
        .expect("Failed to acquire engine stderr.");
    (engine_process, engine_stdin, engine_stdout, engine_stderr)
}

/// Spawns the broker thread, which acts as middleman between the console and the engine
fn spawn_engine_broker_thread(
    mut engine_stdin: std::process::ChildStdin,
    engine_response_rx: std::sync::mpsc::Receiver<String>,
) {
    std::thread::spawn(move || {
        let mut input_buffer: String;
        loop {
            input_buffer = read_katapass_stdin();
            if input_buffer.starts_with(&KATAPASS_CONFIG[CONFIG_INTERCEPT][..]) {
                write_katapass_stderr(KATAPASS_CONSIDERING.to_string());
                let (winrate, pass_command) =
                    process_intercept(input_buffer.clone(), &mut engine_stdin, &engine_response_rx);
                if winrate >= 0.5f64 {
                    write_katapass_stderr(KATAPASS_PASS.to_string());
                    input_buffer = pass_command;
                    write_katapass_stdout(PASS_OUTPUT.to_string());
                } else {
                    write_katapass_stderr(KATAPASS_PLAY.to_string());
                }
            }
            write_engine_stdin(&mut engine_stdin, input_buffer.clone());
            input_buffer.clear();
            write_katapass_stdout(recv_engine_response(&engine_response_rx));
        }
    });
}

/// Spawns the engine response thread
fn spawn_engine_response_thread(
    mut engine_stdout: std::process::ChildStdout,
    engine_response_tx: std::sync::mpsc::Sender<String>,
) {
    std::thread::spawn(move || {
        let mut string_buffer: String;
        let mut response: String = String::new();
        let mut last_three: fixed_vec_deque::FixedVecDeque<[String; 3]> =
            fixed_vec_deque::FixedVecDeque::<[String; 3]>::new();
        loop {
            string_buffer = read_engine_stdout_byte(&mut engine_stdout);
            response.push_str(&string_buffer);
            last_three.push_back().clone_from(&string_buffer);
            if last_three.len() == 3
                && ((last_three[0] == "\n" || last_three[1] == "\n") && last_three[2] == "\n")
            {
                send_engine_response(&engine_response_tx, response.clone());
                response.clear();
                last_three.clear();
            }
        }
    });
}

/// Spawns the engine stderr thread (passthru)
fn spawn_engine_stderr_thread(mut engine_stderr: std::process::ChildStderr) {
    std::thread::spawn(move || loop {
        write_katapass_stderr(read_engine_stderr_byte(&mut engine_stderr));
    });
}

/// Main entrypoint
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut engine_process, engine_stdin, engine_stdout, engine_stderr) = spawn_engine_process();
    let (engine_response_tx, engine_response_rx): (
        std::sync::mpsc::Sender<String>,
        std::sync::mpsc::Receiver<String>,
    ) = std::sync::mpsc::channel();
    spawn_engine_broker_thread(engine_stdin, engine_response_rx);
    spawn_engine_response_thread(engine_stdout, engine_response_tx);
    spawn_engine_stderr_thread(engine_stderr);
    engine_process
        .wait()
        .expect("Engine process died unexpectedly.");
    Ok(())
}
