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
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Aim{pub button:String,pub center_x:f32,pub center_y:f32,#[serde(default="ds")]pub sensitivity:f32,#[serde(default="s1")]pub slot:u8,#[serde(default)]pub invert_y:bool}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Tap{pub key:String,pub x:f32,pub y:f32,#[serde(default="s2")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Hold{pub key:String,pub x:f32,pub y:f32,#[serde(default="s3")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct MouseTap{pub button:String,pub x:f32,pub y:f32,#[serde(default="s4")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct MouseHold{pub button:String,pub x:f32,pub y:f32,#[serde(default="s5")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Serialize)]pub struct Performance{#[serde(default="dt")]pub grab:bool,#[serde(default="dt")]pub realtime:bool}
fn ds()->f32{1.0} fn s1()->u8{1} fn s2()->u8{2} fn s3()->u8{3} fn s4()->u8{4} fn s5()->u8{5} fn dt()->bool{true}

impl Config{
 pub fn validate(&self)->Result<(),Box<dyn Error>>{
  if self.display.width<=0||self.display.height<=0{return Err("invalid display size".into())}
  if self.display.width>16384||self.display.height>16384{return Err("display size is too large".into())}
  if let Some(j)=&self.joystick{
   for k in [&j.up,&j.down,&j.left,&j.right]{crate::input::key_code(k)?}
   if !(0.0..=1.0).contains(&j.center_x)||!(0.0..=1.0).contains(&j.center_y)||j.radius<=0.0||j.radius>1.0{return Err("invalid joystick".into())}
   if j.slot>=16{return Err("joystick slot must be 0..15".into())}
  }
  if let Some(a)=&self.aim{
   crate::input::button_code(&a.button)?;
   if !(0.0..=1.0).contains(&a.center_x)||!(0.0..=1.0).contains(&a.center_y)||a.sensitivity<=0.0||a.slot>=16{return Err("invalid aim".into())}
  }
  for x in &self.taps{crate::input::key_code(&x.key)?;if x.slot>=16||!(0.0..=1.0).contains(&x.x)||!(0.0..=1.0).contains(&x.y){return Err("invalid keyboard tap".into())}}
  for x in &self.holds{crate::input::key_code(&x.key)?;if x.slot>=16||!(0.0..=1.0).contains(&x.x)||!(0.0..=1.0).contains(&x.y){return Err("invalid keyboard hold".into())}}
  for x in &self.mouse_taps{crate::input::button_code(&x.button)?;if x.slot>=16||!(0.0..=1.0).contains(&x.x)||!(0.0..=1.0).contains(&x.y){return Err("invalid mouse tap".into())}}
  for x in &self.mouse_holds{crate::input::button_code(&x.button)?;if x.slot>=16||!(0.0..=1.0).contains(&x.x)||!(0.0..=1.0).contains(&x.y){return Err("invalid mouse hold".into())}}
  Ok(())
 }
 pub fn touch_fifo(&self)->String{
  if let Ok(p)=env::var("WAYDROID_TOUCH_FIFO"){return p}
  let c=["/dev/input/wl_touch_events","/var/lib/waydroid/rootfs/dev/input/wl_touch_events","/opt/waydroid/rootfs/dev/input/wl_touch_events"];
  c.iter().find(|p|Path::new(p).exists()).map(|p|p.to_string()).unwrap_or_else(||c[0].into())
 }
}
