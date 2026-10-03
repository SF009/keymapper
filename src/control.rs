use crate::input::RuntimeControl;
use crate::touch::Mapper;
use std::{
    env,
    error::Error,
    fs,
    io::{self,Read,Write},
    os::unix::net::{UnixListener,UnixStream},
    path::PathBuf,
    sync::{Arc,Mutex},
    thread,
};

pub fn socket_path()->PathBuf{
    if let Some(dir)=env::var_os("XDG_RUNTIME_DIR"){
        return PathBuf::from(dir).join("waydroid-keymapper.sock");
    }
    PathBuf::from(format!("/tmp/waydroid-keymapper-{}.sock",unsafe{libc::getuid()}))
}

pub fn request(command:&str)->Result<String,Box<dyn Error>>{
    let mut stream=UnixStream::connect(socket_path())?;
    stream.write_all(command.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.shutdown(std::net::Shutdown::Write).ok();
    let mut out=String::new();
    stream.read_to_string(&mut out)?;
    Ok(out.trim().to_string())
}

fn handle(mut stream:UnixStream,mapper:&Arc<Mutex<Mapper>>,control:&Arc<RuntimeControl>){
    let mut buf=String::new();
    if stream.read_to_string(&mut buf).is_err(){return}
    let command=buf.lines().next().unwrap_or_default().trim().to_ascii_lowercase();
    let reply=match command.as_str(){
        "status"=>{
            let locked=mapper.lock().map(|m|m.is_mouse_locked()).unwrap_or(false);
            format!("OK running=1 locked={} socket={}",if locked{1}else{0},socket_path().display())
        }
        "lock"=>{
            let can_grab=mapper.lock().map(|m|m.config().performance.grab).unwrap_or(false);
            if !can_grab{
                "ERR cannot-lock: exclusive input grab is disabled".to_string()
            }else{
                control.mouse_locked.store(true,std::sync::atomic::Ordering::Release);
                control.notify_mouse();
                "OK requested=lock".to_string()
            }
        }
        "unlock"=>{
            control.mouse_locked.store(false,std::sync::atomic::Ordering::Release);
            control.notify_mouse();
            "OK requested=unlock".to_string()
        }
        "toggle"=>{
            let next=!control.mouse_locked.load(std::sync::atomic::Ordering::Acquire);
            control.mouse_locked.store(next,std::sync::atomic::Ordering::Release);
            control.notify_mouse();
            format!("OK requested={}",if next{"lock"}else{"unlock"})
        }
        "ping"=>"OK pong".to_string(),
        _=>"ERR unknown-command (status|lock|unlock|toggle|ping)".to_string(),
    };
    let _=stream.write_all(reply.as_bytes());
}

pub fn spawn_server(mapper:Arc<Mutex<Mapper>>,control:Arc<RuntimeControl>)->io::Result<()>{
    let path=socket_path();
    if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
    match fs::remove_file(&path){
        Ok(())=>{},
        Err(e) if e.kind()==io::ErrorKind::NotFound=>{},
        Err(e)=>return Err(e),
    }
    let listener=UnixListener::bind(&path)?;
    let _=fs::set_permissions(&path,fs::Permissions::from_mode(0o600));
    thread::spawn(move||{
        for stream in listener.incoming(){
            match stream{
                Ok(s)=>{
                    let m=mapper.clone();
                    let c=control.clone();
                    thread::spawn(move||handle(s,&m,&c));
                }
                Err(e)=>eprintln!("waydroid-keymapper: control socket error: {e}"),
            }
        }
        let _=fs::remove_file(socket_path());
    });
    eprintln!("waydroid-keymapper: control socket {}",path.display());
    Ok(())
}

use std::os::unix::fs::PermissionsExt;
