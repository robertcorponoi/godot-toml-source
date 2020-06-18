use euclid::Size2D;
use gdnative::{
	methods, Basis, Color, Dictionary, File, GodotString, NativeClass, Node, Plane, Point2, Rect2,
	Transform, Transform2D, Variant, VariantArray, Vector2, Vector3,
};

use fancy_regex::Regex;
use toml::Value;

/// Contains the methods and properties to parse a toml file and work with it.
#[derive(NativeClass)]
#[inherit(Node)]
pub struct GodotToml;

#[methods]
impl GodotToml {
	/// Used by godot to initialize the class.
	fn _init(_owner: Node) -> Self {
		GodotToml
	}

	/// Parses a toml file at a specified path and returns a dictonary populated with the values from it.
	///
	/// # Arguments
	///
	/// `path` - The path to the toml file to parse.
	#[export]
	fn parse_toml(&mut self, _owner: Node, path: GodotString) -> Dictionary {
		// Open the file using Godot's File object.
		let mut file = File::new();
		match file.open(path, 3) {
			Ok(_v) => (),
			Err(e) => {
				godot_print!(
					"Unable to open file with an error of {:?}. Please make sure the path to file is correct",
					e
				);
			}
		};

		// Read the contents of the file as a string and then parse that string with the toml crate.
		let file_to_godot_string = file.get_as_text();
		let file_to_variant = Variant::from_godot_string(&file_to_godot_string);
		let file_to_string = Variant::to_string(&file_to_variant);
		let toml: Value =
			toml::from_str(&file_to_string.to_owned()).expect("Unable to parse toml file.");
		let toml_map = toml
			.as_table()
			.expect("Unable to get contents of toml file");
		// Create the dictionary and then populate it using the contents of the toml file.
		let mut toml_dictionary = Dictionary::new();
		populate_toml_dictionary(&toml, &mut toml_dictionary, toml_map);
		return toml_dictionary;
	}
}

/// Populates a dictonary with the parsed values of the toml table provided.
///
/// If a value contains a string, it is further parsed by convert_godot_types which checks to see if the string is a Godot type and then performs
/// the necessary conversions on it and adds it to the dictionary.
///
/// # Arguments
///
/// `toml` - The parsed toml.
/// `dictionary` - The dictionary to populate.
/// `table` - The Table from the parsed toml.
fn populate_toml_dictionary(
	toml: &Value,
	dictionary: &mut Dictionary,
	table: &toml::map::Map<std::string::String, Value>,
) {
	for (key, value) in table {
		let field_type = value.type_str();
		match field_type {
			"table" => {
				let sub_dic = &mut Dictionary::new();
				let new_table = table[key]
					.as_table()
					.expect("Unable to cast value to table");
				populate_toml_dictionary(toml, sub_dic, new_table);
				dictionary.set(&Variant::from_str(key), &Variant::from_dictionary(sub_dic));
			}
			"array" => {
				let mut dictionary_arr = VariantArray::new();
				let toml_arr = table[key]
					.as_array()
					.expect("Unable to cast value to array");
				for i in toml_arr {
					let sub_dic = &mut Dictionary::new();
					populate_toml_dictionary(toml, sub_dic, i.as_table().unwrap());
					dictionary_arr.push(&Variant::from_dictionary(sub_dic));
				}
				dictionary.set(
					&Variant::from_str(key),
					&Variant::from_array(&dictionary_arr),
				)
			}
			"integer" => dictionary.set(
				&Variant::from_str(key),
				&Variant::from_i64(value.as_integer().expect("Unable to cast value to integer")),
			),
			"string" => {
				let value_as_str = value.as_str().expect("Unable to cast value to string");
				// A simple check to that we can avoid the cost of regex if we don't need to is to check if the string contains a parenthesis.
				if value_as_str.contains("(") {
					encode_godot_types(dictionary, key, value_as_str);
				} else {
					dictionary.set(&Variant::from_str(key), &Variant::from_str(value_as_str))
				}
			}
			"float" => dictionary.set(
				&Variant::from_str(key),
				&Variant::from_f64(value.as_float().expect("Unable to cast value to float")),
			),
			"boolean" => dictionary.set(
				&Variant::from_str(key),
				&Variant::from_bool(value.as_bool().expect("Unable to cast value to bool")),
			),
			"datetime" => dictionary.set(
				&Variant::from_str(key),
				&Variant::from_str(
					value
						.as_datetime()
						.expect("Unable to cast value to float")
						.to_string(),
				),
			),
			_ => (),
		}
	}
}

/// Checks to see if a toml string is a Godot type and if so use `set_godot_type_to_dictionary`
///
/// # Arguments
///
/// `dictionary` - A reference to the dictionary so that the Godot types can be added to it.
/// `key` - The key of the current string item being checked.
/// `value` - The value of the current string item being checked.
fn encode_godot_types(dictionary: &mut Dictionary, key: &str, value: &str) {
	// Create a pattern to check for Godot types (Vector2, Rect2, etc.) and check to see if there are any matches in the string.
	// let type_re = Regex::new(r"((?:\/)?(\w+))").expect("Unable to create regex for type");
	let type_re =
		Regex::new(r"((?:\/)?([a-zA-Z0-9\.]+))").expect("Unable to create regex for type");
	let mut type_idx = 0;
	let mut type_results: Vec<&str> = vec![];
	while let Some(t) = type_re
		.captures_from_pos(value, type_idx)
		.expect("Unable to get captures")
	{
		type_results.push(t.get(1).expect("Unable to get capture group").as_str());
		type_idx = t.get(0).expect("Unable to get capture group").end();
	}

	let godot_types: [&str; 8] = [
		"Vector2",
		"Vector3",
		"Color",
		"Rect2",
		"Plane",
		"Transform2D",
		"Basis",
		"Transform",
	];

	if !godot_types.contains(&type_results[0]) {
		dictionary.set(&Variant::from_str(&key), &Variant::from_str(&value));
		return;
	}

	// If there is a pattern then we need to decode the regex results into the Godot type.
	set_godot_type_to_dictionary(type_results, key, dictionary, &mut None, &mut None);
}

/// Returns a Vector2 at the specified (x, y) location.
///
/// # Arguments
///
/// `x` - The x position of the Vector2.
/// `y` - The y position of the Vector2.
fn encode_vector2(x: &str, y: &str) -> Vector2 {
	let vec_x: f32 = x.trim().parse().expect("Unable to cast to f32");
	let vec_y: f32 = y.trim().parse().expect("Unable to cast to f32");

	return Vector2::new(vec_x, vec_y);
}

/// Returns a Vector3 with the specified x, y, and z values.
///
/// # Arguments
///
/// `x` - The x value of the Vector3.
/// `y` - The y value of the Vector3.
/// `z` - The z value of the Vector3.
fn encode_vector3(x: &str, y: &str, z: &str) -> Vector3 {
	let vec_x: f32 = x.trim().parse().expect("Unable to cast to f32");
	let vec_y: f32 = y.trim().parse().expect("Unable to cast to f32");
	let vec_z: f32 = z.trim().parse().expect("Unable to cast to f32");

	return Vector3::new(vec_x, vec_y, vec_z);
}

/// Returns a Rect2 at the specified point and with the specified size.
///
/// # Arguments
///
/// `pos_vec` - The Vector2 that defines the Rect2's position.
/// `size_vec` - The Vector2 that defines the Rect's size.
fn encode_rect2(pos_vec: Vector2, size_vec: Vector2) -> Rect2 {
	return Rect2::new(
		Point2::new(pos_vec.x, pos_vec.y),
		Size2D::new(size_vec.x, size_vec.y),
	);
}

/// Returns a Transform2D with the provided x, y, and origin vectors.
///
/// # Arguments
///
/// `transform2d_init` - The values provided for the Transform2D.
fn encode_transform2d(
	x_axis_vec: Vector2,
	y_axis_vec: Vector2,
	origin_vec: Vector2,
) -> Transform2D {
	return Transform2D::row_major(
		x_axis_vec.x,
		x_axis_vec.y,
		y_axis_vec.x,
		y_axis_vec.y,
		origin_vec.x,
		origin_vec.y,
	);
}

/// Returns a Transform with the provided axis vectors and the origin vector.
///
/// # Arguments
///
/// `x_axis_vec` - The x axis vector for the transform.
/// `y_axis_vec` - The y axis vector for the transform.
/// `z_axis_vec` - The z axis vector for the transform.
/// `origin_vec` - The vector that specifies the origin of the transform.
fn encode_transform(
	x_axis_vec: Vector3,
	y_axis_vec: Vector3,
	z_axis_vec: Vector3,
	origin_vec: Vector3,
) -> Transform {
	return Transform {
		basis: encode_basis(x_axis_vec, y_axis_vec, z_axis_vec),
		origin: origin_vec,
	};
}

/// Returns a Plane with the provided normal Vector and d value.
///
/// # Arguments
///
/// `normal_vec` - The Plane's normal vector.
/// `d` - The d value of the Plane.
fn encode_plane(normal_vec: Vector3, d: &str) -> Plane {
	let d_parsed: f32 = d.trim().parse().expect("Unable to cast to f32");

	return Plane {
		normal: normal_vec,
		d: d_parsed,
	};
}

/// Returns a 3x3 matrix used consisting of Vector3 values for x, y, and z.
///
/// # Arguments
///
/// `x` - The Vector3 that defines the x column.
/// `y` - The Vector3 that defines the y column.
/// `z` - The Vector3 that defines the z column.
fn encode_basis(x: Vector3, y: Vector3, z: Vector3) -> Basis {
	return Basis {
		elements: [x, y, z],
	};
}

/// Returns a Color from the specified rgb and optional a.
///
/// # Arguments
///
/// `r` - The red value of the color.
/// `g` - The green value of the color.
/// `b` - The blue value of the color.
/// `a` - The optional alpha value of the color.
fn encode_color(r: &str, g: &str, b: &str, a: Option<&str>) -> Color {
	let r_parsed: f32 = r.trim().parse().expect("Unable to cast to float");
	let g_parsed: f32 = g.trim().parse().expect("Unable to cast to float");
	let b_parsed: f32 = b.trim().parse().expect("Unable to cast to float");

	match a {
		Some(alpha) => {
			let a_parsed: f32 = alpha.trim().parse().expect("Unable to cast to float");
			return Color::rgba(r_parsed, g_parsed, b_parsed, a_parsed);
		}
		None => return Color::rgb(r_parsed, g_parsed, b_parsed),
	};
}

/// Takes the results of the regex provided by `convert_godot_types` and determines what Godot type needs to be created
/// and added to the dictionary.
///
/// # Arguments
///
/// `regex_results` - The vector of results from `convert_godot_types`.
/// `key` - The key of the current item.
/// `dictionary` - A reference to the dictionary so that Godot types can be added to it.
/// `vec2_pool` - An optional pool of Vector2s that is used when this function is called recursively for complex types made up of Vector2s.
/// `vec3_pool` - An optional pool of Vector3s that is used when this function is called recursively for complex types made up of Vector3s.
fn set_godot_type_to_dictionary(
	regex_results: Vec<&str>,
	key: &str,
	dictionary: &mut Dictionary,
	vec2_pool: &mut Option<&mut Vec<Vector2>>,
	vec3_pool: &mut Option<&mut Vec<Vector3>>,
) {
	for (i, item) in regex_results.iter().enumerate() {
		match item {
			&"Vector2" => {
				let vector2 = encode_vector2(regex_results[i + 1], regex_results[i + 2]);
				match vec2_pool {
					Some(x) => x.push(vector2),
					None => {
						dictionary.set(&Variant::from_str(&key), &Variant::from_vector2(&vector2));
						break;
					}
				}
			}
			&"Vector3" => {
				let vector3 = encode_vector3(
					regex_results[i + 1],
					regex_results[i + 2],
					regex_results[i + 3],
				);
				match vec3_pool {
					Some(x) => x.push(vector3),
					None => {
						dictionary.set(&Variant::from_str(&key), &Variant::from_vector3(&vector3));
						break;
					}
				};
			}
			&"Color" => {
				let mut alpha: std::option::Option<&str> = None;
				if regex_results.len() == 5 {
					alpha = Some(regex_results[i + 4]);
				}
				let color = encode_color(
					regex_results[i + 1],
					regex_results[i + 2],
					regex_results[i + 3],
					alpha,
				);
				dictionary.set(&Variant::from_str(&key), &Variant::from_color(&color));
				break;
			}
			&"Rect2" => {
				// Since a Rect2 is a complex type that consists of Vector2's, we need to run the this function recursively to get
				// the Vector2 position and Vector2 size values.
				let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
				let vec2_pool: &mut Vec<Vector2> = &mut vec![];
				set_godot_type_to_dictionary(
					new_regex_results,
					key,
					dictionary,
					&mut Some(vec2_pool),
					&mut None,
				);

				let rect2 = encode_rect2(vec2_pool[0], vec2_pool[1]);
				dictionary.set(&Variant::from_str(&key), &Variant::from_rect2(&rect2));
				break;
			}
			&"Plane" => {
				// Plane is a complex type made up of a Vector3 and a float so we need to use recursion to get the Vector value.
				let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
				let vec3_pool: &mut Vec<Vector3> = &mut vec![];
				set_godot_type_to_dictionary(
					new_regex_results,
					key,
					dictionary,
					&mut None,
					&mut Some(vec3_pool),
				);

				let plane = encode_plane(vec3_pool[0], regex_results[regex_results.len() - 1]);
				dictionary.set(&Variant::from_str(&key), &Variant::from_plane(&plane));
				break;
			}
			&"Transform2D" => {
				// Transform2D is a complex type made up of three Vector2s so we need to use recursion to get the Vector2 values.
				let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
				let vec2_pool: &mut Vec<Vector2> = &mut vec![];
				set_godot_type_to_dictionary(
					new_regex_results,
					key,
					dictionary,
					&mut Some(vec2_pool),
					&mut None,
				);

				let transform2d = encode_transform2d(vec2_pool[0], vec2_pool[1], vec2_pool[2]);
				dictionary.set(
					&Variant::from_str(&key),
					&Variant::from_transform2d(&transform2d),
				);
				break;
			}
			&"Basis" => {
				// Basis is a complex type made up to three Vector3s so we need to use recursion to get the Vector3 values.
				let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
				let vec3_pool: &mut Vec<Vector3> = &mut vec![];
				set_godot_type_to_dictionary(
					new_regex_results,
					key,
					dictionary,
					&mut None,
					&mut Some(vec3_pool),
				);

				let basis = encode_basis(vec3_pool[0], vec3_pool[1], vec3_pool[2]);
				dictionary.set(&Variant::from_str(&key), &Variant::from_basis(&basis));
				break;
			}
			&"Transform" => {
				// Transform is a complex type made up of four Vector3s so we need to use recursion to get the Vector3 values.
				let new_regex_results = regex_results[i + 1..regex_results.len()].to_vec();
				let vec3_pool: &mut Vec<Vector3> = &mut vec![];
				set_godot_type_to_dictionary(
					new_regex_results,
					key,
					dictionary,
					&mut None,
					&mut Some(vec3_pool),
				);

				let transform =
					encode_transform(vec3_pool[0], vec3_pool[1], vec3_pool[2], vec3_pool[3]);
				dictionary.set(
					&Variant::from_str(&key),
					&Variant::from_transform(&transform),
				);
				break;
			}
			_ => (),
		}
	}
}
