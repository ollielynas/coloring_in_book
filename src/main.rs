use egui::{Color32, Ui};
use jpeg_encoder::{Encoder, ColorType};
use macroquad::{prelude::*, input::KeyCode};

use std::{fs::{File, read_dir}, path::PathBuf, str::FromStr, io::Write, collections::HashMap, time::Instant, thread::JoinHandle};
use zip_extensions::*;

extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;



const SPEED: f32 = 160.0;

struct Drawing {
    og_image: Option<macroquad::texture::Texture2D>,
    new_image: Option<macroquad::texture::Texture2D>,

}

impl Drawing {



    async fn load(num: usize) -> Self {
        
        let name_list = include_str!("text.txt").lines().map(|f| f.to_owned()).collect::<Vec<String>>();
        
        
    
        let name = name_list[num].clone();
        let archive_file = PathBuf::from_str(r#"img/processed.zip"#).unwrap();
        let entry_path = PathBuf::from_str(&name).unwrap();
        
        if read_dir(format!("img/sketch_{num}")).is_err() {


        let mut buffer : Vec<u8> = vec![];
        match zip_extract_file_to_memory(&archive_file, &entry_path, &mut buffer) {
            Ok(()) => { println!("Extracted {} bytes from archive.", buffer.len()) },
            Err(e) => { println!("The entry does not exist. {e}") }
        };

    
    // new folder
    std::fs::create_dir_all(format!("img/sketch_{num}", )).unwrap();

    // wright buffer to file
    let mut file = File::create(format!("img/sketch_{num}/og_sketch_{num}.jpg", )).unwrap();
    file.write_all(&buffer).unwrap();
    let mut file = File::create(format!("img/sketch_{num}/new_sketch_{num}.jpg", )).unwrap();
    file.write_all(&buffer).unwrap();
    
    }

    // create folder

    let image = match macroquad::texture::load_texture(&format!("img/sketch_{num}/og_sketch_{num}.jpg")).await {
                Ok(tex) => Some(tex),
                Err(e) => panic!("Failed to load texture: {}", e),
            };
    let new_image = match macroquad::texture::load_texture(&format!("img/sketch_{num}/new_sketch_{num}.jpg")).await {
                Ok(tex) => Some(tex),
                Err(e) => panic!("Failed to load texture: {}", e),
    };

        
        Drawing { og_image: image, new_image
        
        
        }
    }


}

#[derive(Savefile)]
struct Pen {
    color_rgb: [u8; 3],

    #[savefile_introspect_ignore]
    #[savefile_ignore]
    color: Color32,
    size: f32,
    fade: f32,
    alpha: f32,
    inside_circle: bool,
    window_open: bool,
    outline: bool,
}

impl Default for Pen {
    fn default() -> Self {
        Pen {
            color: Color32::from_rgba_premultiplied(0, 255, 0, 255/4),
            color_rgb: [0, 255, 0],
            size: 5.0,
            fade: 0.0,
            alpha: 0.2,
            inside_circle: true,
            window_open: true,
            outline: true,
        }
    }
    
}

impl Pen {
    fn render_pen(&mut self, ui: &mut Ui) {

                
                ui.add(egui::Slider::new(&mut self.size, 0.0..=30.0).text("size"));
                ui.style_mut().visuals.extreme_bg_color = self.color;
                ui.add(egui::Slider::new(&mut self.alpha, 0.0..=0.25).trailing_fill(true)
                .text("alpha")).changed().then(|| {
                    self.color = Color32::from_rgba_premultiplied(self.color.r(), self.color.g(), self.color.b(), (self.alpha * 255.0) as u8);
                });
                ui.checkbox(&mut self.inside_circle, "inside circle");
                ui.checkbox(&mut self.outline, "outline");
                ui.color_edit_button_srgba(&mut self.color).changed().then(||{
                    self.alpha = self.color.a() as f32 / 255.0;
                });
                
    }

    fn macroquad_color(&self) -> macroquad::color::Color {
        macroquad::color::Color::new(self.color.r() as f32 / 255.0, self.color.g() as f32 / 255.0, self.color.b() as f32 / 255.0, self.alpha)
    }
}
#[derive(Savefile)]
struct Data {
    saved_pens: Vec<Pen>,
    names: HashMap<usize, String>,
    ui_scale: f32,
}

impl Data {
    fn new() -> Self {
        Data {
            ui_scale: 1.0,
            saved_pens: vec![Pen::default()],
            names: HashMap::new(),
        }
    }

    fn save(&mut self) {
        for pen in self.saved_pens.iter_mut() {
            pen.color_rgb = [pen.color.r(), pen.color.g(), pen.color.b()];
        }
        save_file("img/save.bin", 1, self);
    }

    fn load() -> Self {
        let mut save = Data::new();
        let load:Result<Data, SavefileError> = savefile::load_file("img/save.bin", 1);
        if let Ok(mut f) = load {
            for pen in f.saved_pens.iter_mut() {
                pen.color = Color32::from_rgba_premultiplied(pen.color_rgb[0], pen.color_rgb[1], pen.color_rgb[2], (pen.alpha * 255.0) as u8);
            }
            save = f;
        };

        return save;


    }
}


#[macroquad::main("Darwin's Coloring in Book")]
async fn main() {

    let mut save_data = Data::load();

    let mut drawing: Option<Drawing> = None;
    let mut pen = Pen::default();

    let mut og_data: Option<Image> = None;
    let mut new_data: Option<Image> = None;
    let mut offset = egui::Vec2::ZERO;
    let mut drawing_list: Vec<usize> = vec![];
    let mut rect = egui::Rect::NOTHING;
    let mut zoom = 1.0;
    let mut delta = Instant::now();


    let mut new_from_index: Option<usize> = None;


    let mut save = Instant::now();

    let mut saved_thread: Option<JoinHandle<()>> = None;

    let mut mouse_pos = egui::Vec2::ZERO;

    let mut selected_drawing = 9999;

    let mut og_pxpp = 0.0;
    
    // list dir
    if let Ok(dir) = read_dir("img") {
        for entry in dir {
            if let Ok(entry) = entry {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let name = entry.file_name();
                        let name = name.to_str().unwrap();
                        if name.starts_with("sketch_") {
                            let num = name.split("_").nth(1).unwrap().parse::<usize>().unwrap();
                            drawing_list.push(num);
                        }
                    }
                }
            }
        }
    }

    drawing_list.sort();
    drawing_list.dedup();

    


    egui_macroquad::ui(|egui_ctx| {

        og_pxpp = egui_ctx.pixels_per_point();
        egui_ctx.set_pixels_per_point(og_pxpp * save_data.ui_scale);
        egui_ctx.set_visuals(egui::Visuals::light());
    });
    
    let name_list = include_str!("text.txt").lines().map(|f| f.to_owned()).collect::<Vec<String>>();

    
    loop {
            let mut scroll_delta = 0.0;

        // prevent close
        
        macroquad::input::prevent_quit();
        if is_quit_requested() {
            if let Some( save_thread) = saved_thread {
                save_thread.join().unwrap();
            }
            break;
        }

        
        clear_background(WHITE);

        let delta_sec = delta.elapsed().as_secs_f32();
        delta = Instant::now();


        let mut hover = false;
        let mut draw_on_image = false;

        if save.elapsed().as_secs_f32() > 5.0 {
            println!("saving 1");
            println!("{}",new_data.is_some());

            save_data.save();

            if let Some(new_data) = &new_data {

                // spawn thread
            
                println!("saving 2");

                let new_data = new_data.clone();

                if let Some(ref mut save_thread2) = saved_thread {
                    if save_thread2.is_finished() {
                        saved_thread = None;
                        println!("saved 2");
                    }
                }
                if saved_thread.is_none() {
                saved_thread = Some(std::thread::spawn(move || {
                    // save image
                    println!("saving {selected_drawing}");
                    let path = format!("img/sketch_{num}/new_sketch_{num}.jpg", num=selected_drawing);
                    let raw = new_data.get_image_data().to_vec();
                    // convert [u8;4] to [u8;3]
                    let raw = raw.iter().map(|c| [c[0], c[1], c[2]]).flatten().collect::<Vec<u8>>();
                    
                    // jpeg from raw
                    let encoder = Encoder::new_file(path, 100);
                    if let Ok(encoder) = encoder {
                        if let Err(a) =  encoder.encode(&raw, new_data.width() as u16, new_data.height() as u16, ColorType::Rgb) {
                            println!("Error: {}", a);
                        }
                    };
                }));
            }



            }
            save = Instant::now();
        }


        

        





        egui_macroquad::ui(|egui_ctx| {
            
            
            
            
            

            let rect_sidebar = egui::SidePanel::left("side_panel").show(egui_ctx, |ui| {
                ui.heading("Placeholder Text");
                ui.hyperlink_to("BHL", "https://www.biodiversitylibrary.org/");
                ui.separator();

                pen.render_pen(ui);

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Scale:");
                    if ui.small_button("-").clicked() {
                        save_data.ui_scale *= 0.9;
                        egui_ctx.set_pixels_per_point(og_pxpp * save_data.ui_scale);
                    }
                    if ui.small_button("+").clicked() {
                        save_data.ui_scale *= 1.1;
                        egui_ctx.set_pixels_per_point(og_pxpp * save_data.ui_scale);
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Movement:");
                    ui.add_enabled(false, egui::Button::new("W").small());
                    ui.add_enabled(false, egui::Button::new("A").small());
                    ui.add_enabled(false, egui::Button::new("S").small());
                    ui.add_enabled(false, egui::Button::new("D").small());
                    ui.label("or");
                    ui.add_enabled(false, egui::Button::new("Arrow Keys").small());
                });
                ui.horizontal(|ui| {
                    ui.label("Zoom:");
                    ui.add_enabled(false, egui::Button::new("Ctrl +").small());
                    ui.add_enabled(false, egui::Button::new("Ctrl -").small());
                    ui.label("or");
                    ui.add_enabled(false, egui::Button::new("Ctrl Scroll").small());
                });
                ui.horizontal(|ui| {
                    ui.label("Brush Size:");
                    ui.add_enabled(false, egui::Button::new("Q +").small());
                    ui.add_enabled(false, egui::Button::new("Q -").small());
                    ui.label("or");
                    ui.add_enabled(false, egui::Button::new("Q Scroll").small());
                });
            }).response.rect;

            let rect_sidebar_right = egui::SidePanel::right("right_side_panel").show(egui_ctx, |ui| {
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.collapsing("Current Pages", |ui| {
                    for i in drawing_list.iter() {
                            ui.horizontal(|ui| {
                            if selected_drawing == *i {
                                ui.label(">");
                            }
                            ui.label(format!("Pg. {i}"));
                            if drawing_list.contains(&i) {
                                if ui.button("Open").clicked() {
                                    new_from_index = Some(*i);
                                    
                                }
                            } else {
                                if ui.button("Unpack").clicked() {
                                    new_from_index = Some(*i);
                                }
                            }
                        });
                    }});
                    ui.collapsing("All Pages", |ui| {
                    for i in 0..name_list.len() {
                            ui.horizontal(|ui| {
                            if selected_drawing == i {
                                ui.label(">");
                            }
                            ui.label(format!("Pg. {i}"));
                            if drawing_list.contains(&i) {
                                if ui.button("Open").clicked() {
                                    new_from_index = Some(i);
                                    
                                }
                            } else {
                                if ui.button("Unpack").clicked() {
                                    new_from_index = Some(i);
                                }
                            }
                        });
                    }});
                });
            }).response.rect;
            
            
            rect = egui_ctx.screen_rect();
            rect.set_left(rect_sidebar.right());
            rect.set_right(rect_sidebar_right.left());
            rect = rect.expand2([rect.width()-rect.width()*zoom, rect.height()-rect.height()*zoom].into());
            rect = rect.translate(offset);

            
            hover = egui_ctx.is_pointer_over_area();
            draw_on_image = !egui_ctx.wants_pointer_input() && egui_ctx.input(
                |i| 
                {
                    scroll_delta = i.scroll_delta.y/100.0;
                i.pointer.button_down(egui::PointerButton::Primary)
                }
            );


            if !hover {
                egui_ctx.output_mut(|o| {o.cursor_icon = egui::CursorIcon::None});
            }
        });




        if let Some(n) = new_from_index {
            
            if let Some(draw) = drawing {
                if let Some(pic) = draw.new_image {
                    pic.delete();
                }
                if let Some(pic) = draw.og_image {
                    pic.delete();
                }
            }
            selected_drawing = n;
            drawing = Some(Drawing::load(selected_drawing).await);
            new_from_index = None;
            drawing_list.push(n);
            // remove duplicates
            drawing_list.sort();
            drawing_list.dedup();
        }





        
        if let Some(Drawing {new_image: Some(image), ..}) = drawing {

            let ratio = image.width() as f32 / image.height() as f32;

            

            let shrink = match ratio <1.0 {
                true => [(rect.width()-rect.height()*ratio)/2.0+20.0,20.0],
                
                false => [20.0, (rect.height()-rect.width()*(1.0/ratio))/2.0+ 20.0],
                
            };
            rect = rect.shrink2(shrink.into());

            draw_texture_ex(image, rect.left(), rect.top(), WHITE, DrawTextureParams {
                dest_size: Some(Vec2::new(rect.width(), rect.height())),
                ..Default::default()
            });

            let mut mouse_pos = mouse_position();
            mouse_pos.0 -= rect.left();
            mouse_pos.1 -= rect.top();

            // detect mouse down

            if draw_on_image {
                
                
match og_data {
            Some(ref mut og_data) => {
                
                let mut mouse_pos = mouse_position();
                mouse_pos.0 -= rect.left();
                mouse_pos.1 -= rect.top();

                if new_data.is_none() {
                    println!("new data");
                    new_data = Some(image.get_texture_data());
                }

                if let Some(ref mut new_data2) = new_data {
                    let scale = new_data2.width() as f32 / rect.width();
                mouse_pos.0 *= scale;
                mouse_pos.1 *= new_data2.height() as f32 / rect.height();
                
                let pen_rect = egui::Rect::from_min_size([mouse_pos.0-pen.size*scale, mouse_pos.1-pen.size*scale].into(), [pen.size*scale*2.0, pen.size*scale*2.0].into());

                for y in pen_rect.top().max(0.0) as usize..(pen_rect.bottom() as usize).min(new_data2.height()) {
                    for x in pen_rect.left().max(0.0) as usize..(pen_rect.right() as usize).min(new_data2.width()) {
                        
                        if (mouse_pos.0 - x as f32).powi(2) + (mouse_pos.1 - y as f32).powi(2) < (scale*pen.size).powi(2) {
                            let mut pixel = og_data.get_pixel(x as u32, y as u32);
                            pixel.r = pixel.r  * (1.0-pen.color.a() as f32/255.0) + pen.color.r() as f32/255.0 * pen.color.a()as f32/255.0;
                            pixel.g = pixel.g  * (1.0-pen.color.a() as f32/255.0) + pen.color.g() as f32/255.0 * pen.color.a()as f32/255.0;
                            pixel.b = pixel.b  * (1.0-pen.color.a() as f32/255.0) + pen.color.b() as f32/255.0 * pen.color.a()as f32/255.0;

                            new_data2.set_pixel(x as u32, y as u32, Color::new(pixel.r, pixel.g, pixel.b, 1.0));
                        }
                    }
                }

                image.update(&new_data2);
            }
            }
            
        
        None => {

            if let Some(new_og_data) =  drawing.as_ref().unwrap().og_image {
                og_data = Some(new_og_data.get_texture_data());
            };

            
        }
    }
                
}

}

        egui_macroquad::draw();

        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
            offset.x += SPEED*delta_sec;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            offset.x -= SPEED*delta_sec;
        }
        if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) {
            offset.y += SPEED*delta_sec;
        }
        if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
            offset.y -= SPEED*delta_sec;
        }

        
        if is_key_down(KeyCode::Equal) {
            scroll_delta -= 0.1*delta_sec;
        }
        if is_key_down(KeyCode::Minus) {
            scroll_delta += 0.1*delta_sec;
        }
        if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) {
            zoom += scroll_delta*2.0;
        }

        if is_key_down(KeyCode::Q) {
            pen.size = (pen.size - scroll_delta * 100.0).clamp(0.0, 30.0);
        }
        
        // Draw things after egui

        
        if !hover {
            
            if pen.outline {
                draw_circle_lines(mouse_position().0, mouse_position().1, pen.size, 1.0,GRAY);
            }
            
            macroquad::input::show_mouse(false);

            match pen.inside_circle {
                true => {
                    draw_circle(mouse_position().0, mouse_position().1, pen.size, pen.macroquad_color());
                },
                false => {
                    draw_circle_lines(mouse_position().0, mouse_position().1, pen.size, 3.0,pen.macroquad_color());
                }

            }
            
        }else {
            match pen.inside_circle {
                true => {
                    draw_circle(pen.size+20.0,screen_height()-pen.size-20.0, pen.size, pen.macroquad_color());
                },
                false => {
                    draw_circle_lines(pen.size+20.0,screen_height()-pen.size-20.0, pen.size, 3.0,pen.macroquad_color());
                }
            }
        }

        
        
        next_frame().await;
    }


}