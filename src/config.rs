use serde::Deserialize;
use std::{env,error::Error,path::Path};

#[derive(Clone,Debug,Deserialize)]
pub struct Config{pub display:Display,pub devices:Devices,#[serde(default)]pub joystick:Option<Joystick>,#[serde(default)]pub aim:Option<Aim>,#[serde(default)]pub taps:Vec<Tap>,#[serde(default)]pub holds:Vec<Hold>,#[serde(default)]pub mouse_taps:Vec<MouseTap>,#[serde(default)]pub mouse_holds:Vec<MouseHold>,#[serde(default)]pub performance:Performance}
#[derive(Clone,Debug,Deserialize)]pub struct Display{pub width:i32,pub height:i32}
#[derive(Clone,Debug,Deserialize)]pub struct Devices{pub keyboard:Option<String>,pub mouse:Option<String>}
#[derive(Clone,Debug,Deserialize)]pub struct Joystick{pub up:String,pub down:String,pub left:String,pub right:String,pub center_x:f32,pub center_y:f32,pub radius:f32,#[serde(default)]pub slot:u8}
#[derive(Clone,Debug,Deserialize)]pub struct Aim{pub button:String,pub center_x:f32,pub center_y:f32,#[serde(default="ds")]pub sensitivity:f32,#[serde(default="s1")]pub slot:u8,#[serde(default)]pub invert_y:bool}
#[derive(Clone,Debug,Deserialize)]pub struct Tap{pub key:String,pub x:f32,pub y:f32,#[serde(default="s2")]pub slot:u8}
#[derive(Clone,Debug,Deserialize)]pub struct Hold{pub key:String,pub x:f32,pub y:f32,#[serde(default="s3")]pub slot:u8}
#[derive(Clone,Debug,Deserialize)]pub struct MouseTap{pub button:String,pub x:f32,pub y:f32,#[serde(default="s4")]pub slot:u8}
#[derive(Clone,Debug,Deserialize)]pub struct MouseHold{pub button:String,pub x:f32,pub y:f32,#[serde(default="s5")]pub slot:u8}
#[derive(Clone,Debug,Deserialize,Default)]pub struct Performance{#[serde(default="dt")]pub grab:bool,#[serde(default="dt")]pub realtime:bool}
fn ds()->f32{1.0} fn s1()->u8{1} fn s2()->u8{2} fn s3()->u8{3} fn s4()->u8{4} fn s5()->u8{5} fn dt()->bool{true}

impl Config{
 pub fn validate(&self)->Result<(),Box<dyn Error>>{
  if self.display.width<=0||self.display.height<=0{return Err("invalid display size".into())}
  if let Some(j)=&self.joystick{for k in [&j.up,&j.down,&j.left,&j.right]{crate::input::key_code(k)?}if !(0.0..=1.0).contains(&j.center_x)||!(0.0..=1.0).contains(&j.center_y)||j.radius<=0.0{return Err("invalid joystick".into())}}
  if let Some(a)=&self.aim{crate::input::button_code(&a.button)?}
  for x in &self.taps{crate::input::key_code(&x.key)?}for x in &self.holds{crate::input::key_code(&x.key)?}for x in &self.mouse_taps{crate::input::button_code(&x.button)?}for x in &self.mouse_holds{crate::input::button_code(&x.button)?}Ok(())
 }
 pub fn touch_fifo(&self)->String{
  if let Ok(p)=env::var("WAYDROID_TOUCH_FIFO"){return p}
  let c=["/dev/input/wl_touch_events","/var/lib/waydroid/rootfs/dev/input/wl_touch_events","/opt/waydroid/rootfs/dev/input/wl_touch_events"];
  c.iter().find(|p|Path::new(p).exists()).map(|p|p.to_string()).unwrap_or_else(||c[0].into())
 }
}
