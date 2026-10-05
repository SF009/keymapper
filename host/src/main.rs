mod protocol;

use evdev::{Device, EventSummary, RelativeAxisCode};
use std::{
    env,
    io::{self, Write},
    net::TcpStream,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

const DEFAULT_PORT: u16 = 27183;

fn usage() {
    println!(r#"Waydroid Keymapper host

  --list
  --serial <SERIAL>
  --keyboard <PATH>
  --mouse <PATH>
  --no-grab
  --port <PORT>
"#);
}

fn adb(serial: Option<&str>, args: &[String]) -> io::Result<std::process::Output> {
    let mut c = Command::new("adb");
    if let Some(s) = serial {
        c.arg("-s").arg(s);
    }
    c.args(args).output()
}

fn forward(serial: Option<&str>, port: u16) -> io::Result<()> {
    let local = format!("tcp:{port}");
    let remote = local.clone();

    let _ = adb(
        serial,
        &vec!["forward".into(), "--remove".into(), local.clone()],
    );

    let out = adb(
        serial,
        &vec!["forward".into(), local, remote],
    )?;

    if !out.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }

    Ok(())
}

fn start_gateway(serial: Option<&str>) {
    let _ = adb(
        serial,
        &vec![
            "shell".into(),
            "am".into(),
            "start-foreground-service".into(),
            "-n".into(),
            "io.sf009.waydroidkeymapper/.input.InputGatewayService".into(),
        ],
    );
}

fn list_devices() {
    for (path, dev) in evdev::enumerate() {
        println!("{}  {}", path.to_string_lossy(), dev.name().unwrap_or("input"));
    }
}

fn looks_like(dev: &Device, mouse: bool) -> bool {
    let keys = dev.supported_keys();
    let rel = dev.supported_relative_axes();

    if mouse {
        keys
            .as_ref()
            .map(|k| k.contains(evdev::KeyCode::BTN_LEFT))
            .unwrap_or(false)
            && rel
                .as_ref()
                .map(|a| a.contains(RelativeAxisCode::REL_X))
                .unwrap_or(false)
    } else {
        keys
            .as_ref()
            .map(|k| {
                k.contains(evdev::KeyCode::KEY_A) &&
                    k.contains(evdev::KeyCode::KEY_W)
            })
            .unwrap_or(false)
    }
}

fn choose(mouse: bool) -> Option<String> {
    let mut candidates = Vec::new();

    for (path, dev) in evdev::enumerate() {
        if !looks_like(&dev, mouse) {
            continue;
        }

        let name = dev.name().unwrap_or("input").to_ascii_lowercase();
        let mut score = 100i32;

        if name.contains("virtual") {
            score -= 50;
        }
        if mouse && name.contains("touchpad") {
            score -= 40;
        }
        if !mouse && name.contains("keyboard") {
            score += 20;
        }

        candidates.push((score, path.to_string_lossy().to_string()));
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    candidates.first().map(|x| x.1.clone())
}

fn send_frame(s: &mut TcpStream, ty: u8, payload: &[u8]) -> io::Result<()> {
    let len = u16::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "payload too large"))?;

    let mut header = [0u8; 8];
    header[0..4].copy_from_slice(&protocol::MAGIC.to_be_bytes());
    header[4] = protocol::VERSION;
    header[5] = ty;
    header[6..8].copy_from_slice(&len.to_be_bytes());

    s.write_all(&header)?;
    s.write_all(payload)?;
    Ok(())
}

fn connect(port: u16) -> TcpStream {
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_keepalive(None);
                return stream;
            }
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn keyboard(path: String, locked: Arc<AtomicBool>, grab: bool, port: u16) {
    let mut dev =
        Device::open(&path).unwrap_or_else(|e| panic!("keyboard {path}: {e}"));

    if grab {
        let _ = dev.grab();
    }

    let mut stream = connect(port);
    let _ = send_frame(&mut stream, protocol::HELLO, b"keyboard");

    loop {
        let events = match dev.fetch_events() {
            Ok(events) => events,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("keyboard: {e}");
                break;
            }
        };

        for ev in events {
            if let EventSummary::Key(_, code, value) = ev.destructure() {
                let state = match value {
                    1 => 1u8,
                    0 => 0u8,
                    _ => 2u8,
                };

                if code == evdev::KeyCode::KEY_F8 && state == 1 {
                    locked.fetch_xor(true, Ordering::AcqRel);
                    continue;
                }

                let payload = [(code.0 >> 8) as u8, code.0 as u8, state];

                if send_frame(&mut stream, protocol::KEY, &payload).is_err() {
                    stream = connect(port);
                    let _ = send_frame(&mut stream, protocol::HELLO, b"keyboard");
                }
            }
        }
    }
}

fn mouse(path: String, locked: Arc<AtomicBool>, grab: bool, port: u16) {
    let mut dev =
        Device::open(&path).unwrap_or_else(|e| panic!("mouse {path}: {e}"));

    let mut captured = false;
    let mut stream = connect(port);
    let _ = send_frame(&mut stream, protocol::HELLO, b"mouse");

    loop {
        let desired = locked.load(Ordering::Acquire);

        if desired != captured {
            let result = if grab {
                if desired {
                    dev.grab()
                } else {
                    dev.ungrab()
                }
            } else {
                Ok(())
            };

            if result.is_ok() {
                captured = desired;
            }
        }

        if !captured {
            thread::sleep(Duration::from_millis(1));
            continue;
        }

        let events = match dev.fetch_events() {
            Ok(events) => events,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("mouse: {e}");
                break;
            }
        };

        for ev in events {
            match ev.destructure() {
                EventSummary::RelativeAxis(_, axis, value) => {
                    let (dx, dy) = if axis == RelativeAxisCode::REL_X {
                        (value, 0)
                    } else if axis == RelativeAxisCode::REL_Y {
                        (0, value)
                    } else {
                        (0, 0)
                    };

                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let xb = (dx as i16).to_be_bytes();
                    let yb = (dy as i16).to_be_bytes();
                    let payload = [xb[0], xb[1], yb[0], yb[1]];

                    if send_frame(&mut stream, protocol::MOUSE_MOVE, &payload).is_err() {
                        stream = connect(port);
                        let _ = send_frame(&mut stream, protocol::HELLO, b"mouse");
                    }
                }

                EventSummary::Key(_, code, value)
                    if code.0 >= evdev::KeyCode::BTN_LEFT.0 =>
                {
                    let state = match value {
                        1 => 1u8,
                        0 => 0u8,
                        _ => 2u8,
                    };
                    let payload = [(code.0 >> 8) as u8, code.0 as u8, state];

                    if send_frame(&mut stream, protocol::MOUSE_BUTTON, &payload).is_err() {
                        stream = connect(port);
                        let _ = send_frame(&mut stream, protocol::HELLO, b"mouse");
                    }
                }

                _ => {}
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        return;
    }

    let mut serial = None;
    let mut keyboard_path = None;
    let mut mouse_path = None;
    let mut port = DEFAULT_PORT;
    let mut grab = true;
    let mut list = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--list" => list = true,
            "--no-grab" => grab = false,
            "--serial" | "--keyboard" | "--mouse" | "--port"
                if i + 1 < args.len() =>
            {
                let value = args[i + 1].clone();

                match args[i].as_str() {
                    "--serial" => serial = Some(value),
                    "--keyboard" => keyboard_path = Some(value),
                    "--mouse" => mouse_path = Some(value),
                    "--port" => port = value.parse().unwrap_or(DEFAULT_PORT),
                    _ => {}
                }

                i += 1;
            }
            _ => {}
        }

        i += 1;
    }

    if list {
        list_devices();
        return;
    }

    let keyboard_path = keyboard_path
        .or_else(|| choose(false))
        .expect("No keyboard device found; use --keyboard");

    let mouse_path = mouse_path
        .or_else(|| choose(true))
        .expect("No mouse device found; use --mouse");

    start_gateway(serial.as_deref());

    if let Err(e) = forward(serial.as_deref(), port) {
        eprintln!("adb forward failed: {e}");
        std::process::exit(2);
    }

    let locked = Arc::new(AtomicBool::new(true));

    {
        let lock = Arc::clone(&locked);
        let path = keyboard_path.clone();
        thread::spawn(move || keyboard(path, lock, grab, port));
    }

    {
        let lock = Arc::clone(&locked);
        thread::spawn(move || mouse(mouse_path, lock, grab, port));
    }

    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
