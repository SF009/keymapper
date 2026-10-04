mod config;
mod control;
mod input;
mod touch;

use config::Config;
use input::{spawn_input,InputKind,RuntimeControl};
use std::{env,error::Error,fs,path::PathBuf,sync::{Arc,Mutex},thread,time::Duration};
use touch::Mapper;

fn print_help(){
 eprintln!("Waydroid Keymapper — low-latency keyboard/mouse mapper");
 eprintln!();
 eprintln!("Usage:");
 eprintln!("  waydroid-keymapper [run] [CONFIG]");
 eprintln!("  waydroid-keymapper check [CONFIG]");
 eprintln!("  waydroid-keymapper devices");
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
  for (path,d) in evdev::enumerate(){println!("{}\t{}",path.display(),d.name().unwrap_or("-"));}
  return Ok(())
 }
 if !matches!(cmd.as_str(),"run"|"check"){
  print_help();
  return Err(format!("unknown command: {cmd}").into())
 }
 let path=PathBuf::from(a.next().unwrap_or_else(||env::var("WAYDROID_KEYMAPPER_CONFIG").unwrap_or_else(|_|format!("{}/.config/waydroid-keymapper/config.toml",env::var("HOME").unwrap_or_else(|_|".".into())))));
 let data=fs::read_to_string(&path).map_err(|e|format!("cannot read config '{}': {e}. Open the GTK GUI to create/manage it.",path.display()))?;
 let cfg:Config=toml::from_str(&data)?;
 if cmd=="check"{
  let conflicts=cfg.conflicts();
  if !conflicts.is_empty(){
   eprintln!("input conflicts:");
   for x in conflicts{eprintln!("  - {x}")}
   return Err("configuration has input conflicts".into())
  }
  cfg.validate()?;
  println!("configuration OK");
  println!("touch fifo: {}",cfg.touch_fifo());
  println!("pointer fifo: {}",cfg.pointer_fifo());
  println!("mouse lock: {} (toggle {})",cfg.performance.mouse_lock,cfg.performance.mouse_toggle_key);
  return Ok(())
 }
 if cmd!="run"{eprintln!("usage: waydroid-keymapper <run|check|devices> [config]");return Ok(())}
 cfg.validate()?;
 let mapper=Arc::new(Mutex::new(Mapper::new(cfg.clone())?));
 let control=RuntimeControl::new(cfg.performance.mouse_lock)?;
 if let Some(d)=cfg.devices.keyboard.clone(){spawn_input(d,InputKind::Keyboard,mapper.clone(),control.clone())?}
 if let Some(d)=cfg.devices.mouse.clone(){spawn_input(d,InputKind::Mouse,mapper.clone(),control.clone())?}
 let _=control::spawn_server(mapper.clone(),control.clone());
 eprintln!("waydroid-keymapper: running");
 while !control::shutdown_requested(){thread::sleep(Duration::from_millis(200))}
 if let Ok(mut m)=mapper.lock(){
  m.reset_keyboard_state();
  m.reset_mouse_state();
 }
 control::remove_socket();
 eprintln!("waydroid-keymapper: stopped cleanly");
 Ok(())
}
