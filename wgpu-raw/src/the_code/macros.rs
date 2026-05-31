#[macro_export]
macro_rules! into_wgsl {
    (
        $(#[$meta: meta])*
        $vis:vis
        struct
        $name:ident

        { $($field_vis:vis $field_name:ident : $field_type:ty ),* $(,)? }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $(
                $field_vis $field_name: $field_type,
            )*
        }

        impl $name {
            #[allow(non_snake_case)]
            #[allow(static_mut_refs)]
            $vis fn WGSL() -> &'static str {
                static THE_WGSL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

                let str_ref = THE_WGSL.get_or_init(|| {
                    let mut output = <String as std::str::FromStr>::from_str(concat!("struct ", stringify!($name), " {\n")).unwrap();
                    let f = [
                        $(
                            (stringify!($field_name), stringify!($field_type)),
                        )*
                    ];
                    for (name, ty) in f {
                        output.push('\t');
                        output.push_str(name);
                        output.push_str(": ");
                        match ty {
                            "Vec2" | "glam::Vec2" => output.push_str("vec2<f32>"),
                            "Vec3" | "glam::Vec3" => output.push_str("vec3<f32>"),
                            "Vec4" | "glam::Vec4" => output.push_str("vec4<f32>"),
                            "[f32; 2]" => output.push_str("vec2<f32>"),
                            "[f32; 3]" => output.push_str("vec3<f32>"),
                            "[f32; 4]" => output.push_str("vec4<f32>"),

                            "IVec2" | "glam::IVec2" => output.push_str("vec2<i32>"),
                            "IVec3" | "glam::IVec3" => output.push_str("vec3<i32>"),
                            "IVec4" | "glam::IVec4" => output.push_str("vec4<i32>"),
                            "[i32; 2]" => output.push_str("vec2<i32>"),
                            "[i32; 3]" => output.push_str("vec3<i32>"),
                            "[i32; 4]" => output.push_str("vec4<i32>"),

                            "UVec2" | "glam::UVec2" => output.push_str("vec2<u32>"),
                            "UVec3" | "glam::UVec3" => output.push_str("vec3<u32>"),
                            "UVec4" | "glam::UVec4" => output.push_str("vec4<u32>"),
                            "[u32; 2]" => output.push_str("vec2<u32>"),
                            "[u32; 3]" => output.push_str("vec3<u32>"),
                            "[u32; 4]" => output.push_str("vec4<u32>"),

                            "Vec<Vec2>" => output.push_str("array<vec2<f32>>"),
                            "Vec<Vec3>" => output.push_str("array<vec3<f32>>"),
                            "glam::Vec<Vec2>" => output.push_str("array<vec2<f32>>"),
                            "glam::Vec<Vec3>" => output.push_str("array<vec3<f32>>"),

                            "Vec<f32>" => output.push_str("array<f32>"),
                            "Vec<i32>" => output.push_str("array<i32>"),
                            "Vec<u32>" => output.push_str("array<u32>"),

                            &_ => output.push_str(ty),
                        }
                        output.push_str(",\n");
                    }
                    output.push_str("}");

                    output
                });

                unsafe { std::mem::transmute::<&str, &'static str>(str_ref.as_str()) }
            }
        }
    };
}

#[macro_export]
macro_rules! enum_iter {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $variant:ident $( ( $($the_type:ty => $def:expr $(,)?)* ) )? ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $( $variant $( ( $($the_type,)* ) )? ),*
        }

        impl $name {
            $vis fn iterate_def() -> &'static [$name] {
                &[
                    $(
                        $name::$variant $( ( $($def)*, ) )?,
                    )*
                ]
            }

            $vis fn name(&self) -> &'static str {
                match self {
                    $(
                        $name::$variant $( ( enum_iter!(@replace $($the_type)*) ) )? => stringify!($variant),
                    )*
                }
            }
        }
    };

    (@replace $($tt:tt)*) => { .. };
}