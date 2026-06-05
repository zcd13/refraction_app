#![allow(unused_assignments)]

mod polygon;

use std::f32::consts::PI;
use std::time::Instant;
use macroquad::miniquad::gl;
use macroquad::prelude::*;
use crate::polygon::{Geometry};

fn to_screen(logic: impl Into<Vec2>) -> Vec2 {
    let logic = logic.into();
    let w = screen_width();
    let h = screen_height();

    let min_dim = w.min(h);

    Vec2 {
        x: (w / 2.0) + (logic.x * (min_dim / 2.0)),
        y: (h / 2.0) - (logic.y * (min_dim / 2.0)),
    }
}

fn to_logical(screen: impl Into<Vec2>) -> Vec2 {
    let screen = screen.into();
    let w = screen_width();
    let h = screen_height();

    let min_dim = w.min(h);

    Vec2 {
        x: (screen.x - (w / 2.0)) / (min_dim / 2.0),
        y: ((h / 2.0) - screen.y) / (min_dim / 2.0),
    }
}

#[macroquad::main("Formaldehyde")]
async fn main() {
    light_bounce_test().await;
}

#[allow(dead_code)]
async fn testing_sim() {
    let t = Instant::now();
    let poi = Vec2::new(to_logical(Vec2::ZERO).x, 0.0);
    let mut pd = Vec2::ZERO;

    let mut test_poly = Geometry::circle_poly(6)
        .with_scale(0.5);

    let mut subs = vec![test_poly.clone(); 10];

    loop {
        clear_background(BLACK);
        let mp = to_logical(mouse_position());

        pd = (mp - poi).normalize();

        line(poi, poi + pd * 100.0, 3.0, WHITE);
        {
            let st = Vec2::splat(-0.75);
            let ed = st + pd * 0.15;
            line(st, ed, 3.0, RED);
        }

        test_poly.draw_outline(2.0, RED);
        test_poly.fan_shaded(GOLD.with_alpha(0.5));
        let intersect = test_poly.intersect(poi, pd);
        if let Some((p, inside, normal)) = intersect {
            let inside = inside % 2 == 0;
            if inside {
                circle(p, 5.0, YELLOW);
            } else {
                circle(p, 5.0, GREEN);
            }

            line(p, p + normal * 0.25, 4.0, BLUE);

            let rel = reflect(pd, normal);
            line(p, p + rel * 0.25, 4.0, RED);
        }

        let sin = (t.elapsed().as_secs_f32() * 0.5).sin().abs();
        let amt = 3 + (10.0 * sin) as usize;
        test_poly = Geometry::circle_poly(amt)
            .with_scale(0.5)
            .with_rot(t.elapsed().as_secs_f32() * 0.2);

        for i in 0..subs.len() {
            let f = i as f32 / subs.len() as f32;
            let scale = 0.5 * f;
            subs[i] = Geometry::circle_poly(amt)
                .with_scale(scale)
                .with_rot(t.elapsed().as_secs_f32() * 0.2);
            subs[i].fan_shaded(GOLD.with_alpha(f))
        }

        // ... (rest of the sim drawing code remains exactly the same)
        circle(poi, 5.0, WHITE);
        draw_text("Logic Space: (-1.0, -1.0) to (1.0, 1.0)", 20.0, 30.0, 20.0, GRAY);
        next_frame().await
    }
}


const AIR: f32 = 1.0003;
const GLASS: f32 = 1.5;
const RED_LIGHT: f32 = 650.0;

fn get_index_of_refraction(wavelength_nm: f32) -> f32 {
    let wavelength_microns = wavelength_nm / 1000.0;
    // Typical coefficients for crown glass
    let a = 1.517;
    let b = 0.0042;
    // n = A + B / λ²
    a + (b / wavelength_microns.powi(2))
}

#[allow(dead_code)]
async fn light_sim_test() {
    let mut poi = Vec2::new(to_logical(Vec2::ZERO).x, 0.0);
    let mut pd = Vec2::ZERO;

    let mut wavelength = RED_LIGHT;

    let poly = Geometry::right_tri()
        .with_scale(0.5);

    loop {
        clear_background(BLACK);
        let mp = to_logical(mouse_position());
        pd = (mp - poi).normalize();

        // pd line
        line(poi, poi + pd * 100.0, 2.0, WHITE.with_alpha(0.25));

        // debug line
        let st = Vec2::splat(-0.75);
        let ed = st + pd * 0.15;
        line(st, ed, 3.0, RED);

        // polygon
        poly.draw_outline(2.0, WHITE.with_alpha(0.5));


        if let Some((hit_pos, hit_count, normal)) = poly.intersect(poi, pd) {
            let n_glass = get_index_of_refraction(wavelength);
            // 1. Determine indices based on entry/exit
            let (n1, n2) = match hit_count % 2 == 0 {
                true => (AIR, n_glass), // AIR to GLASS
                false => (n_glass, AIR), // GLASS to AIR
            };

            let hit = do_reflect_refract(hit_pos, pd, normal, n1, n2);


            let aj_hit_pos = hit_pos + hit.refraction * 0.0001;
            if let Some((hit_pos, _, normal)) = poly.intersect(aj_hit_pos, hit.refraction) {
                let (n1, n2) = (n_glass, AIR); // GLASS to AIR

                let _hit = do_reflect_refract(hit_pos, hit.refraction, normal, n1, n2);
            }
        }


        if is_mouse_button_down(MouseButton::Left) {
            poi = mp;
        }
        wavelength += mouse_wheel().1 * 0.05;
        draw_text(&format!("Wavelength {wavelength}"), 100.0, 100.0, 50.0, WHITE);

        next_frame().await;
    }
}

fn set_additive_blending() {
    unsafe {
        gl::glEnable(gl::GL_BLEND);
        // Factor GL_ONE, GL_ONE means: (New Color * 1) + (Old Color * 1)
        // This is pure additive blending.
        gl::glBlendFunc(gl::GL_ONE, gl::GL_ONE);
    }
}

fn restore_default_blending() {
    unsafe {
        // Restore standard Alpha Blending
        gl::glBlendFunc(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
    }

}


const LINE_THICK: f32 = 0.5;
const MAX_BOUNCE: u32 = 300000;
const NO_LAZERS: usize = 5000;
const OFFSET: f32 = 0.00001;
const DISPENSER: f32 = 0.03;
const CUTOFF: f32 = 0.000005;
async fn light_bounce_test() {
    let mut poi = Vec2::new(to_logical(Vec2::ZERO).x, 0.0);
    let mut pd = Vec2::ZERO;

    // let poly = Geometry::circle_poly(40)
    //     .with_scale(0.5);
    
    let poly = Geometry::circle_poly(15)
        .with_scale(0.5);


    loop {
        clear_background(BLACK);

        set_additive_blending();

        struct Lazer {
            point: Vec2,
            dir: Vec2,
            strength: f32,
            wavelength: f32,
            inside: bool,
            bounces: u32,
        }
        let mut lazers = Vec::with_capacity(NO_LAZERS);
        let is_inside = poly.is_inside(poi);
        for i in 0..NO_LAZERS {
            let f = i as f32 / NO_LAZERS as f32;
            let vis = 380.0 + ((720.0 - 380.0) * f);

            let d_f = f - 0.5;
            let dir_rad = pd.y.atan2(pd.x) + d_f * DISPENSER;
            let new_dir = Vec2::new(dir_rad.cos(), dir_rad.sin());

            lazers.push(Lazer {
                point: poi,
                dir: new_dir,
                strength: 0.1,
                wavelength: vis,
                inside: is_inside,
                bounces: 0,
            });
        }

        'graphics: loop {
            // busy code
            let mut tba: Vec<Lazer> = vec![];

            for l in lazers.iter() {
                if let Some((hit_point, _, normal)) = poly.intersect(l.point, l.dir) {
                    line(l.point, hit_point, LINE_THICK, wavelength_to_rgb(l.wavelength).with_alpha(l.strength));

                    let n_glass = get_index_of_refraction(l.wavelength);
                    let (n1, n2) = match l.inside {
                        true => (n_glass, AIR), // GLASS to AIR
                        false => (AIR, n_glass), // AIR to GLASS
                    };

                    let hit = do_reflect_refract(hit_point, l.dir, normal, n1, n2);

                    // REFRACTION
                    tba.push(Lazer {
                        point: hit_point + hit.refraction * OFFSET,
                        dir: hit.refraction,
                        strength: l.strength * hit.refraction_fact,
                        wavelength: l.wavelength,
                        // If it refracts, it switches sides: Air -> Glass or Glass -> Air
                        inside: !l.inside,
                        bounces: l.bounces + 1,
                    });

                    // REFLECTION
                    tba.push(Lazer {
                        point: hit_point + hit.reflection * OFFSET,
                        dir: hit.reflection,
                        strength: l.strength * hit.reflection_fact,
                        wavelength: l.wavelength,
                        // If it reflects, it stays on the side it's currently on
                        inside: l.inside,
                        bounces: l.bounces + 1,
                    });

                } else {
                    line(l.point, l.point + l.dir * 100.0, LINE_THICK, wavelength_to_rgb(l.wavelength).with_alpha(l.strength));
                }
            }

            std::mem::swap(&mut tba, &mut lazers);
            tba.clear();

            lazers.retain(|f| f.strength > CUTOFF && f.bounces < MAX_BOUNCE);
            if lazers.is_empty() {
                break 'graphics;
            }
        }

        unsafe { get_internal_gl().flush(); }
        restore_default_blending();


        let mp = to_logical(mouse_position());
        pd = (mp - poi).normalize();

        // debug line
        let st = Vec2::splat(-0.75);
        let ed = st + pd * 0.15;
        line(st, ed, 3.0, RED);

        // polygon
        poly.draw_outline(2.0, WHITE.with_alpha(0.5));
        // s.draw_outline(5.0, RED.with_alpha(0.5));

        if is_mouse_button_down(MouseButton::Left) {
            poi = mp;
        }

        next_frame().await;
    }

}


struct ReRfCalc {
    reflection: Vec2,
    reflection_fact: f32,

    refraction: Vec2,
    refraction_fact: f32,
}

fn do_reflect_refract(hit_pos: Vec2, pd: Vec2, mut normal: Vec2, n1: f32, n2: f32) -> ReRfCalc {
    if normal.dot(pd) > 0.0 {
        normal = -normal;
    }

    let ratio: f32 = n1 / n2;

    // 2. Calculate c1 (cos of incident angle)
    // We use -pd because the dot product needs the vector pointing TOWARD the normal
    let c1 = -normal.dot(pd);

    // 3. Calculate c2 (cos of refracted angle) using Snell's Law in terms of cosines
    // Check for Total Internal Reflection (TIR) - radicand must be >= 0
    let radicand = 1.0 - ratio.powi(2) * (1.0 - c1.powi(2));

    if radicand >= 0.0 {
        let c2 = radicand.sqrt();

        // 4. Construct the refracted vector
        // T = (ratio * I) + (ratio * c1 - c2) * N
        let refraction = (ratio * pd) + (ratio * c1 - c2) * normal;
        let reflection = pd + 2.0 * c1 * normal;

        // Draw the refracted ray
        // line(hit_pos, hit_pos + refract * 100.0, 2.0, RED);

        // fresnel calc
        let rs = (n1 * c1 - n2 * c2) / (n1 * c1 + n2 * c2);
        let rs_reflectance = rs.powi(2);

        let rp = (n2 * c1 - n1 * c2) / (n2 * c1 + n1 * c2);
        let rp_reflectance = rp.powi(2);

        let reflection_fact = (rs_reflectance + rp_reflectance) / 2.0;
        let refraction_fact = 1.0 - reflection_fact;

        // let rat = format!("REFLECT: {reflection_fact:.2} REFRACT: {refraction_fact:.2}");
        // let screen_hit = to_screen(hit_pos);
        // draw_text(&rat, screen_hit.x, screen_hit.y - 20.0, 20.0, WHITE);
        //
        // line(hit_pos, hit_pos + refraction * 0.25, 5.0, WHITE.with_alpha(refraction_fact));
        // line(hit_pos, hit_pos + reflection * 0.25, 5.0, WHITE.with_alpha(reflection_fact));

        ReRfCalc {
            reflection,
            reflection_fact,
            refraction,
            refraction_fact,
        }

    } else {
        // Total Internal Reflection occurred (no refracted ray)
        // Optionally calculate and draw the reflection vector here:
        let reflect = pd + 2.0 * c1 * normal;
        // line(hit_pos, hit_pos + reflect * 100.0, 2.0, BLUE);

        ReRfCalc {
            reflection: reflect,
            reflection_fact: 1.0,
            refraction: Default::default(),
            refraction_fact: 0.0,
        }
    }
}

fn wavelength_to_rgb(wavelength: f32) -> Color {
    let mut r = 0.0;
    let mut g = 0.0;
    let mut b = 0.0;

    // 1. Calculate primary color intensities based on spectrum zones
    if wavelength >= 380.0 && wavelength < 440.0 {
        r = -(wavelength - 440.0) / (440.0 - 380.0);
        g = 0.0;
        b = 1.0;
    } else if wavelength >= 440.0 && wavelength < 490.0 {
        r = 0.0;
        g = (wavelength - 440.0) / (490.0 - 440.0);
        b = 1.0;
    } else if wavelength >= 490.0 && wavelength < 510.0 {
        r = 0.0;
        g = 1.0;
        b = -(wavelength - 510.0) / (510.0 - 490.0);
    } else if wavelength >= 510.0 && wavelength < 580.0 {
        r = (wavelength - 510.0) / (580.0 - 510.0);
        g = 1.0;
        b = 0.0;
    } else if wavelength >= 580.0 && wavelength < 645.0 {
        r = 1.0;
        g = -(wavelength - 645.0) / (645.0 - 580.0);
        b = 0.0;
    } else if wavelength >= 645.0 && wavelength <= 780.0 {
        r = 1.0;
        g = 0.0;
        b = 0.0;
    }

    // 2. Adjust intensity at the edges of the visible spectrum (fading)
    let factor = if wavelength >= 380.0 && wavelength < 420.0 {
        0.3 + 0.7 * (wavelength - 380.0) / (420.0 - 380.0)
    } else if wavelength >= 420.0 && wavelength <= 700.0 {
        1.0
    } else if wavelength > 700.0 && wavelength <= 780.0 {
        0.3 + 0.7 * (780.0 - wavelength) / (780.0 - 700.0)
    } else {
        0.0
    };

    // 3. Apply the intensity factor
    // Note: Gamma correction (usually 0.8) can be applied here if colors look too dark
    Color::new(r * factor, g * factor, b * factor, 1.0)
}

/// line logical
fn line(a: Vec2, b: Vec2, thick: f32, color: Color) {
    line_screen(to_screen(a), to_screen(b), thick, color)
}
fn line_screen(a: Vec2, b: Vec2, thick: f32, color: Color) {
    draw_line(a.x, a.y, b.x, b.y, thick, color);
}
fn circle(pos: Vec2, size: f32, color: Color) {
    screen_circle(to_screen(pos), size, color);
}

fn screen_circle(pos: Vec2, size: f32, col: Color) {
    let (x, y) = pos.into();
    draw_circle(x, y, size, col);
}

fn reflect(incident: Vec2, normal: Vec2) -> Vec2 {
    // R = D - 2 * (D dot N) * N
    incident - 2.0 * incident.dot(normal) * normal
}

