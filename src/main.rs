use clap::Parser;
use std::{env, fmt, fs, io, path, process::Command};

// "Hello World!" program:
// ++++++++++[>+++++++>++++++++++>+++>+<<<<-]>++.>+.+++++++..+++.>++.<<+++++++++++++++.>.+++.------.--------.>+.>.

#[derive(Parser)]
pub enum Args {
    #[command(about = "enter a repl, only works if you have \"nasm\" and \"ld\"")]
    Repl,
    #[command(about = "compile a brainf*ck program to x86 assembly for linux")]
    Compile {
        src: String,
        #[arg(short = 'o', long)]
        out: String,
    },
}

fn main() {
    env_logger::init();
    if let Args::Compile { src, out } = Args::parse() {
        let x86 = generate_x86(&fs::read_to_string(src).unwrap()).unwrap();
        fs::write(out, x86).unwrap();
        return;
    }

    loop {
        print!(">> ");

        use io::Write;
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if input.as_bytes()[0] == b'q' {
            break;
        }

        let x86 = generate_x86(&input).unwrap();
        let temp_dir = env::temp_dir().join("brainfc");
        if !temp_dir.exists() {
            fs::create_dir(&temp_dir).unwrap();
        }

        let asm_path = temp_dir.join("main.asm");
        fs::write(&asm_path, x86).unwrap();

        let object_path = temp_dir.join("main.o");
        Command::new("nasm")
            .args(["-f", "elf64"])
            .args(["-o", object_path.to_str().unwrap()])
            .arg(asm_path)
            .stderr(io::stderr())
            .stdout(io::stdout())
            .output()
            .unwrap();

        let exec_path = format!("{}/main", temp_dir.display());
        Command::new("ld")
            .args(["-o", &exec_path])
            .arg(object_path)
            .stderr(io::stderr())
            .stdout(io::stdout())
            .output()
            .unwrap();

        let Ok(_) = Command::new(exec_path)
            .stderr(io::stderr())
            .stdout(io::stdout())
            .output()
        else {
            continue;
        };
    }
}

#[derive(Debug)]
pub enum GenerateX86Error {
    FmtError(fmt::Error),
    BracketMismatch,
}

impl From<fmt::Error> for GenerateX86Error {
    fn from(value: fmt::Error) -> Self {
        Self::FmtError(value)
    }
}

fn generate_x86(src: &str) -> Result<String, GenerateX86Error> {
    let mut x86 = String::new();

    use fmt::Write;
    writeln!(x86, "global _start")?;
    writeln!(x86, "_start:")?;
    writeln!(x86, "    mov rbp, rsp")?;

    // Has to be multiple of 16
    let buffer_size = 16 * 20;
    writeln!(x86, "    sub rsp, {buffer_size}")?;

    writeln!(x86, "    mov rax, -16")?;
    // Zero out the xmm0 register
    writeln!(x86, "    pxor xmm0, xmm0")?;
    writeln!(x86, "    mov rax, -16")?;
    writeln!(x86, "init_zero_loop:")?;
    writeln!(x86, "    movdqa [rbp + rax], xmm0")?;
    writeln!(x86, "    sub rax, 16")?;

    writeln!(x86, "    cmp rax, {buffer_size}")?;
    writeln!(x86, "    jb init_zero_loop")?;

    // The idea here is to work on rbx (bl) and only if the pointer is moved
    // save the value to the correct slot.
    writeln!(x86, "    mov rax, 0")?;
    writeln!(x86, "    mov rbx, 0")?;

    let mut label_index = -1;
    let mut scope_stack = Vec::new();
    for byte in src.as_bytes() {
        match byte {
            b'+' => {
                writeln!(x86, "    inc bl")?;
            }
            b'-' => {
                writeln!(x86, "    dec bl")?;
            }
            b'.' => {
                // load the value thats currently evaluated to the slot
                writeln!(x86, "    mov byte [rbp + rax - 9], bl")?;

                // move the slot address into rsi
                writeln!(x86, "    lea rsi, [rbp + rax - 9]")?;

                // save rax for later
                writeln!(x86, "    mov qword [rbp - 8], rax")?;

                // printing setup
                writeln!(x86, "    mov rax, 1")?;
                writeln!(x86, "    mov rdi, 1")?;
                writeln!(x86, "    mov rdx, 1")?;

                writeln!(x86, "    syscall")?;
                writeln!(x86, "    mov rax, qword [rbp - 8]")?;
            }
            b',' => {
                todo!("take a byte as input and insert at current slot")
            }
            byte @ (b'<' | b'>') => {
                // the 9 here is because the first 8 bytes are reserved for saving rax
                // when doing syscalls
                writeln!(x86, "    mov byte [rbp + rax - 9], bl")?;

                // dec and inc is swapped because of the addressing (have to "+ rax" and not "- rax" for some reason)
                let instruction = match byte {
                    b'<' => "inc",
                    b'>' => "dec",
                    _ => unreachable!(),
                };
                writeln!(x86, "    {instruction} rax")?;
                writeln!(x86, "    mov bl, byte [rbp + rax - 9]")?;
            }
            b'[' => {
                label_index += 1;
                scope_stack.push(label_index);

                writeln!(x86, "l{label_index}:")?;
                writeln!(x86, "    cmp bl, 0")?;
                writeln!(x86, "    jle r{label_index}")?;
            }
            b']' => {
                let Some(label_index) = scope_stack.pop() else {
                    panic!("square brace mismatch");
                };

                writeln!(x86, "    jmp l{label_index}")?;
                writeln!(x86, "r{label_index}:")?;
            }
            _ => {}
        }
    }

    writeln!(x86, "    mov rax, 60")?;
    writeln!(x86, "    syscall")?;

    Ok(x86)
}
