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
 "A"=>KeyCode::KEY_A.0,"B"=>KeyCode::KEY_B.0,"C"=>KeyCode::KEY_C.0,"D"=>KeyCode::KEY_D.0,
 "E"=>KeyCode::KEY_E.0,"F"=>KeyCode::KEY_F.0,"G"=>KeyCode::KEY_G.0,"H"=>KeyCode::KEY_H.0,
 "I"=>KeyCode::KEY_I.0,"J"=>KeyCode::KEY_J.0,"K"=>KeyCode::KEY_K.0,"L"=>KeyCode::KEY_L.0,
 "M"=>KeyCode::KEY_M.0,"N"=>KeyCode::KEY_N.0,"O"=>KeyCode::KEY_O.0,"P"=>KeyCode::KEY_P.0,
 "Q"=>KeyCode::KEY_Q.0,"R"=>KeyCode::KEY_R.0,"S"=>KeyCode::KEY_S.0,"T"=>KeyCode::KEY_T.0,
 "U"=>KeyCode::KEY_U.0,"V"=>KeyCode::KEY_V.0,"W"=>KeyCode::KEY_W.0,"X"=>KeyCode::KEY_X.0,
 "Y"=>KeyCode::KEY_Y.0,"Z"=>KeyCode::KEY_Z.0,
 "0"=>KeyCode::KEY_0.0,"1"=>KeyCode::KEY_1.0,"2"=>KeyCode::KEY_2.0,"3"=>KeyCode::KEY_3.0,
 "4"=>KeyCode::KEY_4.0,"5"=>KeyCode::KEY_5.0,"6"=>KeyCode::KEY_6.0,"7"=>KeyCode::KEY_7.0,
 "8"=>KeyCode::KEY_8.0,"9"=>KeyCode::KEY_9.0,
 "SPACE"=>KeyCode::KEY_SPACE.0,"ENTER"=>KeyCode::KEY_ENTER.0,"ESC"|"ESCAPE"=>KeyCode::KEY_ESC.0,
 "SHIFT"|"LEFTSHIFT"=>KeyCode::KEY_LEFTSHIFT.0,"RIGHTSHIFT"=>KeyCode::KEY_RIGHTSHIFT.0,
 "CTRL"|"CONTROL"|"LEFTCTRL"=>KeyCode::KEY_LEFTCTRL.0,"RIGHTCTRL"=>KeyCode::KEY_RIGHTCTRL.0,
 "ALT"|"LEFTALT"=>KeyCode::KEY_LEFTALT.0,"RIGHTALT"=>KeyCode::KEY_RIGHTALT.0,"TAB"=>KeyCode::KEY_TAB.0,
 "BACKSPACE"=>KeyCode::KEY_BACKSPACE.0,"CAPSLOCK"=>KeyCode::KEY_CAPSLOCK.0,
 "F1"=>KeyCode::KEY_F1.0,"F2"=>KeyCode::KEY_F2.0,"F3"=>KeyCode::KEY_F3.0,"F4"=>KeyCode::KEY_F4.0,
 "F5"=>KeyCode::KEY_F5.0,"F6"=>KeyCode::KEY_F6.0,"F7"=>KeyCode::KEY_F7.0,"F8"=>KeyCode::KEY_F8.0,
 "F9"=>KeyCode::KEY_F9.0,"F10"=>KeyCode::KEY_F10.0,"F11"=>KeyCode::KEY_F11.0,"F12"=>KeyCode::KEY_F12.0,
 "HOME"=>KeyCode::KEY_HOME.0,"END"=>KeyCode::KEY_END.0,"UP"=>KeyCode::KEY_UP.0,
 "DOWN"=>KeyCode::KEY_DOWN.0,"LEFT"=>KeyCode::KEY_LEFT.0,"RIGHT"=>KeyCode::KEY_RIGHT.0,
 "PAGEUP"|"PAGE_UP"=>KeyCode::KEY_PAGEUP.0,"PAGEDOWN"|"PAGE_DOWN"=>KeyCode::KEY_PAGEDOWN.0,
 "INSERT"=>KeyCode::KEY_INSERT.0,"DELETE"|"DEL"=>KeyCode::KEY_DELETE.0,
 "NUMLOCK"=>KeyCode::KEY_NUMLOCK.0,"SCROLLLOCK"=>KeyCode::KEY_SCROLLLOCK.0,
 "KP0"=>KeyCode::KEY_KP0.0,"KP1"=>KeyCode::KEY_KP1.0,"KP2"=>KeyCode::KEY_KP2.0,"KP3"=>KeyCode::KEY_KP3.0,
 "KP4"=>KeyCode::KEY_KP4.0,"KP5"=>KeyCode::KEY_KP5.0,"KP6"=>KeyCode::KEY_KP6.0,"KP7"=>KeyCode::KEY_KP7.0,
 "KP8"=>KeyCode::KEY_KP8.0,"KP9"=>KeyCode::KEY_KP9.0,
 _=>return Err(format!("unknown key {s}").into())};
 Ok(v)
}

pub fn button_code(s:&str)->Result<u16,Box<dyn Error>>{
 match s.to_ascii_uppercase().as_str(){
  "MOUSE_LEFT"|"LEFT"=>Ok(KeyCode::BTN_LEFT.0),
  "MOUSE_RIGHT"|"RIGHT"=>Ok(KeyCode::BTN_RIGHT.0),
  "MOUSE_MIDDLE"|"MIDDLE"=>Ok(KeyCode::BTN_MIDDLE.0),
  "MOUSE_SIDE"|"SIDE"|"MOUSE_4"=>Ok(KeyCode::BTN_SIDE.0),
  "MOUSE_EXTRA"|"EXTRA"|"MOUSE_5"=>Ok(KeyCode::BTN_EXTRA.0),
  "MOUSE_FORWARD"|"FORWARD"=>Ok(KeyCode::BTN_FORWARD.0),
  "MOUSE_BACK"|"BACK"=>Ok(KeyCode::BTN_BACK.0),
  "MOUSE_TASK"|"TASK"=>Ok(KeyCode::BTN_TASK.0),
  _=>Err(format!("unknown button {s}").into())
 }
}
