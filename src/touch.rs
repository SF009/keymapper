use crate::{
    config::{Config,Joystick},
    input::{button_code,key_code,KeyAction,MouseAction},
};
use std::{
    error::Error,
    io::{self},
    os::fd::{AsRawFd,FromRawFd},
    sync::Arc,
};

const SYN:u16=0;
const KEY:u16=1;
const REL:u16=2;
const ABS:u16=3;
const BTN_TOUCH:u16=330;
const REL_X:u16=0;
const REL_Y:u16=1;
const SLOT:u16=47;
const MAJOR:u16=48;
const MINOR:u16=49;
const X:u16=53;
const Y:u16=54;
const ID:u16=57;
const PRESS:u16=58;
const MAX_INPUT_CODE:usize=1024;

#[repr(C)]
#[derive(Clone,Copy)]
struct E{
    i64a:i64,
    i64b:i64,
    t:u16,
    c:u16,
    v:i32,
}

struct Pipe{
    p:String,
    f:Option<std::fs::File>,
}
impl Pipe{
    fn new(p:String)->Self{Self{p,f:None}}
    fn connect(&mut self)->io::Result<()>{
        if self.f.is_some(){return Ok(())}
        let c=std::ffi::CString::new(self.p.as_str()).map_err(|_|io::Error::new(io::ErrorKind::InvalidInput,"invalid FIFO path"))?;
        let fd=unsafe{libc::open(c.as_ptr(),libc::O_WRONLY|libc::O_NONBLOCK|libc::O_CLOEXEC)};
        if fd<0{return Err(io::Error::last_os_error())}
        self.f=Some(unsafe{std::fs::File::from_raw_fd(fd)});
        Ok(())
    }
    fn send(&mut self,es:&[(u16,u16,i32)]){
        if es.is_empty(){return}
        if self.f.is_none()&&self.connect().is_err(){return}
        let Some(f)=self.f.as_mut()else{return};

        // Normal mapper batches are far below Linux PIPE_BUF. A single write
        // keeps one multitouch transaction atomic and avoids write_all() on a
        // non-blocking FIFO, which can turn EAGAIN/partial writes into dropped
        // touch state.
        let size=std::mem::size_of::<E>();
        let need=es.len().saturating_mul(size);
        let mut buf=[0u8;1024];

        if need>buf.len(){
            // This path is only for unusually large batches.
            let mut v=Vec::with_capacity(need);
            for &(t,c,value)in es{
                let e=E{i64a:0,i64b:0,t,c,v:value};
                let b=unsafe{std::slice::from_raw_parts((&e as*const E)as*const u8,size)};
                v.extend_from_slice(b);
            }
            if pipe_write_bounded(f.as_raw_fd(),&v).is_err(){self.f=None;}
            return;
        }

        let mut used=0usize;
        for &(t,c,value)in es{
            let e=E{i64a:0,i64b:0,t,c,v:value};
            let b=unsafe{std::slice::from_raw_parts((&e as*const E)as*const u8,size)};
            buf[used..used+size].copy_from_slice(b);
            used+=size;
        }

        if pipe_write_bounded(f.as_raw_fd(),&buf[..used]).is_err(){self.f=None;}
    }
}

fn pipe_write_bounded(fd:i32,data:&[u8])->io::Result<()>{
    // Keep the real-time path bounded: if Waydroid's FIFO reader is temporarily
    // busy, wait only a few short polls instead of blocking the input thread.
    for _ in 0..3{
        let n=unsafe{libc::write(fd,data.as_ptr().cast::<libc::c_void>(),data.len())};
        if n==data.len() as isize{return Ok(())}
        if n<0{
            let err=io::Error::last_os_error();
            match err.raw_os_error(){
                Some(libc::EINTR)=>continue,
                Some(libc::EAGAIN)|Some(libc::EWOULDBLOCK)=>{
                    let mut p=libc::pollfd{fd,events:libc::POLLOUT,revents:0};
                    let rc=unsafe{libc::poll(&mut p,1,1)};
                    if rc>0{continue}
                    if rc==0{break}
                    if io::Error::last_os_error().kind()==io::ErrorKind::Interrupted{continue}
                    return Err(io::Error::last_os_error());
                }
                _=>return Err(err),
            }
        }else{
            // A partial write should not occur for the normal <= PIPE_BUF path.
            // Treat it as a failed transaction rather than sending a truncated
            // multitouch frame.
            return Err(io::Error::new(io::ErrorKind::WriteZero,"partial FIFO write"));
        }
    }
    Err(io::Error::new(io::ErrorKind::WouldBlock,"Waydroid input FIFO busy"))
}
}

#[derive(Clone,Copy)]
struct C{down:bool}

#[derive(Clone,Copy)]
struct JoyRuntime{
    up:u16,
    down:u16,
    left:u16,
    right:u16,
    center_x:f32,
    center_y:f32,
    radius:f32,
    slot:u8,
}

#[derive(Clone,Copy)]
struct AimRuntime{
    button:u16,
    center_x:f32,
    center_y:f32,
    sensitivity:f32,
    slot:u8,
    invert_y:bool,
    relative:bool,
}

pub struct Mapper{
    cfg:Arc<Config>,
    touch:Pipe,
    pointer:Pipe,
    slots:[C;16],
    next:i32,
    mx:f32,
    my:f32,
    aim:bool,
    mouse_locked:bool,
    rel_acc_x:f32,
    rel_acc_y:f32,
    keys:[bool;MAX_INPUT_CODE],
    key_actions:Box<[Option<KeyAction>]>,
    mouse_actions:Box<[Option<MouseAction>]>,
    joystick:Option<JoyRuntime>,
    aim_cfg:Option<AimRuntime>,
    mouse_hold_slots:[bool;16],
}

impl Mapper{
    pub fn new(cfg:Config)->Result<Self,Box<dyn Error>>{
        cfg.validate()?;
        let mut key_actions=vec![None;MAX_INPUT_CODE];
        let mut mouse_actions=vec![None;MAX_INPUT_CODE];

        let joystick=cfg.joystick.as_ref().map(|j|{
            let up=key_code(&j.up).unwrap();
            let down=key_code(&j.down).unwrap();
            let left=key_code(&j.left).unwrap();
            let right=key_code(&j.right).unwrap();
            key_actions[up as usize]=Some(KeyAction::Joystick);
            key_actions[down as usize]=Some(KeyAction::Joystick);
            key_actions[left as usize]=Some(KeyAction::Joystick);
            key_actions[right as usize]=Some(KeyAction::Joystick);
            JoyRuntime{up,down,left,right,center_x:j.center_x,center_y:j.center_y,radius:j.radius,slot:j.slot}
        });

        let aim_cfg=cfg.aim.as_ref().map(|a|{
            let button=button_code(&a.button).unwrap();
            mouse_actions[button as usize]=Some(MouseAction::Aim);
            AimRuntime{
                button,center_x:a.center_x,center_y:a.center_y,
                sensitivity:a.sensitivity,slot:a.slot,invert_y:a.invert_y,
                relative:a.mode.eq_ignore_ascii_case("relative"),
            }
        });

        for x in &cfg.taps{
            key_actions[key_code(&x.key).unwrap() as usize]=Some(KeyAction::Tap{slot:x.slot,x:x.x,y:x.y});
        }
        for x in &cfg.holds{
            key_actions[key_code(&x.key).unwrap() as usize]=Some(KeyAction::Hold{slot:x.slot,x:x.x,y:x.y});
        }
        for x in &cfg.mouse_taps{
            mouse_actions[button_code(&x.button).unwrap() as usize]=Some(MouseAction::Tap{slot:x.slot,x:x.x,y:x.y});
        }
        for x in &cfg.mouse_holds{
            mouse_actions[button_code(&x.button).unwrap() as usize]=Some(MouseAction::Hold{slot:x.slot,x:x.x,y:x.y});
        }

        let mut mouse_hold_slots=[false;16];
        for x in &cfg.mouse_holds{mouse_hold_slots[x.slot as usize]=true;}

        let mouse_locked=cfg.performance.mouse_lock;
        Ok(Self{
            touch:Pipe::new(cfg.touch_fifo()),
            pointer:Pipe::new(cfg.pointer_fifo()),
            cfg:Arc::new(cfg),
            slots:[C{down:false};16],
            next:1,
            mx:0.5,my:0.5,aim:false,mouse_locked,
            rel_acc_x:0.0,rel_acc_y:0.0,
            keys:[false;MAX_INPUT_CODE],
            key_actions:key_actions.into_boxed_slice(),
            mouse_actions:mouse_actions.into_boxed_slice(),
            joystick,aim_cfg,mouse_hold_slots,
        })
    }

    pub fn config(&self)->&Config{&self.cfg}

    pub fn set_mouse_lock(&mut self,locked:bool){
        if self.mouse_locked==locked{return}
        self.mouse_locked=locked;
        if !locked{
            self.release_mouse_inputs();
            self.rel_acc_x=0.0;
            self.rel_acc_y=0.0;
        }
    }

    fn out_touch(&mut self,e:&[(u16,u16,i32)]){self.touch.send(e);}
    fn out_pointer(&mut self,e:&[(u16,u16,i32)]){self.pointer.send(e);}

    fn xy(&self,x:f32,y:f32)->(i32,i32){
        (
            (x.clamp(0.,1.)*(self.cfg.display.width-1)as f32).round()as i32,
            (y.clamp(0.,1.)*(self.cfg.display.height-1)as f32).round()as i32,
        )
    }

    fn down(&mut self,s:u8,x:f32,y:f32){
        let i=s as usize;
        if i>=16||self.slots[i].down{return}
        let first=!self.any_down();
        let(x,y)=self.xy(x,y);
        let id=self.next;
        self.next=self.next.wrapping_add(1);
        self.slots[i]=C{down:true};
        let mut e=vec![
            (ABS,SLOT,s as i32),
            (ABS,ID,id),
            (ABS,X,x),
            (ABS,Y,y),
            (ABS,MAJOR,8),
            (ABS,MINOR,8),
            (ABS,PRESS,80),
        ];
        if first{e.push((KEY,BTN_TOUCH,1))}
        e.push((SYN,0,0));
        self.out_touch(&e);
    }

    fn mv(&mut self,s:u8,x:f32,y:f32){
        let i=s as usize;
        if i>=16||!self.slots[i].down{return}
        let(x,y)=self.xy(x,y);
        self.out_touch(&[(ABS,SLOT,s as i32),(ABS,X,x),(ABS,Y,y),(ABS,PRESS,80),(SYN,0,0)]);
    }

    fn any_down(&self)->bool{self.slots.iter().any(|c|c.down)}

    fn up(&mut self,s:u8){
        let i=s as usize;
        if i>=16||!self.slots[i].down{return}
        self.slots[i]=C{down:false};
        let last=!self.any_down();
        let mut e=vec![(ABS,SLOT,s as i32),(ABS,ID,-1),(ABS,PRESS,0)];
        if last{e.push((KEY,BTN_TOUCH,0))}
        e.push((SYN,0,0));
        self.out_touch(&e);
    }

    pub fn key(&mut self,c:u16,v:i32){
        let i=c as usize;
        if i>=MAX_INPUT_CODE{return}
        self.keys[i]=v!=0;
        let Some(action)=self.key_actions[i]else{return};
        match action{
            KeyAction::Joystick=>self.joy(),
            KeyAction::Tap{slot,x,y}=>if v==1{self.down(slot,x,y);self.up(slot)},
            KeyAction::Hold{slot,x,y}=>{
                if v==1{self.down(slot,x,y)}else if v==0{self.up(slot)}
            }
        }
    }

    fn pressed(&self,c:u16)->bool{self.keys.get(c as usize).copied().unwrap_or(false)}

    fn joy(&mut self){
        let Some(j)=self.joystick else{return};
        let mut dx=0.0f32;
        let mut dy=0.0f32;
        if self.pressed(j.left){dx-=1.}
        if self.pressed(j.right){dx+=1.}
        if self.pressed(j.up){dy-=1.}
        if self.pressed(j.down){dy+=1.}
        let l=(dx*dx+dy*dy).sqrt();
        if l>1.{dx/=l;dy/=l}
        let x=j.center_x+dx*j.radius;
        let y=j.center_y+dy*j.radius;
        if l==0.{self.up(j.slot)}
        else if self.slots[j.slot as usize].down{self.mv(j.slot,x,y)}
        else{self.down(j.slot,x,y)}
    }

    pub fn button(&mut self,c:u16,v:i32){
        let Some(action)=self.mouse_actions.get(c as usize).copied().flatten()else{return};
        match action{
            MouseAction::Aim=>{
                if v==1&&!self.aim{
                    self.aim=true;
                    if let Some(a)=self.aim_cfg{
                        self.mx=a.center_x;self.my=a.center_y;
                        if !a.relative{self.down(a.slot,self.mx,self.my);}
                    }
                }else if v==0&&self.aim{
                    self.aim=false;
                    if let Some(a)=self.aim_cfg{
                        if !a.relative{self.up(a.slot);}
                    }
                }
            }
            MouseAction::Tap{slot,x,y}=>if v==1{self.down(slot,x,y);self.up(slot)},
            MouseAction::Hold{slot,x,y}=>{
                if v==1{self.down(slot,x,y)}
                else if v==0{self.up(slot)}
            }
        }
    }

    pub fn mouse(&mut self,dx:i32,dy:i32){
        if !self.mouse_locked||!self.aim{return}
        let Some(a)=self.aim_cfg else{return};

        if a.relative{
            if dx==0&&dy==0{return}

            // Preserve sub-pixel mouse movement for low sensitivities instead
            // of rounding every evdev packet independently to zero.
            self.rel_acc_x+=(dx as f32)*a.sensitivity;
            self.rel_acc_y+=(dy as f32)*a.sensitivity*(if a.invert_y{-1.}else{1.});

            let sx=self.rel_acc_x.trunc() as i32;
            let sy=self.rel_acc_y.trunc() as i32;
            self.rel_acc_x-=sx as f32;
            self.rel_acc_y-=sy as f32;

            if sx==0&&sy==0{return}

            let mut e=[(REL,REL_X,0),(REL,REL_Y,0),(SYN,0,0)];
            let mut n=0usize;
            if sx!=0{e[n]=(REL,REL_X,sx);n+=1;}
            if sy!=0{e[n]=(REL,REL_Y,sy);n+=1;}
            e[n]=(SYN,0,0);
            self.out_pointer(&e[..=n]);
            return;
        }

        self.mx+=(dx as f32*a.sensitivity)/self.cfg.display.width as f32;
        let sy=if a.invert_y{-1.}else{1.};
        self.my+=(dy as f32*a.sensitivity*sy)/self.cfg.display.height as f32;

        // Keep the virtual touch near the center. This remains bounded by the
        // Android touch protocol; relative mode above is the true unbounded path.
        if self.mx<0.12||self.mx>0.88{self.mx=a.center_x;}
        if self.my<0.12||self.my>0.88{self.my=a.center_y;}
        self.mv(a.slot,self.mx,self.my);
    }

    fn release_mouse_inputs(&mut self){
        self.aim=false;
        if let Some(a)=self.aim_cfg{if !a.relative{self.up(a.slot);}}
        for slot in 0..16{
            if self.mouse_hold_slots[slot]{self.up(slot as u8);}
        }
        self.rel_acc_x=0.0;
        self.rel_acc_y=0.0;
    }

    /// Clear only keyboard-owned state after an evdev keyboard disconnect.
    pub fn reset_keyboard_state(&mut self){
        self.keys.fill(false);
        if let Some(j)=self.joystick{self.up(j.slot);}
        let slots:Vec<u8>=self.cfg.holds.iter().map(|x|x.slot).collect();
        for slot in slots{self.up(slot);}
    }

    /// Clear mouse-owned state after an evdev mouse disconnect.
    pub fn reset_mouse_state(&mut self){
        self.release_mouse_inputs();
        self.rel_acc_x=0.0;
        self.rel_acc_y=0.0;
    }
}
