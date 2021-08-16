use std::env;
use std::io::{Error, ErrorKind, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::exit;

use fastcgi::Request;

fn print_help() {
    println!(
        "{} version {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    println!("Usage:");
    println!(
        "\t{} <PATH>",
        env::args()
            .nth(0)
            .unwrap_or(env!("CARGO_PKG_NAME").to_string())
    );
    println!("Where PATH is a path to the unix socket to serve");
}

fn serve_file(req: &mut Request) -> Result<(), Error> {
    let file = std::fs::File::open(req.param("DOCUMENT_PATH").ok_or(Error::new(
        std::io::ErrorKind::Other,
        "Missing Path FastCGI Parameter",
    ))?)?;

    let mut filebuf: Vec<u8> = vec![];
    xz2::read::XzDecoder::new(file).read_to_end(&mut filebuf)?;

    dbg!(&filebuf.len());

    req.stdout()
        .write("Content-Type: application/octet-stream\r\n\r\n".as_bytes())?;
    req.stdout().write(filebuf.as_slice())?;
    req.stdout().write("\r\n\r\n".as_bytes())?;

    Ok(())
}

fn main() {
    let mut args = env::args();
    if args.len() < 2
        || args
            .find(|arg| (arg == "-h") || (arg == "--help"))
            .is_some()
    {
        print_help();
        exit(0)
    }

    let socket = {
        let socket_path = PathBuf::from(env::args().nth(1).unwrap());
        let sock = UnixListener::bind(socket_path.clone());
        match sock {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Could not bind to {}: {}", socket_path.to_string_lossy(), e);
                exit(74); // IOERR exit code
            }
        }
    };

    // This is stupid but doesn't matter enough to change
    let path: PathBuf = socket
        .local_addr()
        .unwrap()
        .as_pathname()
        .unwrap()
        .to_path_buf();
    ctrlc::set_handler(move || {
        std::fs::remove_file(path.clone()).unwrap();
        exit(0);
    })
    .unwrap();

    println!(
        "Listening on {:?}",
        socket.local_addr().unwrap().as_pathname().unwrap()
    );

    fastcgi::run_raw(
        |mut req| {
            match serve_file(&mut req) {
                Ok(_) => (),
                Err(e) => {
                    if e.kind() != ErrorKind::BrokenPipe {
                        eprintln!("{}", e);
                        req.stderr()
                            .write("Status: 500 Internal Server Error\r\n\r\n".as_bytes())
                            .unwrap();
                    }
                }
            };
        },
        socket.as_raw_fd(),
    );

    std::fs::remove_file(socket.local_addr().unwrap().as_pathname().unwrap()).unwrap();
}
