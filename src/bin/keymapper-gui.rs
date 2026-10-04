#[path="../config.rs"] mod config;
#[path="../input.rs"] mod input;
#[path="../touch.rs"] mod touch;
#[path="../control.rs"] mod control;

use config::{Aim,Config,Display,Devices,Hold,Joystick,MouseHold,MouseTap,Performance,Tap};
use gtk4::prelude::*;
use gtk4::{
    cairo, glib, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton,
    ComboBoxText, Dialog, DrawingArea, Entry, EventControllerKey, Frame, GestureClick, GestureDrag,
    Grid, Label, ListBox, ListBoxRow, Orientation, Paned, PolicyType, ScrolledWindow, Separator,
    SpinButton,
};
use std::{
    cell::{Cell,RefCell},
    env,
    error::Error,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path,PathBuf},
    process::Command,
    rc::Rc,
    time::Duration,
};

const APP_ID:&str="io.sf009.WaydroidKeymapper";

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum BindingRef { Tap(usize), Hold(usize), MouseTap(usize), MouseHold(usize), Aim, Joystick }

struct State {
    cfg:Config,
    profile_path:PathBuf,
    selected:Option<BindingRef>,
    dirty:bool,
}

#[derive(Clone)]
struct Ui {
    state:Rc<RefCell<State>>,
    profile_list:ListBox,
    bindings_box:GtkBox,
    canvas:DrawingArea,
    status:Label,
    profile_name:Entry,
    width:SpinButton,
    height:SpinButton,
    keyboard:ComboBoxText,
    mouse:ComboBoxText,
    aim_enabled:CheckButton,
    aim_button:Entry,
    aim_mode:ComboBoxText,
    aim_x:SpinButton,
    aim_y:SpinButton,
    aim_sensitivity:SpinButton,
    aim_slot:SpinButton,
    aim_invert_y:CheckButton,
    joy_enabled:CheckButton,
    joy_up:Entry,
    joy_down:Entry,
    joy_left:Entry,
    joy_right:Entry,
    joy_x:SpinButton,
    joy_y:SpinButton,
    joy_radius:SpinButton,
    joy_slot:SpinButton,
    grab:CheckButton,
    realtime:CheckButton,
    mouse_lock:CheckButton,
    mouse_toggle:Entry,
    runtime_status:Label,
    lock_status:Label,
    input_access:Label,
}

fn home_dir()->PathBuf{
    env::var_os("HOME").map(PathBuf::from).unwrap_or_else(||PathBuf::from("."))
}

fn profiles_dir()->PathBuf{
    home_dir().join(".config/waydroid-keymapper/profiles")
}

fn active_config_path()->PathBuf{
    home_dir().join(".config/waydroid-keymapper/config.toml")
}


fn shooter_profile(base:&Config)->Config{
    let mut cfg=base.clone();
    cfg.joystick=Some(Joystick{
        up:"W".into(),down:"S".into(),left:"A".into(),right:"D".into(),
        center_x:0.15,center_y:0.76,radius:0.085,slot:0,
    });
    cfg.aim=Some(Aim{
        button:"MOUSE_RIGHT".into(),center_x:0.50,center_y:0.50,
        sensitivity:2.0,slot:1,invert_y:false,mode:"relative".into(),
    });
    cfg
}

fn preset_free_fire()->Config{
    let mut cfg=shooter_profile(&default_config());
    cfg.taps=vec![
        Tap{key:"SPACE".into(),x:0.84,y:0.86,slot:2}, // jump
        Tap{key:"R".into(),x:0.93,y:0.18,slot:3},     // reload
        Tap{key:"1".into(),x:0.72,y:0.18,slot:4},     // primary weapon
        Tap{key:"2".into(),x:0.78,y:0.18,slot:5},     // secondary weapon
        Tap{key:"G".into(),x:0.58,y:0.18,slot:6},     // grenade
    ];
    cfg.holds=vec![
        Hold{key:"F".into(),x:0.76,y:0.83,slot:7},   // interact
        Hold{key:"SHIFT".into(),x:0.28,y:0.76,slot:8},// sprint
        Hold{key:"C".into(),x:0.34,y:0.88,slot:9},   // crouch
    ];
    cfg.mouse_taps=Vec::new();
    cfg.mouse_holds=vec![
        MouseHold{button:"MOUSE_LEFT".into(),x:0.88,y:0.78,slot:10}, // fire
    ];
    cfg
}

fn preset_fps()->Config{
    let mut cfg=shooter_profile(&default_config());
    cfg.aim.as_mut().unwrap().sensitivity=2.2;
    cfg.aim.as_mut().unwrap().mode="relative".into();
    cfg.taps=vec![
        Tap{key:"SPACE".into(),x:0.84,y:0.86,slot:2},
        Tap{key:"R".into(),x:0.93,y:0.18,slot:3},
        Tap{key:"1".into(),x:0.72,y:0.18,slot:4},
        Tap{key:"2".into(),x:0.78,y:0.18,slot:5},
        Tap{key:"G".into(),x:0.58,y:0.18,slot:6},
        Tap{key:"Q".into(),x:0.47,y:0.18,slot:7},
    ];
    cfg.holds=vec![
        Hold{key:"SHIFT".into(),x:0.28,y:0.76,slot:8},
        Hold{key:"CTRL".into(),x:0.34,y:0.88,slot:9},
        Hold{key:"F".into(),x:0.76,y:0.83,slot:10},
    ];
    cfg.mouse_taps=Vec::new();
    cfg.mouse_holds=vec![
        MouseHold{button:"MOUSE_LEFT".into(),x:0.88,y:0.78,slot:11},
    ];
    cfg
}

fn preset_minimal()->Config{
    let mut cfg=shooter_profile(&default_config());
    cfg.taps=vec![
        Tap{key:"SPACE".into(),x:0.84,y:0.86,slot:2},
        Tap{key:"R".into(),x:0.93,y:0.18,slot:3},
    ];
    cfg.holds=vec![
        Hold{key:"SHIFT".into(),x:0.28,y:0.76,slot:4},
        Hold{key:"F".into(),x:0.76,y:0.83,slot:5},
    ];
    cfg.mouse_taps=Vec::new();
    cfg.mouse_holds=vec![
        MouseHold{button:"MOUSE_LEFT".into(),x:0.88,y:0.78,slot:6},
    ];
    cfg
}

#[derive(Clone,Copy)]
enum ShooterPreset{FreeFire,Fps,Minimal}

fn apply_preset(ui:&Ui,preset:ShooterPreset){
    let preset_cfg=match preset{
        ShooterPreset::FreeFire=>preset_free_fire(),
        ShooterPreset::Fps=>preset_fps(),
        ShooterPreset::Minimal=>preset_minimal(),
    };
    let mut st=ui.state.borrow_mut();
    let mut cfg=preset_cfg;
    cfg.display=st.cfg.display.clone();
    cfg.devices=st.cfg.devices.clone();
    cfg.performance=st.cfg.performance.clone();
    st.cfg=cfg;
    st.selected=None;
    st.dirty=true;
    drop(st);
    sync_form(ui);
    rebuild_bindings(ui);
    ui.canvas.queue_draw();
    set_status(ui,match preset{
        ShooterPreset::FreeFire=>"Free Fire preset loaded — drag controls and Save",
        ShooterPreset::Fps=>"FPS/BR Shooter preset loaded — drag controls and Save",
        ShooterPreset::Minimal=>"Minimal Shooter preset loaded — drag controls and Save",
    });
}

fn default_config()->Config{
    Config{
        display:Display{width:1920,height:1080},
        devices:Devices{keyboard:None,mouse:None},
        joystick:Some(Joystick{
            up:"W".into(),down:"S".into(),left:"A".into(),right:"D".into(),
            center_x:0.15,center_y:0.76,radius:0.085,slot:0,
        }),
        aim:Some(Aim{
            button:"MOUSE_RIGHT".into(),center_x:0.50,center_y:0.50,sensitivity:2.,
            slot:1,invert_y:false,mode:"touch".into(),
        }),
        taps:vec![
            Tap{key:"SPACE".into(),x:0.86,y:0.86,slot:2},
            Tap{key:"R".into(),x:0.93,y:0.18,slot:3},
        ],
        holds:vec![Hold{key:"F".into(),x:0.78,y:0.84,slot:4}],
        mouse_taps:Vec::new(),
        mouse_holds:vec![MouseHold{button:"MOUSE_LEFT".into(),x:0.88,y:0.78,slot:5}],
        performance:Performance{grab:true,realtime:true,mouse_lock:true,mouse_toggle_key:"F8".into()},
    }
}

fn ensure_profiles()->Result<PathBuf,Box<dyn Error>>{
    let dir=profiles_dir();
    fs::create_dir_all(&dir)?;
    let default_path=dir.join("default.toml");
    if !default_path.exists(){
        let cfg=active_config_path();
        if cfg.exists(){
            match load_profile(&cfg){
                Ok(existing)=>fs::write(&default_path,toml::to_string_pretty(&existing)?)?,
                Err(_)=>fs::write(&default_path,toml::to_string_pretty(&default_config())?)?,
            }
        }else{
            fs::write(&default_path,toml::to_string_pretty(&default_config())?)?;
        }
    }
    Ok(dir)
}

fn profile_files()->Vec<PathBuf>{
    let dir=profiles_dir();
    let Ok(read)=fs::read_dir(dir) else {return Vec::new()};
    let mut files:Vec<PathBuf>=read.flatten()
        .map(|e|e.path())
        .filter(|p|p.extension().and_then(|x|x.to_str())==Some("toml"))
        .collect();
    files.sort_by_key(|p|p.file_name().map(|x|x.to_os_string()));
    files
}

fn load_profile(path:&Path)->Result<Config,Box<dyn Error>>{
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

fn save_profile(path:&Path,cfg:&Config)->Result<(),Box<dyn Error>>{
    cfg.validate()?;
    if let Some(parent)=path.parent(){fs::create_dir_all(parent)?}
    let tmp=path.with_extension("toml.tmp");
    fs::write(&tmp,toml::to_string_pretty(cfg)?)?;
    fs::rename(tmp,path)?;
    Ok(())
}

fn display_name(path:&Path)->String{
    path.file_stem().and_then(|x|x.to_str()).unwrap_or("profile").to_string()
}

fn clamp(v:f64)->f32{v.clamp(0.,1.) as f32}

fn add_margins<W:gtk4::prelude::WidgetExt>(w:&W,m:i32){
    w.set_margin_top(m);
    w.set_margin_bottom(m);
    w.set_margin_start(m);
    w.set_margin_end(m);
}

fn set_status(ui:&Ui,msg:&str){
    ui.status.set_text(msg);
}


fn selected_position(cfg:&Config,sel:BindingRef)->Option<(f32,f32)>{
    match sel{
        BindingRef::Tap(i)=>cfg.taps.get(i).map(|x|(x.x,x.y)),
        BindingRef::Hold(i)=>cfg.holds.get(i).map(|x|(x.x,x.y)),
        BindingRef::MouseTap(i)=>cfg.mouse_taps.get(i).map(|x|(x.x,x.y)),
        BindingRef::MouseHold(i)=>cfg.mouse_holds.get(i).map(|x|(x.x,x.y)),
        BindingRef::Aim=>cfg.aim.as_ref().map(|x|(x.center_x,x.center_y)),
        BindingRef::Joystick=>cfg.joystick.as_ref().map(|x|(x.center_x,x.center_y)),
    }
}

fn set_selected_position(cfg:&mut Config,sel:BindingRef,x:f32,y:f32){
    match sel{
        BindingRef::Tap(i)=>if let Some(v)=cfg.taps.get_mut(i){v.x=x;v.y=y},
        BindingRef::Hold(i)=>if let Some(v)=cfg.holds.get_mut(i){v.x=x;v.y=y},
        BindingRef::MouseTap(i)=>if let Some(v)=cfg.mouse_taps.get_mut(i){v.x=x;v.y=y},
        BindingRef::MouseHold(i)=>if let Some(v)=cfg.mouse_holds.get_mut(i){v.x=x;v.y=y},
        BindingRef::Aim=>if let Some(v)=cfg.aim.as_mut(){v.center_x=x;v.center_y=y},
        BindingRef::Joystick=>if let Some(v)=cfg.joystick.as_mut(){v.center_x=x;v.center_y=y},
    }
}

fn marker_color(sel:BindingRef)->(f64,f64,f64){
    match sel{
        BindingRef::Aim=>(1.,0.30,0.30),
        BindingRef::Joystick=>(0.30,1.,0.50),
        BindingRef::Tap(_)=>(1.,0.80,0.25),
        BindingRef::Hold(_)=>(0.40,0.70,1.),
        BindingRef::MouseTap(_)=>(0.85,0.55,1.),
        BindingRef::MouseHold(_)=>(1.,0.45,0.80),
    }
}

fn binding_label(sel:BindingRef,cfg:&Config)->String{
    match sel{
        BindingRef::Tap(i)=>cfg.taps.get(i).map(|x|format!("⌨ {} • TAP",x.key)).unwrap_or_default(),
        BindingRef::Hold(i)=>cfg.holds.get(i).map(|x|format!("⌨ {} • HOLD",x.key)).unwrap_or_default(),
        BindingRef::MouseTap(i)=>cfg.mouse_taps.get(i).map(|x|format!("🖱 {} • TAP",x.button)).unwrap_or_default(),
        BindingRef::MouseHold(i)=>cfg.mouse_holds.get(i).map(|x|format!("🖱 {} • HOLD",x.button)).unwrap_or_default(),
        BindingRef::Aim=>format!("🎯 AIM • {}",cfg.aim.as_ref().map(|x|x.button.as_str()).unwrap_or("-")),
        BindingRef::Joystick=>String::from("🕹 JOYSTICK"),
    }
}

fn all_selectable(cfg:&Config)->Vec<BindingRef>{
    let mut v=Vec::new();
    if cfg.joystick.is_some(){v.push(BindingRef::Joystick)}
    if cfg.aim.is_some(){v.push(BindingRef::Aim)}
    v.extend((0..cfg.taps.len()).map(BindingRef::Tap));
    v.extend((0..cfg.holds.len()).map(BindingRef::Hold));
    v.extend((0..cfg.mouse_taps.len()).map(BindingRef::MouseTap));
    v.extend((0..cfg.mouse_holds.len()).map(BindingRef::MouseHold));
    v
}

fn nearest_binding(cfg:&Config,x:f32,y:f32)->Option<BindingRef>{
    let mut best=None;
    let mut best_d=0.045_f32;
    for sel in all_selectable(cfg){
        if let Some((px,py))=selected_position(cfg,sel){
            let d=((px-x)*(px-x)+(py-y)*(py-y)).sqrt();
            if d<best_d{best_d=d;best=Some(sel)}
        }
    }
    best
}

fn draw_canvas(ui_state:&Rc<RefCell<State>>,_area:&DrawingArea,cr:&cairo::Context,w:i32,h:i32){
    let st=ui_state.borrow();
    let cfg=&st.cfg;
    let pad=16.;
    let cw=(w as f64-2.*pad).max(10.);
    let ch=(h as f64-2.*pad).max(10.);
    let scale=(cw/cfg.display.width.max(1) as f64).min(ch/cfg.display.height.max(1) as f64);
    let vw=cfg.display.width as f64*scale;
    let vh=cfg.display.height as f64*scale;
    let ox=(w as f64-vw)/2.;
    let oy=(h as f64-vh)/2.;

    cr.set_source_rgb(0.055,0.065,0.08);
    cr.rectangle(0.,0.,w as f64,h as f64); let _=cr.fill();
    cr.set_source_rgb(0.10,0.115,0.14);
    cr.rectangle(ox,oy,vw,vh); let _=cr.fill();

    cr.set_source_rgb(0.15,0.17,0.20);
    for n in 1..10{
        let gx=ox+vw*(n as f64/10.);
        let gy=oy+vh*(n as f64/10.);
        cr.move_to(gx,oy);cr.line_to(gx,oy+vh);
        cr.move_to(ox,gy);cr.line_to(ox+vw,gy);
    }
    let _=cr.stroke();

    let p=|x:f32,y:f32|(ox+vw*x as f64,oy+vh*y as f64);

    if let Some(j)=&cfg.joystick{
        let (x,y)=p(j.center_x,j.center_y);
        let r=vw.min(vh)*j.radius as f64;
        cr.set_source_rgba(0.20,0.95,0.45,0.16);
        cr.arc(x,y,r,0.,std::f64::consts::TAU);let _=cr.fill();
        cr.set_source_rgb(0.30,1.,0.50);
        cr.arc(x,y,r,0.,std::f64::consts::TAU);let _=cr.stroke();
        cr.arc(x,y,5.,0.,std::f64::consts::TAU);let _=cr.fill();
        cr.move_to(x+8.,y-8.);cr.show_text("WASD");
    }

    let draw_marker=|sel:BindingRef,x:f32,y:f32,text:&str|{
        let (px,py)=p(x,y);
        let (r,g,b)=marker_color(sel);
        let selected=st.selected==Some(sel);
        let radius=if selected{13.}else{10.};
        cr.set_source_rgba(r,g,b,0.20);
        cr.arc(px,py,radius+6.,0.,std::f64::consts::TAU);let _=cr.fill();
        cr.set_source_rgb(r,g,b);
        cr.arc(px,py,radius,0.,std::f64::consts::TAU);let _=cr.fill();
        if selected{
            cr.set_source_rgb(1.,1.,1.);
            cr.arc(px,py,radius+4.,0.,std::f64::consts::TAU);let _=cr.stroke();
        }
        cr.set_source_rgb(1.,1.,1.);
        cr.move_to(px+radius+5.,py+4.);cr.show_text(text);
    };

    if let Some(a)=&cfg.aim{draw_marker(BindingRef::Aim,a.center_x,a.center_y,"AIM");}
    for(i,x)in cfg.taps.iter().enumerate(){draw_marker(BindingRef::Tap(i),x.x,x.y,&x.key);}
    for(i,x)in cfg.holds.iter().enumerate(){draw_marker(BindingRef::Hold(i),x.x,x.y,&format!("{} HOLD",x.key));}
    for(i,x)in cfg.mouse_taps.iter().enumerate(){draw_marker(BindingRef::MouseTap(i),x.x,x.y,&format!("{} TAP",x.button));}
    for(i,x)in cfg.mouse_holds.iter().enumerate(){draw_marker(BindingRef::MouseHold(i),x.x,x.y,&format!("{} HOLD",x.button));}

    cr.set_source_rgb(0.60,0.63,0.68);
    cr.move_to(ox,oy+vh+22.);
    cr.show_text("Click a marker to select • drag to reposition • Save to persist");
}

fn form_row(grid:&Grid,row:i32,label:&str,w:&impl gtk4::prelude::WidgetExt){
    let l=Label::new(Some(label));l.set_halign(gtk4::Align::Start);
    grid.attach(&l,0,row,1,1);grid.attach(w,1,row,1,1);
}

fn fill_devices(combo:&ComboBoxText,selected:&Option<String>,mouse:bool){
    combo.remove_all();
    combo.append(None,"(None)");

    let candidates:Vec<input::InputDeviceInfo>=input::list_input_devices()
        .into_iter()
        .filter(|d|if mouse{d.is_mouse}else{d.is_keyboard})
        .collect();

    for d in &candidates{
        let label=format!("{} — {}",d.name,d.path);
        combo.append(Some(&d.path),&label);
    }

    if let Some(s)=selected{
        let found=candidates.iter().any(|d|d.path==*s);
        if !found{
            combo.append(Some(s),&format!("⚠ Missing device — {}",s));
        }
        if !combo.set_active_id(Some(s)){combo.set_active(Some(0));}
    }else if let Some(d)=candidates.first(){
        combo.set_active_id(Some(&d.path));
    }else{
        combo.set_active(Some(0));
    }
}

fn make_spin(min:f64,max:f64,step:f64,digits:u32)->SpinButton{
    let s=SpinButton::with_range(min,max,step);s.set_digits(digits);s
}

fn add_section(parent:&GtkBox,title:&str)->GtkBox{
    let outer=GtkBox::new(Orientation::Vertical,6);
    let frame=Frame::new(Some(title));frame.set_child(Some(&outer));add_margins(&frame,6);
    parent.append(&frame);outer
}

fn rebuild_bindings(ui:&Ui){
    while let Some(child)=ui.bindings_box.first_child(){ui.bindings_box.remove(&child);}
    let cfg=ui.state.borrow().cfg.clone();

    let mk_group=|title:&str|->GtkBox{
        let box_=GtkBox::new(Orientation::Vertical,4);
        let l=Label::new(Some(title));l.add_css_class("heading");l.set_halign(gtk4::Align::Start);
        box_.append(&l);box_
    };

    for (title,items,make_sel) in [
        ("Keyboard TAP",cfg.taps.len(),BindingRef::Tap as fn(usize)->BindingRef),
        ("Keyboard HOLD",cfg.holds.len(),BindingRef::Hold as fn(usize)->BindingRef),
        ("Mouse TAP",cfg.mouse_taps.len(),BindingRef::MouseTap as fn(usize)->BindingRef),
        ("Mouse HOLD",cfg.mouse_holds.len(),BindingRef::MouseHold as fn(usize)->BindingRef),
    ]{
        if items==0{continue}
        let group=mk_group(title);
        for i in 0..items{
            let sel=make_sel(i);
            let row=GtkBox::new(Orientation::Horizontal,6);
            let text=Label::new(Some(&binding_label(sel,&cfg)));text.set_halign(gtk4::Align::Start);text.set_hexpand(true);
            let edit=Button::with_label("Edit");
            let del=Button::with_label("Delete");
            let ui2=ui.clone();
            edit.connect_clicked(move |_|{open_binding_dialog(&ui2,Some(sel));});
            let ui3=ui.clone();
            del.connect_clicked(move |_|{delete_binding(&ui3,sel);});
            row.append(&text);row.append(&edit);row.append(&del);
            group.append(&row);
        }
        let holder=GtkBox::new(Orientation::Vertical,2);holder.append(&group);
        ui.bindings_box.append(&holder);
    }
    if let Some(j)=&cfg.joystick{
        let row=GtkBox::new(Orientation::Horizontal,6);
        let text=Label::new(Some(&format!("🕹 JOYSTICK • WASD • {:0.0}% {:0.0}%",j.center_x*100.,j.center_y*100.)));
        text.set_halign(gtk4::Align::Start);text.set_hexpand(true);
        let edit=Button::with_label("Edit");
        let ui2=ui.clone();edit.connect_clicked(move |_|{select_binding(&ui2,BindingRef::Joystick);});
        row.append(&text);row.append(&edit);ui.bindings_box.append(&row);
    }
    if let Some(a)=&cfg.aim{
        let row=GtkBox::new(Orientation::Horizontal,6);
        let text=Label::new(Some(&format!("🎯 AIM • {} • {} • {:0.2}x",a.button,a.mode,a.sensitivity)));
        text.set_halign(gtk4::Align::Start);text.set_hexpand(true);
        let edit=Button::with_label("Edit");
        let ui2=ui.clone();edit.connect_clicked(move |_|{select_binding(&ui2,BindingRef::Aim);});
        row.append(&text);row.append(&edit);ui.bindings_box.append(&row);
    }
    ui.bindings_box.queue_draw();
}

fn select_binding(ui:&Ui,sel:BindingRef){
    ui.state.borrow_mut().selected=Some(sel);
    ui.canvas.queue_draw();
    set_status(ui,&format!("Selected: {}",binding_label(sel,&ui.state.borrow().cfg)));
}

fn delete_binding(ui:&Ui,sel:BindingRef){
    {
        let mut st=ui.state.borrow_mut();
        match sel{
            BindingRef::Tap(i)=>{if i<st.cfg.taps.len(){st.cfg.taps.remove(i);}},
            BindingRef::Hold(i)=>{if i<st.cfg.holds.len(){st.cfg.holds.remove(i);}},
            BindingRef::MouseTap(i)=>{if i<st.cfg.mouse_taps.len(){st.cfg.mouse_taps.remove(i);}},
            BindingRef::MouseHold(i)=>{if i<st.cfg.mouse_holds.len(){st.cfg.mouse_holds.remove(i);}},
            BindingRef::Aim=>st.cfg.aim=None,
            BindingRef::Joystick=>st.cfg.joystick=None,
        }
        st.selected=None;st.dirty=true;
    }
    rebuild_bindings(ui);sync_form(ui);ui.canvas.queue_draw();set_status(ui,"Binding deleted — press Save");
}

#[derive(Clone,Copy)]
enum EditType{KeyboardTap,KeyboardHold,MouseTap,MouseHold}

fn key_alias(name:&str)->String{
    let n=name.to_ascii_uppercase();
    match n.as_str(){
        "SPACE"=>"SPACE".into(),"RETURN"|"ENTER"=>"ENTER".into(),
        "ESCAPE"|"ESC"=>"ESC".into(),"TAB"=>"TAB".into(),
        "SHIFT_L"|"SHIFT_R"|"SHIFT"=>"SHIFT".into(),
        "CONTROL_L"|"CONTROL_R"|"CTRL"=>"CTRL".into(),
        "ALT_L"|"ALT_R"|"ALT"=>"ALT".into(),
        "BACKSPACE"=>"BACKSPACE".into(),"DELETE"=>"DELETE".into(),
        "INSERT"=>"INSERT".into(),"HOME"=>"HOME".into(),"END"=>"END".into(),
        "PAGE_UP"=>"PAGEUP".into(),"PAGE_DOWN"=>"PAGEDOWN".into(),
        "UP"=>"UP".into(),"DOWN"=>"DOWN".into(),"LEFT"=>"LEFT".into(),"RIGHT"=>"RIGHT".into(),
        x=>x.into(),
    }
}

fn attach_key_capture(entry:&Entry,button:&Button,status:&Label){
    let armed=Rc::new(Cell::new(false));
    let a=armed.clone();let e=entry.clone();let s=status.clone();
    button.connect_clicked(move |_|{a.set(true);e.grab_focus();s.set_text("Press a key…");});
    let controller=EventControllerKey::new();
    let a2=armed.clone();let e2=entry.clone();let s2=status.clone();
    controller.connect_key_pressed(move |_,key,_,_|{
        if !a2.get(){return glib::Propagation::Proceed}
        let name=key.name().map(|x|x.to_string()).unwrap_or_default();
        if !name.is_empty(){e2.set_text(&key_alias(&name));a2.set(false);s2.set_text("Key captured");}
        glib::Propagation::Stop
    });
    entry.add_controller(controller);
}

fn open_add_dialog(ui:&Ui,kind:EditType){
    open_binding_dialog_inner(ui,None,kind);
}

fn open_binding_dialog(ui:&Ui,existing:Option<BindingRef>){
    let kind=match existing{
        Some(BindingRef::Tap(_))=>EditType::KeyboardTap,
        Some(BindingRef::Hold(_))=>EditType::KeyboardHold,
        Some(BindingRef::MouseTap(_))=>EditType::MouseTap,
        Some(BindingRef::MouseHold(_))=>EditType::MouseHold,
        _=>return,
    };
    open_binding_dialog_inner(ui,existing,kind);
}

fn open_binding_dialog_inner(ui:&Ui,existing:Option<BindingRef>,kind:EditType){
    let parent=ui.canvas.root().and_then(|w|w.downcast::<ApplicationWindow>().ok());
    let Some(parent)=parent else{return};
    let dialog=Dialog::builder().transient_for(&parent).modal(true)
        .title(match kind{EditType::KeyboardTap=>"Keyboard Tap",EditType::KeyboardHold=>"Keyboard Hold",EditType::MouseTap=>"Mouse Tap",EditType::MouseHold=>"Mouse Hold"})
        .build();
    dialog.add_button("Cancel",gtk4::ResponseType::Cancel);
    dialog.add_button("Save",gtk4::ResponseType::Accept);

    let box_=GtkBox::new(Orientation::Vertical,8);add_margins(&box_,12);
    let grid=Grid::new();grid.set_row_spacing(7);grid.set_column_spacing(10);

    let key_label=Label::new(Some(if matches!(kind,EditType::KeyboardTap|EditType::KeyboardHold){"Key"}else{"Mouse button"}));
    key_label.set_halign(gtk4::Align::Start);
    let key_entry=Entry::new();
    let capture=Button::with_label("Capture");
    let key_box=GtkBox::new(Orientation::Horizontal,5);key_box.append(&key_entry);key_box.append(&capture);
    grid.attach(&key_label,0,0,1,1);grid.attach(&key_box,1,0,1,1);

    let x=make_spin(0.,1.,0.01,3);let y=make_spin(0.,1.,0.01,3);let slot=make_spin(0.,15.,1.,0);
    grid.attach(&Label::new(Some("X")),0,1,1,1);grid.attach(&x,1,1,1,1);
    grid.attach(&Label::new(Some("Y")),0,2,1,1);grid.attach(&y,1,2,1,1);
    grid.attach(&Label::new(Some("Touch slot")),0,3,1,1);grid.attach(&slot,1,3,1,1);
    box_.append(&grid);dialog.content_area().append(&box_);

    let cfg=ui.state.borrow().cfg.clone();
    if let Some(sel)=existing{
        match sel{
            BindingRef::Tap(i)=>if let Some(v)=cfg.taps.get(i){key_entry.set_text(&v.key);x.set_value(v.x as f64);y.set_value(v.y as f64);slot.set_value(v.slot as f64)},
            BindingRef::Hold(i)=>if let Some(v)=cfg.holds.get(i){key_entry.set_text(&v.key);x.set_value(v.x as f64);y.set_value(v.y as f64);slot.set_value(v.slot as f64)},
            BindingRef::MouseTap(i)=>if let Some(v)=cfg.mouse_taps.get(i){key_entry.set_text(&v.button);x.set_value(v.x as f64);y.set_value(v.y as f64);slot.set_value(v.slot as f64)},
            BindingRef::MouseHold(i)=>if let Some(v)=cfg.mouse_holds.get(i){key_entry.set_text(&v.button);x.set_value(v.x as f64);y.set_value(v.y as f64);slot.set_value(v.slot as f64)},
            _=>{},
        }
    }else{
        match kind{
            EditType::KeyboardTap|EditType::KeyboardHold=>key_entry.set_text("SPACE"),
            EditType::MouseTap|EditType::MouseHold=>key_entry.set_text("MOUSE_LEFT"),
        }
        slot.set_value(2.);
    }
    let status=ui.status.clone();
    if matches!(kind,EditType::KeyboardTap|EditType::KeyboardHold){attach_key_capture(&key_entry,&capture,&status)}else{capture.set_visible(false);}

    let ui2=ui.clone();
    dialog.connect_response(move |d,response|{
        if response==gtk4::ResponseType::Accept{
            let token=key_entry.text().trim().to_string();
            let xx=x.value() as f32;let yy=y.value() as f32;let ss=slot.value() as u8;
            let result=if matches!(kind,EditType::KeyboardTap|EditType::KeyboardHold){
                input::key_code(&token).map(|_|()).map_err(|e|e.to_string())
            }else{
                input::button_code(&token).map(|_|()).map_err(|e|e.to_string())
            };
            match result{
                Ok(())=>{
                    let mut st=ui2.state.borrow_mut();
                    match (existing,kind){
                        (Some(BindingRef::Tap(i)),EditType::KeyboardTap)=>if let Some(v)=st.cfg.taps.get_mut(i){v.key=token.clone();v.x=xx;v.y=yy;v.slot=ss},
                        (Some(BindingRef::Hold(i)),EditType::KeyboardHold)=>if let Some(v)=st.cfg.holds.get_mut(i){v.key=token.clone();v.x=xx;v.y=yy;v.slot=ss},
                        (Some(BindingRef::MouseTap(i)),EditType::MouseTap)=>if let Some(v)=st.cfg.mouse_taps.get_mut(i){v.button=token.clone();v.x=xx;v.y=yy;v.slot=ss},
                        (Some(BindingRef::MouseHold(i)),EditType::MouseHold)=>if let Some(v)=st.cfg.mouse_holds.get_mut(i){v.button=token.clone();v.x=xx;v.y=yy;v.slot=ss},
                        (None,EditType::KeyboardTap)=>st.cfg.taps.push(Tap{key:token.clone(),x:xx,y:yy,slot:ss}),
                        (None,EditType::KeyboardHold)=>st.cfg.holds.push(Hold{key:token.clone(),x:xx,y:yy,slot:ss}),
                        (None,EditType::MouseTap)=>st.cfg.mouse_taps.push(MouseTap{button:token.clone(),x:xx,y:yy,slot:ss}),
                        (None,EditType::MouseHold)=>st.cfg.mouse_holds.push(MouseHold{button:token.clone(),x:xx,y:yy,slot:ss}),
                        _=>{},
                    }
                    st.dirty=true;
                    drop(st);
                    rebuild_bindings(&ui2);set_status(&ui2,"Binding saved — press Save");ui2.canvas.queue_draw();
                    d.close();
                }
                Err(e)=>set_status(&ui2,&format!("Invalid binding: {e}")),
            }
        }else{d.close();}
    });
    dialog.show();
}

fn sync_form(ui:&Ui){
    let st=ui.state.borrow();
    ui.profile_name.set_text(&display_name(&st.profile_path));
    ui.width.set_value(st.cfg.display.width as f64);
    ui.height.set_value(st.cfg.display.height as f64);
    ui.aim_enabled.set_active(st.cfg.aim.is_some());
    if let Some(a)=&st.cfg.aim{
        ui.aim_button.set_text(&a.button);
        ui.aim_mode.set_active_id(Some(a.mode.as_str()));
        ui.aim_x.set_value(a.center_x as f64);ui.aim_y.set_value(a.center_y as f64);
        ui.aim_sensitivity.set_value(a.sensitivity as f64);ui.aim_slot.set_value(a.slot as f64);
        ui.aim_invert_y.set_active(a.invert_y);
    }
    ui.joy_enabled.set_active(st.cfg.joystick.is_some());
    if let Some(j)=&st.cfg.joystick{
        ui.joy_up.set_text(&j.up);ui.joy_down.set_text(&j.down);ui.joy_left.set_text(&j.left);ui.joy_right.set_text(&j.right);
        ui.joy_x.set_value(j.center_x as f64);ui.joy_y.set_value(j.center_y as f64);
        ui.joy_radius.set_value(j.radius as f64);ui.joy_slot.set_value(j.slot as f64);
    }
    ui.grab.set_active(st.cfg.performance.grab);
    ui.realtime.set_active(st.cfg.performance.realtime);
    ui.mouse_lock.set_active(st.cfg.performance.mouse_lock);
    ui.mouse_toggle.set_text(&st.cfg.performance.mouse_toggle_key);
    fill_devices(&ui.keyboard,&st.cfg.devices.keyboard,false);
    fill_devices(&ui.mouse,&st.cfg.devices.mouse,true);
}

fn sync_state_from_form(ui:&Ui){
    let mut st=ui.state.borrow_mut();
    let w=ui.width.value().round() as i32;let h=ui.height.value().round() as i32;
    st.cfg.display.width=w;st.cfg.display.height=h;
    st.cfg.devices.keyboard=ui.keyboard.active_id().map(|x|x.to_string());
    st.cfg.devices.mouse=ui.mouse.active_id().map(|x|x.to_string());
    st.cfg.performance.grab=ui.grab.is_active();
    st.cfg.performance.realtime=ui.realtime.is_active();
    st.cfg.performance.mouse_lock=ui.mouse_lock.is_active();
    st.cfg.performance.mouse_toggle_key=ui.mouse_toggle.text().trim().to_string();

    if ui.aim_enabled.is_active(){
        st.cfg.aim=Some(Aim{
            button:ui.aim_button.text().trim().to_string(),
            center_x:ui.aim_x.value() as f32,center_y:ui.aim_y.value() as f32,
            sensitivity:ui.aim_sensitivity.value() as f32,slot:ui.aim_slot.value() as u8,
            invert_y:ui.aim_invert_y.is_active(),
            mode:ui.aim_mode.active_id().map(|x|x.to_string()).unwrap_or_else(||"touch".into()),
        });
    }else{st.cfg.aim=None;}

    if ui.joy_enabled.is_active(){
        st.cfg.joystick=Some(Joystick{
            up:ui.joy_up.text().trim().to_string(),down:ui.joy_down.text().trim().to_string(),
            left:ui.joy_left.text().trim().to_string(),right:ui.joy_right.text().trim().to_string(),
            center_x:ui.joy_x.value() as f32,center_y:ui.joy_y.value() as f32,
            radius:ui.joy_radius.value() as f32,slot:ui.joy_slot.value() as u8,
        });
    }else{st.cfg.joystick=None;}
    st.dirty=true;
}

fn save_current(ui:&Ui)->Result<(),String>{
    sync_state_from_form(ui);
    let name=ui.profile_name.text().trim().to_string();
    if name.is_empty(){return Err("Profile name is empty".into())}
    let safe=name.chars().map(|c|if c.is_ascii_alphanumeric()||c=='-'||c=='_'{c}else{'_'}).collect::<String>();
    let safe=safe.trim_matches('_').to_string();
    if safe.is_empty(){return Err("Invalid profile name".into())}

    let mut st=ui.state.borrow_mut();
    let old_path=st.profile_path.clone();
    let new_path=profiles_dir().join(format!("{safe}.toml"));
    if new_path!=old_path && new_path.exists(){return Err("A profile with that name already exists".into())}
    match save_profile(&new_path,&st.cfg){
        Ok(())=>{
            if new_path!=old_path{let _=fs::remove_file(old_path);}
            st.profile_path=new_path;st.dirty=false;ui.profile_name.set_text(&safe);Ok(())
        }
        Err(e)=>Err(e.to_string()),
    }
}

const USER_SERVICE:&str="waydroid-keymapper.service";

fn user_bin_dir()->PathBuf{home_dir().join(".local/bin")}
fn user_service_dir()->PathBuf{home_dir().join(".config/systemd/user")}
fn daemon_install_path()->PathBuf{user_bin_dir().join("waydroid-keymapper")}
fn gui_install_path()->PathBuf{user_bin_dir().join("keymapper-gui")}
fn desktop_file_path()->PathBuf{home_dir().join(".local/share/applications/waydroid-keymapper.desktop")}

fn install_user_executable(src:&Path,dst:&Path)->Result<(),String>{
    if src.canonicalize().ok()==dst.canonicalize().ok(){return Ok(())}
    let tmp=dst.with_extension("tmp");
    fs::copy(src,&tmp).map_err(|e|format!("install {}: {e}",dst.display()))?;
    let mut perms=fs::metadata(&tmp).map_err(|e|e.to_string())?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tmp,perms).map_err(|e|e.to_string())?;
    fs::rename(&tmp,dst).map_err(|e|format!("activate {}: {e}",dst.display()))?;
    Ok(())
}

fn desktop_entry()->String{
    let exe=gui_install_path().to_string_lossy().replace('\\',"\\\\").replace(' ',"\\ ");
    format!("[Desktop Entry]\\nType=Application\\nName=Waydroid Keymapper\\nComment=Low-latency Waydroid keyboard and mouse profile editor\\nExec={}\\nIcon=input-gaming\\nTerminal=false\\nCategories=Utility;Game;\\nKeywords=Waydroid;Android;Gaming;Keymapper;\\n",exe)
}

fn udev_rules_text()->&'static str{
    r#"KERNEL=="uinput", MODE="0660", GROUP="input", TAG+="uaccess"
SUBSYSTEM=="input", KERNEL=="event*", MODE="0660", GROUP="input", TAG+="uaccess"
"#
}

fn install_input_permissions()->Result<String,String>{
    let dir=home_dir().join(".config/waydroid-keymapper");
    fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
    let tmp=dir.join("99-waydroid-keymapper.rules.tmp");
    fs::write(&tmp,udev_rules_text()).map_err(|e|format!("write temporary udev rules: {e}"))?;

    let install=Command::new("pkexec")
        .args(["install","-Dm644",tmp.to_string_lossy().as_ref(),"/etc/udev/rules.d/99-waydroid-keymapper.rules"])
        .output()
        .map_err(|e|format!("pkexec unavailable: {e}"))?;
    if !install.status.success(){
        let _=fs::remove_file(&tmp);
        let err=String::from_utf8_lossy(&install.stderr).trim().to_string();
        return Err(if err.is_empty(){"authentication cancelled or udev rule installation failed".into()}else{err});
    }

    let reload=Command::new("pkexec").args(["udevadm","control","--reload-rules"]).output()
        .map_err(|e|format!("pkexec udevadm unavailable: {e}"))?;
    if !reload.status.success(){
        let _=fs::remove_file(&tmp);
        return Err(String::from_utf8_lossy(&reload.stderr).trim().to_string());
    }

    let trigger=Command::new("pkexec").args(["udevadm","trigger","--subsystem-match=input"]).output()
        .map_err(|e|format!("pkexec udevadm trigger unavailable: {e}"))?;
    let _=fs::remove_file(&tmp);
    if !trigger.status.success(){
        return Err(String::from_utf8_lossy(&trigger.stderr).trim().to_string());
    }

    Ok("Input permissions repaired ✓".into())
}

fn daemon_source()->Option<PathBuf>{
    let exe=env::current_exe().ok();
    let mut candidates=Vec::new();
    if let Some(e)=exe{
        if let Some(parent)=e.parent(){candidates.push(parent.join("waydroid-keymapper"));}
    }
    if let Ok(cwd)=env::current_dir(){
        candidates.push(cwd.join("target/release/waydroid-keymapper"));
        candidates.push(cwd.join("waydroid-keymapper"));
    }
    candidates.push(daemon_install_path());
    for p in candidates{
        if p.is_file(){return Some(p)}
    }
    None
}

fn service_unit()->String{
    format!(r#"[Unit]
Description=Waydroid Rust Game Keymapper
After=graphical-session.target
Wants=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/waydroid-keymapper run %h/.config/waydroid-keymapper/config.toml
Restart=on-failure
RestartSec=1
Nice=0

[Install]
WantedBy=graphical-session.target
"#)
}

fn install_runtime()->Result<(),String>{
    fs::create_dir_all(user_bin_dir()).map_err(|e|e.to_string())?;
    fs::create_dir_all(user_service_dir()).map_err(|e|e.to_string())?;

    let dst=daemon_install_path();
    if let Some(src)=daemon_source(){
        install_user_executable(&src,&dst)?;
    }else if !dst.is_file(){
        return Err("waydroid-keymapper binary not found next to the GUI or in ~/.local/bin".into())
    }

    if let Ok(exe)=env::current_exe(){
        if exe.is_file(){install_user_executable(&exe,&gui_install_path())?;}
    }

    if !active_config_path().is_file(){
        let seed=profiles_dir().join("default.toml");
        if seed.is_file(){
            if let Some(parent)=active_config_path().parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
            let tmp=active_config_path().with_extension("toml.tmp");
            fs::copy(&seed,&tmp).map_err(|e|format!("seed active config: {e}"))?;
            fs::rename(&tmp,active_config_path()).map_err(|e|format!("activate default config: {e}"))?;
        }
    }

    if let Some(parent)=desktop_file_path().parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
    let desktop=desktop_file_path();
    let desktop_tmp=desktop.with_extension("desktop.tmp");
    fs::write(&desktop_tmp,desktop_entry()).map_err(|e|format!("write desktop launcher: {e}"))?;
    fs::rename(&desktop_tmp,&desktop).map_err(|e|format!("activate desktop launcher: {e}"))?;

    let unit=user_service_dir().join(USER_SERVICE);
    let tmp=unit.with_extension("tmp");
    fs::write(&tmp,service_unit()).map_err(|e|format!("write service: {e}"))?;
    fs::rename(&tmp,&unit).map_err(|e|format!("activate service: {e}"))?;

    let reload=Command::new("systemctl").args(["--user","daemon-reload"]).output()
        .map_err(|e|format!("systemctl daemon-reload: {e}"))?;
    if !reload.status.success(){
        return Err(String::from_utf8_lossy(&reload.stderr).trim().to_string())
    }
    Ok(())
}

fn systemctl_user(args:&[&str])->Result<String,String>{
    let out=Command::new("systemctl").args(["--user"]).args(args).output().map_err(|e|e.to_string())?;
    if out.status.success(){Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())}
    else{
        let err=String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty(){String::from_utf8_lossy(&out.stdout).trim().to_string()}else{err})
    }
}

fn service_action(action:&str)->Result<String,String>{
    match action{
        "stop"=>{
            match systemctl_user(&["stop",USER_SERVICE]){
                Ok(x)=>Ok(x),
                Err(e) if e.contains("not loaded") || e.contains("not found")=>Ok(String::new()),
                Err(e)=>Err(e),
            }
        }
        "start"=>{
            install_runtime()?;
            systemctl_user(&["start",USER_SERVICE])
        }
        "restart"=>{
            install_runtime()?;
            match systemctl_user(&["restart",USER_SERVICE]){
                Ok(x)=>Ok(x),
                Err(e) if e.contains("not loaded") || e.contains("not found")=>systemctl_user(&["start",USER_SERVICE]),
                Err(e)=>Err(e),
            }
        }
        "enable"=>{
            install_runtime()?;
            systemctl_user(&["enable",USER_SERVICE])
        }
        "disable"=>{
            match systemctl_user(&["disable","--now",USER_SERVICE]){
                Ok(x)=>Ok(x),
                Err(e) if e.contains("not loaded") || e.contains("not found")=>Ok(String::new()),
                Err(e)=>Err(e),
            }
        }
        _=>Err("unknown service action".into()),
    }
}

fn runtime_service_state()->String{
    let out=Command::new("systemctl").args(["--user","is-active",USER_SERVICE]).output();
    match out{
        Ok(o) if o.status.success()=>"Running".into(),
        Ok(_)=>{
            let failed=Command::new("systemctl").args(["--user","is-failed",USER_SERVICE]).output();
            if matches!(failed,Ok(ref x) if x.status.success()){"Failed".into()}
            else if user_service_dir().join(USER_SERVICE).is_file(){"Stopped".into()}
            else{"Not installed".into()}
        }
        Err(_)=>"systemctl unavailable".into(),
    }
}

fn device_access(path:Option<String>)->String{
    let Some(path)=path else{return "not selected".into()};
    match evdev::Device::open(&path){
        Ok(_)=>format!("OK • {}",path),
        Err(e)=>format!("DENIED • {} • {}",path,e),
    }
}

fn update_runtime_status(ui:&Ui){
    let svc=runtime_service_state();
    ui.runtime_status.set_text(&format!("Service: {svc}"));
    let (kbd,mouse)={
        let st=ui.state.borrow();
        (st.cfg.devices.keyboard.clone(),st.cfg.devices.mouse.clone())
    };
    ui.input_access.set_text(&format!("Keyboard: {}\\nMouse: {}",device_access(kbd),device_access(mouse)));
    match control::request("status"){
        Ok(reply)=>{
            let locked=reply.split_whitespace().find_map(|x|x.strip_prefix("locked=")).unwrap_or("0");
            let running=reply.split_whitespace().find_map(|x|x.strip_prefix("running=")).unwrap_or("1");
            if running=="1"{
                ui.lock_status.set_text(if locked=="1"{"Mouse: 🔒 LOCKED"}else{"Mouse: 🖱 UNLOCKED"});
            }else{
                ui.lock_status.set_text("Mouse: offline");
            }
        }
        Err(_)=>ui.lock_status.set_text("Mouse: offline"),
    }
}

fn diagnostics(ui:&Ui){
    sync_state_from_form(ui);
    let cfg=ui.state.borrow().cfg.clone();
    let mut issues=Vec::new();

    if !cfg.conflicts().is_empty(){issues.push(format!("{} input conflict(s)",cfg.conflicts().len()));}
    if let Err(e)=cfg.validate(){issues.push(format!("config: {e}"));}

    if let Some(k)=cfg.devices.keyboard.clone(){
        if let Err(e)=evdev::Device::open(&k){issues.push(format!("keyboard access: {e}"));}
    }else{issues.push("keyboard device not selected".into());}

    if let Some(m)=cfg.devices.mouse.clone(){
        if let Err(e)=evdev::Device::open(&m){issues.push(format!("mouse access: {e}"));}
    }else{issues.push("mouse device not selected".into());}

    let touch=cfg.touch_fifo();
    if !Path::new(&touch).exists(){issues.push(format!("touch FIFO missing: {touch}"));}
    if cfg.aim.as_ref().is_some_and(|a|a.mode.eq_ignore_ascii_case("relative")){
        let pointer=cfg.pointer_fifo();
        if !Path::new(&pointer).exists(){issues.push(format!("pointer FIFO missing: {pointer}"));}
    }

    match control::request("ping"){
        Ok(reply) if reply=="OK pong"=>{},
        Ok(reply)=>issues.push(format!("daemon control: {reply}")),
        Err(_)=>issues.push("daemon control socket offline".into()),
    }

    let service=runtime_service_state();
    if service=="Failed"{issues.push("daemon service is failed".into());}

    let waydroid=waydroid_state();
    if waydroid=="Unavailable"{issues.push("Waydroid command unavailable".into());}

    if issues.is_empty(){
        set_status(ui,"Diagnostics: ALL CHECKS PASSED ✓");
    }else{
        set_status(ui,&format!("Diagnostics: {} issue(s) • {}",issues.len(),issues.join(" | ")));
    }
    update_runtime_status(ui);
}

fn runtime_control(ui:&Ui,command:&str){
    match control::request(command){
        Ok(reply)=>set_status(ui,&format!("Daemon: {reply}")),
        Err(e)=>set_status(ui,&format!("Daemon control unavailable: {e}")),
    }
    update_runtime_status(ui);
}

fn waydroid_action(ui:&Ui,action:&str){
    let result=Command::new("waydroid").args(["session",action]).spawn();
    match result{
        Ok(_)=>set_status(ui,&format!("Waydroid session {action} requested")),
        Err(e)=>set_status(ui,&format!("Waydroid command failed: {e}")),
    }
}

fn waydroid_state()->String{
    match Command::new("waydroid").arg("status").output(){
        Ok(o)=>{
            let x=String::from_utf8_lossy(&o.stdout).trim().replace('\n'," • ");
            if x.is_empty(){String::from("Unknown")}else{x}
        }
        Err(_) => String::from("Unavailable"),
    }
}

fn apply_and_run(ui:&Ui){
    if let Err(e)=save_current(ui){set_status(ui,&format!("Save failed: {e}"));return}
    if let Err(e)=install_runtime(){set_status(ui,&format!("Runtime setup failed: {e}"));return}

    let profile_path=ui.state.borrow().profile_path.clone();
    let active=active_config_path();
    let data=match fs::read_to_string(&profile_path){
        Ok(x)=>x,
        Err(e)=>{set_status(ui,&format!("Read profile failed: {e}"));return}
    };
    if let Some(parent)=active.parent(){let _=fs::create_dir_all(parent);}
    let tmp=active.with_extension("toml.tmp");
    if let Err(e)=fs::write(&tmp,data){set_status(ui,&format!("Write active config failed: {e}"));return}
    if let Err(e)=fs::rename(&tmp,&active){
        let _=fs::remove_file(&tmp);
        set_status(ui,&format!("Activate config failed: {e}"));
        return
    }

    match service_action("restart"){
        Ok(_)=>set_status(ui,"Profile applied • daemon started/restarted ✓"),
        Err(e)=>set_status(ui,&format!("Profile applied; daemon restart failed: {e}")),
    }
    rebuild_profiles(ui);
    update_runtime_status(ui);
}

fn rebuild_profiles(ui:&Ui){
    while let Some(child)=ui.profile_list.first_child(){ui.profile_list.remove(&child);}
    let files=profile_files();
    let current=ui.state.borrow().profile_path.clone();
    let mut selected=None;
    for p in files{
        let row=ListBoxRow::new();
        let label=Label::new(Some(&display_name(&p)));label.set_xalign(0.);
        add_margins(&label,8);row.set_child(Some(&label));
        if p==current{selected=Some(row.clone());}
        ui.profile_list.append(&row);
    }
    if let Some(row)=selected{ui.profile_list.select_row(Some(&row));}
}

fn load_selected_profile(ui:&Ui,row:&ListBoxRow){
    let index=row.index();
    let files=profile_files();
    let Some(path)=files.get(index as usize) else{return};
    match load_profile(path){
        Ok(cfg)=>{
            let mut st=ui.state.borrow_mut();
            st.cfg=cfg;st.profile_path=path.clone();st.selected=None;st.dirty=false;
            drop(st);
            sync_form(ui);rebuild_bindings(ui);ui.canvas.queue_draw();
            set_status(ui,&format!("Loaded profile {}",display_name(path)));
        }
        Err(e)=>set_status(ui,&format!("Load failed: {e}")),
    }
}

fn ask_name(parent:&ApplicationWindow,title:&str,initial:&str,callback:impl Fn(String)+'static){
    let dialog=Dialog::builder().transient_for(parent).modal(true).title(title).build();
    dialog.add_button("Cancel",gtk4::ResponseType::Cancel);
    dialog.add_button("OK",gtk4::ResponseType::Accept);
    let entry=Entry::new();entry.set_text(initial);entry.set_activates_default(true);add_margins(&entry,12);
    dialog.content_area().append(&entry);
    dialog.connect_response(move |d,r|{
        if r==gtk4::ResponseType::Accept{callback(entry.text().trim().to_string())}
        d.close();
    });
    dialog.show();
}

fn new_profile(ui:&Ui,app:&ApplicationWindow){
    let ui2=ui.clone();
    ask_name(app,"New profile","pubg",move|name|{
        if name.is_empty(){return}
        let path=profiles_dir().join(format!("{name}.toml"));
        if path.exists(){set_status(&ui2,"Profile already exists");return}
        let cfg=default_config();
        match save_profile(&path,&cfg){
            Ok(())=>{
                let mut st=ui2.state.borrow_mut();
                st.cfg=cfg;st.profile_path=path;st.selected=None;st.dirty=false;drop(st);
                sync_form(&ui2);rebuild_bindings(&ui2);rebuild_profiles(&ui2);ui2.canvas.queue_draw();
                set_status(&ui2,"New profile created");
            }
            Err(e)=>set_status(&ui2,&format!("Create failed: {e}")),
        }
    });
}

fn duplicate_profile(ui:&Ui,app:&ApplicationWindow){
    let current=ui.state.borrow().profile_path.clone();
    let ui2=ui.clone();
    ask_name(app,"Duplicate profile",&format!("{}_copy",display_name(&current)),move|name|{
        if name.is_empty(){return}
        let path=profiles_dir().join(format!("{name}.toml"));
        if path.exists(){set_status(&ui2,"Profile already exists");return}
        let cfg=ui2.state.borrow().cfg.clone();
        match save_profile(&path,&cfg){
            Ok(())=>{
                let mut st=ui2.state.borrow_mut();st.profile_path=path;st.dirty=false;drop(st);
                rebuild_profiles(&ui2);sync_form(&ui2);set_status(&ui2,"Profile duplicated");
            }
            Err(e)=>set_status(&ui2,&format!("Duplicate failed: {e}")),
        }
    });
}

fn delete_profile(ui:&Ui){
    let path=ui.state.borrow().profile_path.clone();
    if profile_files().len()<=1{set_status(ui,"Keep at least one profile");return}
    match fs::remove_file(&path){
        Ok(())=>{
            let next=profile_files().into_iter().next().unwrap();
            match load_profile(&next){
                Ok(cfg)=>{
                    let mut st=ui.state.borrow_mut();st.cfg=cfg;st.profile_path=next;st.selected=None;st.dirty=false;drop(st);
                    rebuild_profiles(ui);sync_form(ui);rebuild_bindings(ui);ui.canvas.queue_draw();set_status(ui,"Profile deleted");
                }
                Err(e)=>set_status(ui,&format!("Load replacement failed: {e}")),
            }
        }
        Err(e)=>set_status(ui,&format!("Delete failed: {e}")),
    }
}

fn validate_current(ui:&Ui){
    sync_state_from_form(ui);
    let st=ui.state.borrow();
    let conflicts=st.cfg.conflicts();
    if !conflicts.is_empty(){
        set_status(ui,&format!("{} input conflict(s): {}",conflicts.len(),conflicts.join(" | ")));
        return;
    }
    match st.cfg.validate(){Ok(())=>set_status(ui,"Configuration is valid ✓"),Err(e)=>set_status(ui,&format!("Validation error: {e}"))}
}

fn build_ui(app:&Application){
    let dir=ensure_profiles().unwrap_or_else(|_|profiles_dir());
    let current=profile_files().first().cloned().unwrap_or_else(||dir.join("default.toml"));
    let cfg=load_profile(&current).unwrap_or_else(|_|default_config());
    let state=Rc::new(RefCell::new(State{cfg,profile_path:current,selected:None,dirty:false}));

    let profile_list=ListBox::new();profile_list.set_selection_mode(gtk4::SelectionMode::Single);
    profile_list.set_vexpand(true);
    let canvas=DrawingArea::new();canvas.set_content_width(860);canvas.set_content_height(620);canvas.set_hexpand(true);canvas.set_vexpand(true);

    let status=Label::new(Some("Ready"));status.set_halign(gtk4::Align::Start);add_margins(&status,6);

    let profile_name=Entry::new();
    let width=make_spin(320.,16384.,1.,0);let height=make_spin(240.,16384.,1.,0);
    let keyboard=ComboBoxText::new();let mouse=ComboBoxText::new();

    let aim_enabled=CheckButton::with_label("Enable aim");
    let aim_button=Entry::new();
    let aim_mode=ComboBoxText::new();aim_mode.append(Some("touch"),"Touch / absolute");aim_mode.append(Some("relative"),"Relative / FPS");let aim_x=make_spin(0.,1.,0.01,3);let aim_y=make_spin(0.,1.,0.01,3);
    let aim_sensitivity=make_spin(0.01,20.,0.05,2);let aim_slot=make_spin(0.,15.,1.,0);let aim_invert_y=CheckButton::with_label("Invert Y");
    let joy_enabled=CheckButton::with_label("Enable joystick");
    let joy_up=Entry::new();let joy_down=Entry::new();let joy_left=Entry::new();let joy_right=Entry::new();
    let joy_x=make_spin(0.,1.,0.01,3);let joy_y=make_spin(0.,1.,0.01,3);let joy_radius=make_spin(0.01,1.,0.005,3);let joy_slot=make_spin(0.,15.,1.,0);
    let grab=CheckButton::with_label("Exclusive input grab");let realtime=CheckButton::with_label("Realtime preference");
    let mouse_lock=CheckButton::with_label("Lock mouse on start");
    let mouse_toggle=Entry::new();mouse_toggle.set_text("F8");
    let runtime_status=Label::new(Some("Service: Not installed"));
    let lock_status=Label::new(Some("Mouse: offline"));
    let input_access=Label::new(Some("Keyboard: checking…\\nMouse: checking…"));
    runtime_status.set_halign(gtk4::Align::Start);
    lock_status.set_halign(gtk4::Align::Start);
    input_access.set_halign(gtk4::Align::Start);
    input_access.set_wrap(true);

    let bindings_box=GtkBox::new(Orientation::Vertical,6);
    let ui=Ui{state:state.clone(),profile_list:profile_list.clone(),bindings_box:bindings_box.clone(),canvas:canvas.clone(),status:status.clone(),profile_name:profile_name.clone(),width:width.clone(),height:height.clone(),keyboard:keyboard.clone(),mouse:mouse.clone(),aim_enabled:aim_enabled.clone(),aim_button:aim_button.clone(),aim_mode:aim_mode.clone(),aim_x:aim_x.clone(),aim_y:aim_y.clone(),aim_sensitivity:aim_sensitivity.clone(),aim_slot:aim_slot.clone(),aim_invert_y:aim_invert_y.clone(),joy_enabled:joy_enabled.clone(),joy_up:joy_up.clone(),joy_down:joy_down.clone(),joy_left:joy_left.clone(),joy_right:joy_right.clone(),joy_x:joy_x.clone(),joy_y:joy_y.clone(),joy_radius:joy_radius.clone(),joy_slot:joy_slot.clone(),grab:grab.clone(),realtime:realtime.clone(), mouse_lock:mouse_lock.clone(),mouse_toggle:mouse_toggle.clone(),runtime_status:runtime_status.clone(),lock_status:lock_status.clone(),input_access:input_access.clone()};

    let root=GtkBox::new(Orientation::Vertical,0);
    let header=GtkBox::new(Orientation::Horizontal,8);add_margins(&header,8);
    let title=Label::new(Some("Waydroid Keymapper • Shooter Control Editor"));title.add_css_class("title-2");title.set_hexpand(true);title.set_halign(gtk4::Align::Start);
    let new_btn=Button::with_label("New");let dup_btn=Button::with_label("Duplicate");let del_btn=Button::with_label("Delete");
    let validate=Button::with_label("Validate");let save=Button::with_label("Save");let apply=Button::with_label("Apply & Run");
    header.append(&title);header.append(&new_btn);header.append(&dup_btn);header.append(&del_btn);header.append(&validate);header.append(&save);header.append(&apply);
    root.append(&header);root.append(&Separator::new(Orientation::Horizontal));

    let paned=Paned::new(Orientation::Horizontal);paned.set_wide_handle(true);
    let left=GtkBox::new(Orientation::Vertical,6);add_margins(&left,8);
    left.append(&Label::new(Some("Profiles")));
    let profile_scroll=ScrolledWindow::new();profile_scroll.set_policy(PolicyType::Never,PolicyType::Automatic);profile_scroll.set_child(Some(&profile_list));profile_scroll.set_min_content_width(220);profile_scroll.set_vexpand(true);left.append(&profile_scroll);
    let addbar=GtkBox::new(Orientation::Horizontal,5);
    let add_key_tap=Button::with_label("+ Key TAP");
    let add_key_hold=Button::with_label("+ Key HOLD");
    let add_mouse_tap=Button::with_label("+ Mouse TAP");
    let add_mouse_hold=Button::with_label("+ Mouse HOLD");
    addbar.append(&add_key_tap);addbar.append(&add_key_hold);
    addbar.append(&add_mouse_tap);addbar.append(&add_mouse_hold);
    left.append(&addbar);
    let bindings_scroll=ScrolledWindow::new();bindings_scroll.set_policy(PolicyType::Never,PolicyType::Automatic);
    bindings_scroll.set_child(Some(&bindings_box));bindings_scroll.set_vexpand(true);bindings_scroll.set_min_content_height(240);
    left.append(&bindings_scroll);

    let center=GtkBox::new(Orientation::Vertical,0);center.append(&canvas);center.append(&status);
    let right=GtkBox::new(Orientation::Vertical,4);add_margins(&right,8);
    let settings_scroll=ScrolledWindow::new();settings_scroll.set_policy(PolicyType::Never,PolicyType::Automatic);
    settings_scroll.set_child(Some(&right));settings_scroll.set_vexpand(true);settings_scroll.set_min_content_width(340);

    let presets=add_section(&right,"Shooter Presets");
    let preset_help=Label::new(Some("Optimized starting layouts for Free Fire and FPS/BR games. Coordinates are intentionally editable."));
    preset_help.set_wrap(true);preset_help.set_halign(gtk4::Align::Start);presets.append(&preset_help);
    let preset_bar=GtkBox::new(Orientation::Horizontal,5);
    let free_fire_btn=Button::with_label("Free Fire");
    let fps_btn=Button::with_label("FPS / BR");
    let minimal_btn=Button::with_label("Minimal");
    preset_bar.append(&free_fire_btn);preset_bar.append(&fps_btn);preset_bar.append(&minimal_btn);
    presets.append(&preset_bar);

    let general=add_section(&right,"Profile / Display");
    let g=Grid::new();g.set_row_spacing(7);g.set_column_spacing(8);
    form_row(&g,0,"Profile",&profile_name);form_row(&g,1,"Width",&width);form_row(&g,2,"Height",&height);
    general.append(&g);

    let devices=add_section(&right,"Input devices");
    let dg=Grid::new();dg.set_row_spacing(7);dg.set_column_spacing(8);
    form_row(&dg,0,"Keyboard",&keyboard);form_row(&dg,1,"Mouse",&mouse);devices.append(&dg);
    let refresh_dev=Button::with_label("Refresh devices");
    let fix_input=Button::with_label("🔑 Repair input permissions");
    devices.append(&refresh_dev);devices.append(&fix_input);
    devices.append(&input_access);

    let aim=add_section(&right,"Aim");
    aim.append(&aim_enabled);
    let ag=Grid::new();ag.set_row_spacing(7);ag.set_column_spacing(8);
    form_row(&ag,0,"Button",&aim_button);form_row(&ag,1,"Mode",&aim_mode);form_row(&ag,2,"Center X",&aim_x);form_row(&ag,3,"Center Y",&aim_y);
    form_row(&ag,4,"Sensitivity",&aim_sensitivity);form_row(&ag,5,"Slot",&aim_slot);
    ag.attach(&aim_invert_y,1,6,1,1);aim.append(&ag);

    let joystick=add_section(&right,"Joystick");
    joystick.append(&joy_enabled);
    let jg=Grid::new();jg.set_row_spacing(7);jg.set_column_spacing(8);
    form_row(&jg,0,"Up",&joy_up);form_row(&jg,1,"Down",&joy_down);form_row(&jg,2,"Left",&joy_left);form_row(&jg,3,"Right",&joy_right);
    form_row(&jg,4,"Center X",&joy_x);form_row(&jg,5,"Center Y",&joy_y);form_row(&jg,6,"Radius",&joy_radius);form_row(&jg,7,"Slot",&joy_slot);
    joystick.append(&jg);

    let perf=add_section(&right,"Performance");perf.append(&grab);perf.append(&realtime);perf.append(&mouse_lock);
    let mg=Grid::new();mg.set_row_spacing(7);mg.set_column_spacing(8);
    let toggle_box=GtkBox::new(Orientation::Horizontal,5);
    let capture_toggle=Button::with_label("Capture");
    toggle_box.append(&mouse_toggle);toggle_box.append(&capture_toggle);
    form_row(&mg,0,"Lock toggle key",&toggle_box);perf.append(&mg);
    let help=Label::new(Some("The toggle key is reserved for mouse capture and cannot also be a gameplay binding."));
    help.set_wrap(true);help.set_halign(gtk4::Align::Start);perf.append(&help);

    let runtime=add_section(&right,"Runtime");
    runtime.append(&runtime_status);runtime.append(&lock_status);
    let rb1=GtkBox::new(Orientation::Horizontal,5);
    let install_btn=Button::with_label("Install / Repair");
    let start_btn=Button::with_label("Start");
    let stop_btn=Button::with_label("Stop");
    let restart_btn=Button::with_label("Restart");
    let enable_btn=Button::with_label("Enable at login");
    let disable_btn=Button::with_label("Disable at login");
    let diagnostics_btn=Button::with_label("🔍 Diagnostics");
    rb1.append(&install_btn);rb1.append(&start_btn);rb1.append(&stop_btn);rb1.append(&restart_btn);runtime.append(&rb1);
    let rb0=GtkBox::new(Orientation::Horizontal,5);
    rb0.append(&enable_btn);rb0.append(&disable_btn);runtime.append(&rb0);
    runtime.append(&diagnostics_btn);
    let rb2=GtkBox::new(Orientation::Horizontal,5);
    let lock_btn=Button::with_label("🔒 Lock");
    let unlock_btn=Button::with_label("🖱 Unlock");
    let toggle_btn=Button::with_label("Toggle");
    rb2.append(&lock_btn);rb2.append(&unlock_btn);rb2.append(&toggle_btn);runtime.append(&rb2);
    let waydroid_state_label=Label::new(Some(&format!("Waydroid: {}",waydroid_state())));
    waydroid_state_label.set_halign(gtk4::Align::Start);runtime.append(&waydroid_state_label);
    let wb=GtkBox::new(Orientation::Horizontal,5);
    let waydroid_start=Button::with_label("Start Waydroid");
    let waydroid_stop=Button::with_label("Stop Waydroid");
    wb.append(&waydroid_start);wb.append(&waydroid_stop);runtime.append(&wb);

    paned.set_start_child(Some(&left));paned.set_resize_start_child(true);paned.set_shrink_start_child(false);
    paned.set_end_child(Some(&center));paned.set_resize_end_child(true);
    root.append(&paned);

    // The right panel is added as an overlay-like third pane through a secondary Paned.
    let main_paned=Paned::new(Orientation::Horizontal);
    main_paned.set_start_child(Some(&paned));
    main_paned.set_end_child(Some(&settings_scroll));
    main_paned.set_position(1200);
    root.remove(&paned);root.append(&main_paned);

    let app_window=ApplicationWindow::builder().application(app).title("Waydroid Keymapper").default_width(1550).default_height(900).build();
    app_window.set_child(Some(&root));

    canvas.set_draw_func({
        let st=state.clone();
        move |area,cr,w,h|draw_canvas(&st,area,cr,w,h)
    });

    let click=GestureClick::new();
    let ui2=ui.clone();
    click.connect_released(move |_,_,x,y|{
        let st=ui2.state.borrow();
        let pad=16.;let cw=(ui2.canvas.width() as f64-2.*pad).max(10.);let ch=(ui2.canvas.height() as f64-2.*pad).max(10.);
        let scale=(cw/st.cfg.display.width.max(1) as f64).min(ch/st.cfg.display.height.max(1) as f64);
        let vw=st.cfg.display.width as f64*scale;let vh=st.cfg.display.height as f64*scale;
        let ox=(ui2.canvas.width() as f64-vw)/2.;let oy=(ui2.canvas.height() as f64-vh)/2.;
        let nx=((x-ox)/vw).clamp(0.,1.) as f32;let ny=((y-oy)/vh).clamp(0.,1.) as f32;
        let sel=nearest_binding(&st.cfg,nx,ny);drop(st);
        if let Some(s)=sel{select_binding(&ui2,s);}
    });
    canvas.add_controller(click);

    let drag=GestureDrag::new();
    let drag_sel=Rc::new(Cell::new(None::<BindingRef>));
    let start=Rc::new(Cell::new((0.,0.)));
    {
        let ui2=ui.clone();let ds=drag_sel.clone();let ss=start.clone();
        drag.connect_drag_begin(move |_,x,y|{
            let st=ui2.state.borrow();
            let pad=16.;let cw=(ui2.canvas.width() as f64-2.*pad).max(10.);let ch=(ui2.canvas.height() as f64-2.*pad).max(10.);
            let scale=(cw/st.cfg.display.width.max(1) as f64).min(ch/st.cfg.display.height.max(1) as f64);
            let vw=st.cfg.display.width as f64*scale;let vh=st.cfg.display.height as f64*scale;
            let ox=(ui2.canvas.width() as f64-vw)/2.;let oy=(ui2.canvas.height() as f64-vh)/2.;
            let nx=((x-ox)/vw).clamp(0.,1.) as f32;let ny=((y-oy)/vh).clamp(0.,1.) as f32;
            let sel=nearest_binding(&st.cfg,nx,ny);ds.set(sel);ss.set((nx as f64,ny as f64));
        });
    }
    {
        let ui2=ui.clone();let ds=drag_sel.clone();let ss=start.clone();
        drag.connect_drag_update(move |_,dx,dy|{
            let Some(sel)=ds.get() else{return};
            let st=ui2.state.borrow();
            let pad=16.;let cw=(ui2.canvas.width() as f64-2.*pad).max(10.);let ch=(ui2.canvas.height() as f64-2.*pad).max(10.);
            let scale=(cw/st.cfg.display.width.max(1) as f64).min(ch/st.cfg.display.height.max(1) as f64);
            let vw=st.cfg.display.width as f64*scale;let vh=st.cfg.display.height as f64*scale;
            let (sx,sy)=ss.get();
            drop(st);
            let nx=clamp(sx+dx/vw);let ny=clamp(sy+dy/vh);
            let mut state=ui2.state.borrow_mut();
            set_selected_position(&mut state.cfg,sel,nx,ny);
            state.dirty=true;
            state.selected=Some(sel);
            ui2.canvas.queue_draw();
        });
        let ds2=drag_sel.clone();
        drag.connect_drag_end(move |_,_,_|ds2.set(None));
    }
    canvas.add_controller(drag);

    profile_list.connect_row_selected({
        let ui2=ui.clone();
        move |_,row|if let Some(r)=row{load_selected_profile(&ui2,r);}
    });

    let initial=profile_files();
    for p in initial{
        let row=ListBoxRow::new();let label=Label::new(Some(&display_name(&p)));label.set_xalign(0.);add_margins(&label,8);row.set_child(Some(&label));profile_list.append(&row);
        if p==ui.state.borrow().profile_path{profile_list.select_row(Some(&row));}
    }
    rebuild_bindings(&ui);sync_form(&ui);
    if let Some(row)=profile_list.selected_row(){row.grab_focus();}

    attach_key_capture(&mouse_toggle,&capture_toggle,&status);

    let ui2=ui.clone();enable_btn.connect_clicked(move |_|{
        match service_action("enable"){Ok(_)=>set_status(&ui2,"Daemon autostart enabled ✓"),Err(e)=>set_status(&ui2,&format!("Enable failed: {e}"))}
        update_runtime_status(&ui2);
    });
    let ui2=ui.clone();disable_btn.connect_clicked(move |_|{
        match service_action("disable"){Ok(_)=>set_status(&ui2,"Daemon autostart disabled"),Err(e)=>set_status(&ui2,&format!("Disable failed: {e}"))}
        update_runtime_status(&ui2);
    });
    let ui2=ui.clone();diagnostics_btn.connect_clicked(move |_|diagnostics(&ui2));

    let ui2=ui.clone();install_btn.connect_clicked(move |_|{
        match install_runtime(){Ok(())=>set_status(&ui2,"Runtime installed/repaired ✓"),Err(e)=>set_status(&ui2,&format!("Runtime setup failed: {e}"))}
        update_runtime_status(&ui2);
    });
    let ui2=ui.clone();start_btn.connect_clicked(move |_|{
        match service_action("start"){Ok(_)=>set_status(&ui2,"Daemon service started ✓"),Err(e)=>set_status(&ui2,&format!("Start failed: {e}"))}
        update_runtime_status(&ui2);
    });
    let ui2=ui.clone();stop_btn.connect_clicked(move |_|{
        match service_action("stop"){Ok(_)=>set_status(&ui2,"Daemon service stopped"),Err(e)=>set_status(&ui2,&format!("Stop failed: {e}"))}
        update_runtime_status(&ui2);
    });
    let ui2=ui.clone();restart_btn.connect_clicked(move |_|{
        match service_action("restart"){Ok(_)=>set_status(&ui2,"Daemon service restarted ✓"),Err(e)=>set_status(&ui2,&format!("Restart failed: {e}"))}
        update_runtime_status(&ui2);
    });
    let ui2=ui.clone();lock_btn.connect_clicked(move |_|runtime_control(&ui2,"lock"));
    let ui2=ui.clone();unlock_btn.connect_clicked(move |_|runtime_control(&ui2,"unlock"));
    let ui2=ui.clone();toggle_btn.connect_clicked(move |_|runtime_control(&ui2,"toggle"));
    let ui2=ui.clone();waydroid_start.connect_clicked(move |_|waydroid_action(&ui2,"start"));
    let ui2=ui.clone();waydroid_stop.connect_clicked(move |_|waydroid_action(&ui2,"stop"));

    {
        let ui2=ui.clone();
        glib::timeout_add_local(Duration::from_millis(700),move||{
            update_runtime_status(&ui2);
            glib::ControlFlow::Continue
        });
    }
    {
        let label=waydroid_state_label.clone();
        glib::timeout_add_local(Duration::from_secs(2),move||{
            label.set_text(&format!("Waydroid: {}",waydroid_state()));
            glib::ControlFlow::Continue
        });
    }

    let ui2=ui.clone();let win_new=app_window.clone();
    new_btn.connect_clicked(move |_|new_profile(&ui2,&win_new));
    let ui2=ui.clone();let win_dup=app_window.clone();
    dup_btn.connect_clicked(move |_|duplicate_profile(&ui2,&win_dup));
    let ui2=ui.clone();del_btn.connect_clicked(move |_|delete_profile(&ui2));
    let ui2=ui.clone();save.connect_clicked(move |_|match save_current(&ui2){Ok(())=>{rebuild_profiles(&ui2);set_status(&ui2,"Saved ✓")},Err(e)=>set_status(&ui2,&format!("Save failed: {e}"))});
    let ui2=ui.clone();apply.connect_clicked(move |_|apply_and_run(&ui2));
    let ui2=ui.clone();validate.connect_clicked(move |_|validate_current(&ui2));
    let ui2=ui.clone();fix_input.connect_clicked(move |_|{
        match install_input_permissions(){
            Ok(msg)=>set_status(&ui2,&msg),
            Err(e)=>set_status(&ui2,&format!("Input permission repair failed: {e}")),
        }
        update_runtime_status(&ui2);
    });

    let ui2=ui.clone();refresh_dev.connect_clicked(move |_|{
        let cfg=ui2.state.borrow().cfg.clone();
        fill_devices(&ui2.keyboard,&cfg.devices.keyboard,false);
        fill_devices(&ui2.mouse,&cfg.devices.mouse,true);
        set_status(&ui2,"Input devices refreshed");
    });

    let ui2=ui.clone();free_fire_btn.connect_clicked(move |_|apply_preset(&ui2,ShooterPreset::FreeFire));
    let ui2=ui.clone();fps_btn.connect_clicked(move |_|apply_preset(&ui2,ShooterPreset::Fps));
    let ui2=ui.clone();minimal_btn.connect_clicked(move |_|apply_preset(&ui2,ShooterPreset::Minimal));

    let ui2=ui.clone();add_key_tap.connect_clicked(move |_|open_binding_dialog_inner(&ui2,None,EditType::KeyboardTap));
    let ui2=ui.clone();add_key_hold.connect_clicked(move |_|open_binding_dialog_inner(&ui2,None,EditType::KeyboardHold));
    let ui2=ui.clone();add_mouse_tap.connect_clicked(move |_|open_binding_dialog_inner(&ui2,None,EditType::MouseTap));
    let ui2=ui.clone();add_mouse_hold.connect_clicked(move |_|open_binding_dialog_inner(&ui2,None,EditType::MouseHold));

    app_window.present();
}

fn main(){
    let app=Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}
