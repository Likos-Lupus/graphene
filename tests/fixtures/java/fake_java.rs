use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    thread,
    time::Duration,
};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let executable_name = env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_default();

    if args.iter().any(|arg| arg == "-version") {
        if executable_name.contains("timeout") {
            thread::sleep(Duration::from_secs(5));
        }

        if executable_name.contains("malformed") {
            eprintln!("fixture probe intentionally malformed");
            return;
        }

        eprintln!("Property settings:");
        eprintln!("    java.version = 21.0.4");
        eprintln!("    java.vendor = Graphene Fixture Vendor");
        eprintln!("    os.arch = {}", env::consts::ARCH);
        eprintln!("    java.home = fixture-java-home");
        eprintln!("openjdk version \"21.0.4\"");
        return;
    }

    if let Ok(path) = env::var("GRAPHENE_FAKE_JAVA_CAPTURE") {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .expect("open capture file");
        for arg in &args {
            writeln!(file, "{arg}").expect("write captured argument");
        }
    }

    if let Some(path) = args.iter().find_map(|arg| arg.strip_prefix("--fake-capture=")) {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .expect("open argument capture file");
        for arg in &args {
            writeln!(file, "{arg}").expect("write captured argument");
        }
    }

    if let Some(output) = args
        .windows(2)
        .find_map(|pair| (pair[0] == "--out").then_some(pair[1].as_str()))
    {
        if let Some(parent) = std::path::Path::new(output).parent() {
            fs::create_dir_all(parent).expect("create fake processor output parent");
        }
        fs::write(output, b"generated-content").expect("write fake processor output");
    }

    println!("fixture stdout");
    eprintln!("fixture stderr");
    let _ = io::stdout().write_all(&[0xff, b'\n']);
    if let Some(count) = args.iter().find_map(|arg| {
        arg.strip_prefix("--fake-spam=")
            .and_then(|value| value.parse::<usize>().ok())
    }) {
        for index in 0..count.min(100_000) {
            println!("fixture spam stdout {index}");
            eprintln!("fixture spam stderr {index}");
        }
    }

    if args.iter().any(|arg| arg == "--fake-sleep") {
        thread::sleep(Duration::from_secs(30));
    }

    if let Some(code) = args.iter().find_map(|arg| {
        arg.strip_prefix("--fake-exit=")
            .and_then(|value| value.parse::<i32>().ok())
    }) {
        std::process::exit(code);
    }
}
