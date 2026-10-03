mod config;
mod control;
mod input;
mod touch;

use config::Config;
use input::{spawn_input,InputKind,RuntimeControl};
use std::{env,error::Error,fs,sync::{Arc,Mutex},thread,time::Duration};
use touch::Mapper;

fn main()->Result<(),Box<dyn Error>>{
 // Waydroid's FIFO reader can disappear during a restart. Ignore SIGPIPE so
 // the mapper receives EPIPE and can reconnect instead of being terminated.
 unsafe{libc::signal(libc::SIGPIPE,libc::SIG_IGN);}

 let mut a=env::args().skip(1);
 let cmd=a.next().unwrap_or_else(||"run".into());
 let path=a.next().unwrap_or_else(||env::var("WAYDROID_KEYMAPPER_CONFIG").unwrap_or_else(|_|format!("{}/.config/waydroid-keymapper/config.toml",env::var("HOME").unwrap_or_else(|_|".".into()))));
 if cmd=="devices"{for (_path,d) in evdev::enumerate(){println!("{}  {}",d.physical_path().unwrap_or("-"),d.name().unwrap_or("-"));}return Ok(())}
 let cfg:Config=toml::from_str(&fs::read_to_string(path)?)?;
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
 eprintln!("waydroid-keymapper: running");
 loop{thread::sleep(Duration::from_secs(3600))}
}
