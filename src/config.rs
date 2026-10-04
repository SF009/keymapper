use serde::{Deserialize,Serialize};
use std::{env,error::Error,path::Path};

#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct Config{
 pub display:Display,
 pub devices:Devices,
 #[serde(default)]pub joystick:Option<Joystick>,
 #[serde(default)]pub aim:Option<Aim>,
 #[serde(default)]pub taps:Vec<Tap>,
 #[serde(default)]pub holds:Vec<Hold>,
 #[serde(default)]pub mouse_taps:Vec<MouseTap>,
 #[serde(default)]pub mouse_holds:Vec<MouseHold>,
 #[serde(default)]pub performance:Performance,#[serde(default)]pub touch:TouchSettings
}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Display{pub width:i32,pub height:i32}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Devices{pub keyboard:Option<String>,pub mouse:Option<String>}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Joystick{pub up:String,pub down:String,pub left:String,pub right:String,pub center_x:f32,pub center_y:f32,pub radius:f32,#[serde(default="dtrue")]pub normalize_diagonal:bool,#[serde(default)]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Aim{pub button:String,pub center_x:f32,pub center_y:f32,#[serde(default="ds")]pub sensitivity:f32,#[serde(default="s1")]pub slot:u8,#[serde(default)]pub invert_x:bool,#[serde(default)]pub invert_y:bool,#[serde(default="one")]pub scale_x:f32,#[serde(default="one")]pub scale_y:f32,#[serde(default="edge")]pub edge_margin:f32,#[serde(default="touch_mode")]pub mode:String}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Tap{pub key:String,pub x:f32,pub y:f32,#[serde(default="s2")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Hold{pub key:String,pub x:f32,pub y:f32,#[serde(default="s3")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct MouseTap{pub button:String,pub x:f32,pub y:f32,#[serde(default="s4")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct MouseHold{pub button:String,pub x:f32,pub y:f32,#[serde(default="s5")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Performance{#[serde(default="dt")]pub grab:bool,#[serde(default="dt")]pub realtime:bool,#[serde(default="prio")]pub realtime_priority:i32,#[serde(default="dm")]pub mouse_lock:bool,#[serde(default="f8")]pub mouse_toggle_key:String,#[serde(default="wr")]pub fifo_write_retries:u8,#[serde(default="ww")]pub fifo_write_wait_ms:u64,#[serde(default="rb")]pub fifo_reconnect_ms:u64}
fn ds()->f32{1.0} fn one()->f32{1.0} fn edge()->f32{0.12} fn s1()->u8{1} fn s2()->u8{2} fn s3()->u8{3} fn s4()->u8{4} fn s5()->u8{5} fn dt()->bool{true} fn dtrue()->bool{true} fn dm()->bool{false} fn f8()->String{"F8".into()} fn prio()->i32{10} fn wr()->u8{3} fn ww()->u64{1} fn rb()->u64{25} fn tp()->i32{80} fn tm()->i32{8} fn touch_mode()->String{"touch".into()}
impl Default for Performance{fn default()->Self{Self{grab:true,realtime:true,realtime_priority:10,mouse_lock:false,mouse_toggle_key:"F8".into(),fifo_write_retries:3,fifo_write_wait_ms:1,fifo_reconnect_ms:25}}}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct TouchSettings{#[serde(default="tp")]pub pressure:i32,#[serde(default="tm")]pub major:i32,#[serde(default="tm")]pub minor:i32}
impl Default for TouchSettings{fn default()->Self{Self{pressure:80,major:8,minor:8}}}

impl Config{
 pub fn conflicts(&self)->Vec<String>{
  let mut out=Vec::new();
  let mut keys:Vec<(u16,String)>=Vec::new();
  let mut mice:Vec<(u16,String)>=Vec::new();
  if let Some(j)=&self.joystick{
   for (name,key) in [("joystick.up",&j.up),("joystick.down",&j.down),("joystick.left",&j.left),("joystick.right",&j.right)]{
    if let Ok(code)=crate::input::key_code(key){keys.push((code,name.into()));}
   }
  }
  for (i,x) in self.taps.iter().enumerate(){if let Ok(code)=crate::input::key_code(&x.key){keys.push((code,format!("taps[{i}]")));}}
  for (i,x) in self.holds.iter().enumerate(){if let Ok(code)=crate::input::key_code(&x.key){keys.push((code,format!("holds[{i}]")));}}
  if let Ok(toggle)=crate::input::key_code(&self.performance.mouse_toggle_key){keys.push((toggle,"performance.mouse_toggle_key".into()));}
  for i in 0..keys.len(){for j in (i+1)..keys.len(){if keys[i].0==keys[j].0{out.push(format!("keyboard conflict: {} <-> {}",keys[i].1,keys[j].1));}}}
  if let Some(a)=&self.aim{if let Ok(code)=crate::input::button_code(&a.button){mice.push((code,"aim.button".into()));}}
  for (i,x) in self.mouse_taps.iter().enumerate(){if let Ok(code)=crate::input::button_code(&x.button){mice.push((code,format!("mouse_taps[{i}]")));}}
  for (i,x) in self.mouse_holds.iter().enumerate(){if let Ok(code)=crate::input::button_code(&x.button){mice.push((code,format!("mouse_holds[{i}]")));}}
  for i in 0..mice.len(){for j in (i+1)..mice.len(){if mice[i].0==mice[j].0{out.push(format!("mouse conflict: {} <-> {}",mice[i].1,mice[j].1));}}}
  if let Some(j)=&self.joystick{
   let dirs=[("up",&j.up), ("down",&j.down), ("left",&j.left), ("right",&j.right)];
   for i in 0..dirs.len(){for k in (i+1)..dirs.len(){if dirs[i].1.eq_ignore_ascii_case(dirs[k].1){out.push(format!("joystick conflict: {} and {} use {}",dirs[i].0,dirs[k].0,dirs[i].1));}}}
  }
  out
 }
 pub fn validate(&self)->Result<(),Box<dyn Error>>{
  if self.display.width<=0||self.display.height<=0{return Err("invalid display size".into())}
  if self.display.width>16384||self.display.height>16384{return Err("display size is too large".into())}
  if let (Some(k),Some(m))=(&self.devices.keyboard,&self.devices.mouse){
   if !k.is_empty()&&!m.is_empty()&&k==m{return Err("keyboard and mouse cannot use the same evdev device".into())}
  }
  if self.performance.mouse_lock&&!self.performance.grab{
   return Err("mouse_lock requires performance.grab=true".into())
  }
  if self.performance.realtime_priority<1||self.performance.realtime_priority>99{return Err("realtime_priority must be 1..99".into())}
  if self.performance.fifo_write_retries==0||self.performance.fifo_write_retries>8{return Err("fifo_write_retries must be 1..8".into())}
  if self.performance.fifo_write_wait_ms>5{return Err("fifo_write_wait_ms must be 0..5 ms".into())}
  if self.performance.fifo_reconnect_ms<5||self.performance.fifo_reconnect_ms>2000{return Err("fifo_reconnect_ms must be 5..2000 ms".into())}
  if self.touch.pressure<1||self.touch.pressure>255||self.touch.major<1||self.touch.major>255||self.touch.minor<1||self.touch.minor>255{return Err("touch pressure/major/minor must be 1..255".into())}
  if let Some(j)=&self.joystick{
   for k in [&j.up,&j.down,&j.left,&j.right]{crate::input::key_code(k)?;}
   if !(0.0..=1.0).contains(&j.center_x)||!(0.0..=1.0).contains(&j.center_y)||j.radius<=0.0||j.radius>1.0{return Err("invalid joystick".into())}
   if j.slot>=16{return Err("joystick slot must be 0..15".into())}
  }
  let mut used=[false;16];
  let mut reserve=|slot:u8|->Result<(),Box<dyn Error>>{
   let i=slot as usize;
   if i>=used.len(){return Err("touch slot must be 0..15".into())}
   if used[i]{return Err(format!("duplicate touch slot {slot}").into())}
   used[i]=true;Ok(())
  };
  if let Some(j)=&self.joystick{
   reserve(j.slot)?;
  }
  if let Some(a)=&self.aim{
   reserve(a.slot)?;
   crate::input::button_code(&a.button)?;
   if !(0.0..=1.0).contains(&a.center_x)||!(0.0..=1.0).contains(&a.center_y)||a.sensitivity<=0.0||a.sensitivity>100.0||a.scale_x<=0.0||a.scale_x>20.0||a.scale_y<=0.0||a.scale_y>20.0||a.edge_margin<0.0||a.edge_margin>=0.5{return Err("invalid aim".into())}
   match a.mode.trim().to_ascii_lowercase().as_str(){"touch"|"relative"=>{},_=>return Err("aim mode must be touch or relative".into())}
  }
  for x in &self.taps{crate::input::key_code(&x.key)?;reserve(x.slot)?;if !(0.0..=1.0).contains(&x.x)||!(0.0..=1.0).contains(&x.y){return Err("invalid keyboard tap".into())}}
  for x in &self.holds{crate::input::key_code(&x.key)?;reserve(x.slot)?;if !(0.0..=1.0).contains(&x.x)||!(0.0..=1.0).contains(&x.y){return Err("invalid keyboard hold".into())}}
  for x in &self.mouse_taps{crate::input::button_code(&x.button)?;reserve(x.slot)?;if !(0.0..=1.0).contains(&x.x)||!(0.0..=1.0).contains(&x.y){return Err("invalid mouse tap".into())}}
  for x in &self.mouse_holds{crate::input::button_code(&x.button)?;reserve(x.slot)?;if !(0.0..=1.0).contains(&x.x)||!(0.0..=1.0).contains(&x.y){return Err("invalid mouse hold".into())}}
  crate::input::key_code(&self.performance.mouse_toggle_key)?;
  if let Some(msg)=self.conflicts().into_iter().next(){return Err(msg.into())}
  Ok(())
 }
 pub fn validate_runtime(&self)->Result<(),Box<dyn Error>>{
  self.validate()?;

  let keyboard_required=self.performance.mouse_lock||self.joystick.is_some()||!self.taps.is_empty()||!self.holds.is_empty();
  if keyboard_required&&self.devices.keyboard.as_deref().unwrap_or("").is_empty(){
   return Err("a keyboard device is required for the configured keyboard mappings".into())
  }

  let mouse_required=self.performance.mouse_lock||self.aim.is_some()||!self.mouse_taps.is_empty()||!self.mouse_holds.is_empty();
  if mouse_required&&self.devices.mouse.as_deref().unwrap_or("").is_empty(){
   return Err("a mouse device is required for the configured mouse mappings/lock".into())
  }

  Ok(())
 }

 pub fn touch_fifo(&self)->String{
  if let Ok(p)=env::var("WAYDROID_TOUCH_FIFO"){return p}
  let c=["/dev/input/wl_touch_events","/var/lib/waydroid/rootfs/dev/input/wl_touch_events","/opt/waydroid/rootfs/dev/input/wl_touch_events"];
  c.iter().find(|p|Path::new(p).exists()).map(|p|p.to_string()).unwrap_or_else(||c[0].into())
 }
 pub fn pointer_fifo(&self)->String{
  if let Ok(p)=env::var("WAYDROID_POINTER_FIFO"){return p}
  let c=["/dev/input/wl_pointer_events","/var/lib/waydroid/rootfs/dev/input/wl_pointer_events","/opt/waydroid/rootfs/dev/input/wl_pointer_events"];
  c.iter().find(|p|Path::new(p).exists()).map(|p|p.to_string()).unwrap_or_else(||c[0].into())
 }
}


#[cfg(test)]
mod tests{
 use super::*;

 fn base()->Config{
  Config{
   display:Display{width:1920,height:1080},
   devices:Devices{keyboard:None,mouse:None},
   joystick:None,aim:None,
   taps:Vec::new(),holds:Vec::new(),mouse_taps:Vec::new(),mouse_holds:Vec::new(),
   performance:Performance::default(),touch:TouchSettings::default(),
  }
 }

 #[test]
 fn default_performance_is_shooter_safe(){
  let p=Performance::default();
  assert!(p.grab&&p.realtime&&!p.mouse_lock);
  assert_eq!(p.mouse_toggle_key,"F8");assert_eq!(p.realtime_priority,10);assert_eq!(p.fifo_write_retries,3);assert_eq!(p.fifo_write_wait_ms,1);assert_eq!(p.fifo_reconnect_ms,25);
 }

 #[test]
 fn missing_mouse_lock_defaults_to_unlocked(){
  let p:Performance=toml::from_str("grab=true\nrealtime=true\n").unwrap();
  assert!(!p.mouse_lock);
 }

 #[test]
 fn relative_aim_is_valid(){
  let mut c=base();
  c.aim=Some(Aim{
   button:"MOUSE_RIGHT".into(),center_x:0.5,center_y:0.5,sensitivity:2.,
   slot:1,invert_x:false,invert_y:false,scale_x:1.,scale_y:1.,edge_margin:0.12,mode:"relative".into(),
  });
  assert!(c.validate().is_ok());
 }

 #[test]
 fn duplicate_physical_keyboard_input_is_rejected(){
  let mut c=base();
  c.joystick=Some(Joystick{
   up:"W".into(),down:"S".into(),left:"A".into(),right:"D".into(),
   center_x:0.15,center_y:0.76,radius:0.085,normalize_diagonal:true,slot:0,
  });
  c.holds.push(Hold{key:"W".into(),x:0.3,y:0.3,slot:2});
  assert!(c.conflicts().iter().any(|x|x.contains("keyboard conflict")));
  assert!(c.validate().is_err());
 }

 #[test]
 fn aim_and_fire_button_conflict_is_rejected(){
  let mut c=base();
  c.aim=Some(Aim{
   button:"MOUSE_LEFT".into(),center_x:0.5,center_y:0.5,sensitivity:2.,
   slot:1,invert_x:false,invert_y:false,scale_x:1.,scale_y:1.,edge_margin:0.12,mode:"touch".into(),
  });
  c.mouse_holds.push(MouseHold{button:"MOUSE_LEFT".into(),x:0.8,y:0.8,slot:2});
  assert!(c.conflicts().iter().any(|x|x.contains("mouse conflict")));
  assert!(c.validate().is_err());
 }

 #[test]
 fn mouse_lock_requires_grab(){
  let mut c=base();
  c.performance.mouse_lock=true;
  c.performance.grab=false;
  assert!(c.validate().is_err());
 }

 #[test]
 fn runtime_validation_requires_devices_for_active_mappings(){
  let mut c=base();
  c.joystick=Some(Joystick{
   up:"W".into(),down:"S".into(),left:"A".into(),right:"D".into(),
   center_x:0.15,center_y:0.76,radius:0.085,normalize_diagonal:true,slot:0,
  });
  assert!(c.validate().is_ok());
  assert!(c.validate_runtime().is_err());

  c.joystick=None;
  c.performance.mouse_lock=true;
  assert!(c.validate_runtime().is_err());

  c.devices.keyboard=Some("/dev/input/event0".into());
  c.aim=Some(Aim{
   button:"MOUSE_RIGHT".into(),center_x:0.5,center_y:0.5,sensitivity:2.,
   slot:1,invert_x:false,invert_y:false,scale_x:1.,scale_y:1.,edge_margin:0.12,mode:"relative".into(),
  });
  assert!(c.validate_runtime().is_err());

  c.devices.mouse=Some("/dev/input/event1".into());
  assert!(c.validate_runtime().is_ok());
 }

 #[test]
 fn unique_slots_are_accepted(){
  let mut c=base();
  c.taps.push(Tap{key:"SPACE".into(),x:0.8,y:0.8,slot:2});
  c.mouse_holds.push(MouseHold{button:"MOUSE_LEFT".into(),x:0.9,y:0.8,slot:3});
  assert!(c.validate().is_ok());
 }

 #[test]
 fn legacy_aim_fields_get_safe_defaults(){
  let a:Aim=toml::from_str("button=\"MOUSE_RIGHT\"\ncenter_x=0.5\ncenter_y=0.5\nsensitivity=2.0\nmode=\"relative\"\n").unwrap();
  assert!(!a.invert_x&&!a.invert_y);
  assert_eq!(a.scale_x,1.0);assert_eq!(a.scale_y,1.0);assert_eq!(a.edge_margin,0.12);
 }

 #[test]
 fn legacy_config_gets_new_defaults(){
  let c:Config=toml::from_str("[display]\nwidth=1920\nheight=1080\n[devices]\nkeyboard=\"/dev/input/event0\"\nmouse=\"/dev/input/event1\"\n").unwrap();
  assert!(c.joystick.is_none()&&c.aim.is_none());
  assert_eq!(c.performance.realtime_priority,10);
  assert_eq!(c.performance.fifo_write_retries,3);
  assert_eq!(c.performance.fifo_write_wait_ms,1);
  assert_eq!(c.performance.fifo_reconnect_ms,25);
  assert_eq!(c.touch.pressure,80);assert_eq!(c.touch.major,8);assert_eq!(c.touch.minor,8);
 }
}
