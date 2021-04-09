mod chunk;
mod compiler;
mod interpreter;
mod scanner;
mod token;
mod value;

use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::env;
use std::error;
use std::fs;
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
    let mut rl = Editor::<()>::new();
    let mut compilable_unit = String::new();
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let line = line.trim_end();
                if line.ends_with("\\") {
                    let strip_backslash = &line[..(line.len() - 1)];
                    compilable_unit.push_str(strip_backslash);
                } else if line == "exit()" {
                    std::process::exit(0);
                } else {
                    compilable_unit.push_str(&line);
                    run(&compilable_unit, &mut interpreter);
                    compilable_unit.clear();
                }
                rl.add_history_entry(line);
            }
            Err(ReadlineError::Interrupted) => {
                println!("Interrupted. Type exit() to quit.");
            }
            Err(ReadlineError::Eof) => {
                println!("Eof Encountered");
            }
            Err(err) => {
                println!("An error occurred {}", err);
            }
        }
    }
}

fn run(source: &String, interpreter: &mut interpreter::VM) {
    match scanner::scan_tokens(source) {
        Ok(tokens) => {
            let mut compiler = compiler::Compiler::new(tokens, interpreter.take_virtual_memory());
            if let Ok(main) = compiler.compile() {
                let heap = compiler.heap;
                println!("{:?}", main.chunk.code);
                if let Err(e) = interpreter.interpret(main, heap) {
                    println!("An error ocurred while interpreting.");
                    println!("Runtime Error: {}", e)
                }
            } else {
                let heap = compiler.heap;
                interpreter.give_virtual_memory(heap);
            }
        }
        Err(error) => {
            println!("An error ocurred while scanning.");
            println!("{}", error);
        }
    }
}
