use crate::touch::Mapper;
use evdev::{Device,EventSummary,KeyCode,RelativeAxisCode};
use std::{error::Error,sync::{Arc,Mutex},thread,time::Duration};

#[derive(Clone,Copy)]pub enum InputKind{Keyboard,Mouse}
pub fn spawn_input(path:String,kind:InputKind,mapper:Arc<Mutex<Mapper>>)->Result<(),Box<dyn Error>>{
 thread::Builder::new().name("wd-input".into()).spawn(move||{
  let mut d=match Device::open(&path){Ok(x)=>x,Err(e)=>{eprintln!("open {path}: {e}");return}};
  if mapper.lock().unwrap().config().performance.grab{let _=d.grab();}
  loop{match d.fetch_events(){Ok(es)=>for e in es{let mut m=mapper.lock().unwrap();match(kind,e.destructure()){
   (InputKind::Keyboard,EventSummary::Key(_,c,v))=>m.key(c.0,v),
   (InputKind::Mouse,EventSummary::RelativeAxis(_,c,v))=>{if c==RelativeAxisCode::REL_X{m.mouse(v,0)}else if c==RelativeAxisCode::REL_Y{m.mouse(0,v)}}
   (InputKind::Mouse,EventSummary::Key(_,c,v))=>m.button(c.0,v),_=>{}
  }},Err(_)=>thread::sleep(Duration::from_millis(1))}}
 })?;Ok(())
}
pub fn key_code(s:&str)->Result<u16,Box<dyn Error>>{
 let v=match s.to_ascii_uppercase().as_str(){
 "A"=>KeyCode::KEY_A.0,"B"=>KeyCode::KEY_B.0,"C"=>KeyCode::KEY_C.0,"D"=>KeyCode::KEY_D.0,"E"=>KeyCode::KEY_E.0,"F"=>KeyCode::KEY_F.0,"G"=>KeyCode::KEY_G.0,"H"=>KeyCode::KEY_H.0,"I"=>KeyCode::KEY_I.0,"J"=>KeyCode::KEY_J.0,"K"=>KeyCode::KEY_K.0,"L"=>KeyCode::KEY_L.0,"M"=>KeyCode::KEY_M.0,"N"=>KeyCode::KEY_N.0,"O"=>KeyCode::KEY_O.0,"P"=>KeyCode::KEY_P.0,"Q"=>KeyCode::KEY_Q.0,"R"=>KeyCode::KEY_R.0,"S"=>KeyCode::KEY_S.0,"T"=>KeyCode::KEY_T.0,"U"=>KeyCode::KEY_U.0,"V"=>KeyCode::KEY_V.0,"W"=>KeyCode::KEY_W.0,"X"=>KeyCode::KEY_X.0,"Y"=>KeyCode::KEY_Y.0,"Z"=>KeyCode::KEY_Z.0,
 "SPACE"=>KeyCode::KEY_SPACE.0,"ENTER"=>KeyCode::KEY_ENTER.0,"ESC"=>KeyCode::KEY_ESC.0,"SHIFT"=>KeyCode::KEY_LEFTSHIFT.0,"CTRL"=>KeyCode::KEY_LEFTCTRL.0,"ALT"=>KeyCode::KEY_LEFTALT.0,"TAB"=>KeyCode::KEY_TAB.0,
 _=>return Err(format!("unknown key {s}").into())};Ok(v)
}
pub fn button_code(s:&str)->Result<u16,Box<dyn Error>>{match s.to_ascii_uppercase().as_str(){"MOUSE_LEFT"|"LEFT"=>Ok(KeyCode::BTN_LEFT.0),"MOUSE_RIGHT"|"RIGHT"=>Ok(KeyCode::BTN_RIGHT.0),"MOUSE_MIDDLE"|"MIDDLE"=>Ok(KeyCode::BTN_MIDDLE.0),_=>Err(format!("unknown button {s}").into())}}
