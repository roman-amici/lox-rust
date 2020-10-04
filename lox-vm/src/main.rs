mod chunk;
mod compiler;
mod interpreter;
mod scanner;
mod token;
mod value;

use std::env;
use std::error;
use std::fs;
use std::io;
use std::io::Write;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => run_prompt(),
        2 => run_file(&args[1]).unwrap(),
        _ => println!("Usage: [script]"),
    }
    process::exit(64);
}

fn run_file(filename: &str) -> Result<(), Box<dyn error::Error + 'static>> {
    let mut interpreter = interpreter::VM::new();
    let file_contents = fs::read_to_string(filename)?;
    run(&file_contents, &mut interpreter);
    Ok(())
}

fn run_prompt() {
    let mut interpreter = interpreter::VM::new();
    let mut buffer = String::new();
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        if let Ok(_) = io::stdin().read_line(&mut buffer) {
            if buffer.len() > 1 {
                run(&buffer, &mut interpreter);
            }
        } else {
            return;
        }
        buffer.clear();
    }
}

fn run(source: &String, interpreter: &mut interpreter::VM) {
    let tokens = scanner::scan_tokens(source).unwrap();
    let mut compiler = compiler::Compiler::new(tokens);
    match compiler.compile() {
        Ok(chunk) => {
            println!("{:?}", chunk.code);
            if let Err(e) = interpreter.interpret(chunk) {
                println!("An error ocurred while interpreting");
                println!("{}", e.to_string())
            }
        }
        Err(error) => println!("{}", error.to_string()),
    };
}
