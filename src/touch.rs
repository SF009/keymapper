use crate::{config::{Config,Joystick},input::{button_code,key_code}};
use std::{error::Error,io::{self,Write},os::fd::FromRawFd,sync::Arc};

const SYN:u16=0;const KEY:u16=1;const ABS:u16=3;const BTN_TOUCH:u16=330;const SLOT:u16=47;const MAJOR:u16=48;const MINOR:u16=49;const X:u16=53;const Y:u16=54;const ID:u16=57;const PRESS:u16=58;
#[repr(C)]#[derive(Clone,Copy)]struct E{i64a:i64,i64b:i64,t:u16,c:u16,v:i32}
struct Pipe{p:String,f:Option<std::fs::File>}
impl Pipe{
 fn new(p:String)->Self{Self{p,f:None}}
 fn connect(&mut self)->io::Result<()>{
  if self.f.is_some(){return Ok(())}let c=std::ffi::CString::new(self.p.as_str()).unwrap();
  let fd=unsafe{libc::open(c.as_ptr(),libc::O_WRONLY|libc::O_NONBLOCK|libc::O_CLOEXEC)};if fd<0{return Err(io::Error::last_os_error())}
  self.f=Some(unsafe{std::fs::File::from_raw_fd(fd)});Ok(())
 }
 fn send(&mut self,es:&[(u16,u16,i32)])->io::Result<()>{
  let Some(f)=self.f.as_mut()else{return Ok(())};
  for&(t,c,v)in es{let e=E{i64a:0,i64b:0,t,c,v};let b=unsafe{std::slice::from_raw_parts((&e as*const E)as*const u8,std::mem::size_of::<E>())};if let Err(x)=f.write_all(b){self.f=None;return Err(x)}}
  Ok(())
 }
}
#[derive(Clone,Copy)]struct C{down:bool,tracking:i32}
pub struct Mapper{cfg:Arc<Config>,pipe:Pipe,slots:[C;16],next:i32,mx:f32,my:f32,aim:bool,keys:[bool;512]}
impl Mapper{
 pub fn new(cfg:Config)->Result<Self,Box<dyn Error>>{let mut p=Pipe::new(cfg.touch_fifo());let _=p.connect();Ok(Self{cfg:Arc::new(cfg),pipe:p,slots:[C{down:false,tracking:-1};16],next:1,mx:.5,my:.5,aim:false,keys:[false;512]})}
 pub fn config(&self)->&Config{&self.cfg}
 fn out(&mut self,e:&[(u16,u16,i32)]){if self.pipe.f.is_none(){let _=self.pipe.connect()}let _=self.pipe.send(e)}
 fn xy(&self,x:f32,y:f32)->(i32,i32){((x.clamp(0.,1.)*(self.cfg.display.width-1)as f32).round()as i32,(y.clamp(0.,1.)*(self.cfg.display.height-1)as f32).round()as i32)}
 fn down(&mut self,s:u8,x:f32,y:f32){let i=s as usize;if i>=16{return}let(x,y)=self.xy(x,y);let id=self.next;self.next+=1;self.slots[i]=C{down:true,tracking:id};self.out(&[(ABS,SLOT,s as i32),(ABS,ID,id),(ABS,X,x),(ABS,Y,y),(ABS,MAJOR,8),(ABS,MINOR,8),(ABS,PRESS,80),(KEY,BTN_TOUCH,1),(SYN,0,0)])}
 fn mv(&mut self,s:u8,x:f32,y:f32){let i=s as usize;if i>=16||!self.slots[i].down{return}let(x,y)=self.xy(x,y);self.out(&[(ABS,SLOT,s as i32),(ABS,X,x),(ABS,Y,y),(ABS,PRESS,80),(SYN,0,0)])}
 fn up(&mut self,s:u8){let i=s as usize;if i>=16||!self.slots[i].down{return}self.out(&[(ABS,SLOT,s as i32),(ABS,ID,-1),(ABS,PRESS,0),(KEY,BTN_TOUCH,0),(SYN,0,0)]);self.slots[i].down=false}
 pub fn key(&mut self,c:u16,v:i32){if let Some(x)=self.keys.get_mut(c as usize){*x=v!=0}
  if let Some(j)=self.cfg.joystick.clone(){if [key_code(&j.up),key_code(&j.down),key_code(&j.left),key_code(&j.right)].iter().flatten().any(|&x|x==c){self.joy(j);return}}
  for t in self.cfg.taps.clone(){if key_code(&t.key).ok()==Some(c)&&v==1{self.down(t.slot,t.x,t.y);self.up(t.slot);return}}
  for h in self.cfg.holds.clone(){if key_code(&h.key).ok()==Some(c){if v==1{self.down(h.slot,h.x,h.y)}else if v==0{self.up(h.slot)}return}}
 }
 fn pressed(&self,s:&str)->bool{key_code(s).ok().and_then(|k|self.keys.get(k as usize).copied()).unwrap_or(false)}
 fn joy(&mut self,j:Joystick){let mut dx=0.;let mut dy=0.;if self.pressed(&j.left){dx-=1.}if self.pressed(&j.right){dx+=1.}if self.pressed(&j.up){dy-=1.}if self.pressed(&j.down){dy+=1.}let l=(dx*dx+dy*dy).sqrt();if l>1.{dx/=l;dy/=l}let x=j.center_x+dx*j.radius;let y=j.center_y+dy*j.radius;if l==0.{self.up(j.slot)}else if self.slots[j.slot as usize].down{self.mv(j.slot,x,y)}else{self.down(j.slot,x,y)}}
 pub fn button(&mut self,c:u16,v:i32){if let Some(a)=self.cfg.aim.clone(){if button_code(&a.button).ok()==Some(c){self.aim=v!=0;if self.aim{self.mx=a.center_x;self.my=a.center_y;self.down(a.slot,self.mx,self.my)}else{self.up(a.slot)}}}}
 pub fn mouse(&mut self,dx:i32,dy:i32){let Some(a)=self.cfg.aim.clone()else{return};if !self.aim{return}self.mx+=dx as f32*a.sensitivity/self.cfg.display.width as f32;let sy=if a.invert_y{-1.}else{1.};self.my+=dy as f32*a.sensitivity*sy/self.cfg.display.height as f32;if self.mx<.08||self.mx>.92{self.mx=.5}if self.my<.08||self.my>.92{self.my=.5}self.mv(a.slot,self.mx,self.my)}
}
