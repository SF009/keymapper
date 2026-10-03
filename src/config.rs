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
 #[serde(default)]pub performance:Performance
}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Display{pub width:i32,pub height:i32}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Devices{pub keyboard:Option<String>,pub mouse:Option<String>}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Joystick{pub up:String,pub down:String,pub left:String,pub right:String,pub center_x:f32,pub center_y:f32,pub radius:f32,#[serde(default)]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Aim{pub button:String,pub center_x:f32,pub center_y:f32,#[serde(default="ds")]pub sensitivity:f32,#[serde(default="s1")]pub slot:u8,#[serde(default)]pub invert_y:bool,#[serde(default="touch_mode")]pub mode:String}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Tap{pub key:String,pub x:f32,pub y:f32,#[serde(default="s2")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Hold{pub key:String,pub x:f32,pub y:f32,#[serde(default="s3")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct MouseTap{pub button:String,pub x:f32,pub y:f32,#[serde(default="s4")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct MouseHold{pub button:String,pub x:f32,pub y:f32,#[serde(default="s5")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Performance{#[serde(default="dt")]pub grab:bool,#[serde(default="dt")]pub realtime:bool,#[serde(default="dt")]pub mouse_lock:bool,#[serde(default="f8")]pub mouse_toggle_key:String}
fn ds()->f32{1.0} fn s1()->u8{1} fn s2()->u8{2} fn s3()->u8{3} fn s4()->u8{4} fn s5()->u8{5} fn dt()->bool{true} fn f8()->String{"F8".into()} fn touch_mode()->String{"touch".into()}
impl Default for Performance{fn default()->Self{Self{grab:true,realtime:true,mouse_lock:true,mouse_toggle_key:"F8".into()}}}

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
   if !(0.0..=1.0).contains(&a.center_x)||!(0.0..=1.0).contains(&a.center_y)||a.sensitivity<=0.0||a.sensitivity>100.0{return Err("invalid aim".into())}
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
   performance:Performance::default(),
  }
 }

 #[test]
 fn default_performance_is_shooter_safe(){
  let p=Performance::default();
  assert!(p.grab&&p.realtime&&p.mouse_lock);
  assert_eq!(p.mouse_toggle_key,"F8");
 }

 #[test]
 fn relative_aim_is_valid(){
  let mut c=base();
  c.aim=Some(Aim{
   button:"MOUSE_RIGHT".into(),center_x:0.5,center_y:0.5,sensitivity:2.,
   slot:1,invert_y:false,mode:"relative".into(),
  });
  assert!(c.validate().is_ok());
 }

 #[test]
 fn duplicate_physical_keyboard_input_is_rejected(){
  let mut c=base();
  c.joystick=Some(Joystick{
   up:"W".into(),down:"S".into(),left:"A".into(),right:"D".into(),
   center_x:0.15,center_y:0.76,radius:0.085,slot:0,
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
   slot:1,invert_y:false,mode:"touch".into(),
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
 fn unique_slots_are_accepted(){
  let mut c=base();
  c.taps.push(Tap{key:"SPACE".into(),x:0.8,y:0.8,slot:2});
  c.mouse_holds.push(MouseHold{button:"MOUSE_LEFT".into(),x:0.9,y:0.8,slot:3});
  assert!(c.validate().is_ok());
 }
}
