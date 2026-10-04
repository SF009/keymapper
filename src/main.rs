use waydroid_keymapper::{
 config::Config,
 input::{spawn_input,InputKind,RuntimeControl},
};
use waydroid_keymapper::{control,touch::Mapper};
use std::{env,error::Error,fs,path::PathBuf,sync::{Arc,Mutex},thread,time::Duration};

fn print_help(){
 eprintln!("Waydroid Keymapper — low-latency keyboard/mouse mapper");
 eprintln!();
 eprintln!("Usage:");
 eprintln!("  waydroid-keymapper [run] [CONFIG]");
 eprintln!("  waydroid-keymapper check [CONFIG]");
 eprintln!("  waydroid-keymapper devices");
 eprintln!("  waydroid-keymapper doctor [CONFIG]");
 eprintln!("  waydroid-keymapper --help");
}

fn main()->Result<(),Box<dyn Error>>{
 control::install_signal_handlers();
 // Waydroid's FIFO reader can disappear during a restart. Ignore SIGPIPE so
 // the mapper receives EPIPE and can reconnect instead of being terminated.
 unsafe{libc::signal(libc::SIGPIPE,libc::SIG_IGN);}

 let mut a=env::args().skip(1);
 let cmd=a.next().unwrap_or_else(||"run".into());
 if matches!(cmd.as_str(),"--help"|"-h"|"help"){print_help();return Ok(())}
 if matches!(cmd.as_str(),"--version"|"-V"){
  println!("{}",env!("CARGO_PKG_VERSION"));
  return Ok(())
 }
 if cmd=="devices"{
  for d in input::list_input_devices(){println!("{}\t{}",d.path,d.name);}
  return Ok(())
 }
 if !matches!(cmd.as_str(),"run"|"check"|"doctor"){
  print_help();
  return Err(format!("unknown command: {cmd}").into())
 }
 let path=PathBuf::from(a.next().unwrap_or_else(||env::var("WAYDROID_KEYMAPPER_CONFIG").unwrap_or_else(|_|format!("{}/.config/waydroid-keymapper/config.toml",env::var("HOME").unwrap_or_else(|_|".".into())))));
 let data=fs::read_to_string(&path).map_err(|e|format!("cannot read config '{}': {e}. Open the GTK GUI to create/manage it.",path.display()))?;
 let cfg:Config=toml::from_str(&data)?;

 if cmd=="doctor"{
  let mut issues=Vec::<String>::new();
  if let Err(e)=cfg.validate_runtime(){issues.push(format!("config: {e}"));}
  for (kind,path_opt) in [("keyboard",&cfg.devices.keyboard),("mouse",&cfg.devices.mouse)]{
   if let Some(path)=path_opt{
    if let Err(e)=evdev::Device::open(path){issues.push(format!("{kind} evdev: {e}"));}
   }
  }
  let touch=cfg.touch_fifo();
  if !std::path::Path::new(&touch).exists(){issues.push(format!("touch FIFO missing: {touch}"));}
  if cfg.aim.as_ref().is_some_and(|a|a.mode.eq_ignore_ascii_case("relative")){
   let pointer=cfg.pointer_fifo();
   if !std::path::Path::new(&pointer).exists(){issues.push(format!("pointer FIFO missing: {pointer}"));}
  }
  match std::process::Command::new("waydroid").arg("status").output(){
   Ok(o)=>{
    let s=String::from_utf8_lossy(&o.stdout).trim().replace('\n'," | ");
    println!("waydroid: {}",if s.is_empty(){"unknown"}else{&s});
   }
   Err(e)=>issues.push(format!("waydroid command: {e}")),
  }
  if issues.is_empty(){println!("doctor: all checks passed ✓");return Ok(())}
  eprintln!("doctor: {} issue(s)",issues.len());
  for x in issues{eprintln!("  - {x}")}
  return Err("Waydroid Keymapper doctor found problems".into())
 }
 if cmd=="check"{
  let conflicts=cfg.conflicts();
  if !conflicts.is_empty(){
   eprintln!("input conflicts:");
   for x in conflicts{eprintln!("  - {x}")}
   return Err("configuration has input conflicts".into())
  }
  cfg.validate_runtime()?;
  println!("configuration OK");
  println!("touch fifo: {}",cfg.touch_fifo());
  println!("pointer fifo: {}",cfg.pointer_fifo());
  println!("mouse lock: {} (toggle {})",cfg.performance.mouse_lock,cfg.performance.mouse_toggle_key);
  return Ok(())
 }
 if cmd!="run"{eprintln!("usage: waydroid-keymapper <run|check|devices> [config]");return Ok(())}
 cfg.validate_runtime()?;
 let mapper=Arc::new(Mutex::new(Mapper::new(cfg.clone())?));
 let control=RuntimeControl::new(cfg.performance.mouse_lock)?;

 // Bind the control endpoint before touching any input device. This makes
 // duplicate daemon instances fail before they can compete for evdev input.
 if let Err(e)=control::spawn_server(mapper.clone(),control.clone()){
  eprintln!("waydroid-keymapper: cannot start control server: {e}");
  return Err(e.into())
 }

 if let Some(d)=cfg.devices.keyboard.clone(){
  if let Err(e)=spawn_input(d,InputKind::Keyboard,mapper.clone(),control.clone()){
   control::remove_socket();
   return Err(e)
  }
 }
 if let Some(d)=cfg.devices.mouse.clone(){
  if let Err(e)=spawn_input(d,InputKind::Mouse,mapper.clone(),control.clone()){
   control::remove_socket();
   return Err(e)
  }
 }
 eprintln!("waydroid-keymapper: running (Aim auto-lock={}, manual toggle={})",cfg.performance.auto_lock_on_aim,cfg.performance.mouse_toggle_key);
 while !control::shutdown_requested(){thread::sleep(Duration::from_millis(200))}
 if let Ok(mut m)=mapper.lock(){
  m.reset_keyboard_state();
  m.reset_mouse_state();
 }
 control::remove_socket();
 eprintln!("waydroid-keymapper: stopped cleanly");
 Ok(())
}
