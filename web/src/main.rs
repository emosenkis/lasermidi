#![recursion_limit="128"] // This is needed for the large js! block
extern crate css_color_parser;
extern crate docopt;
extern crate lasermidi;
extern crate rimd;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate shlex;
#[macro_use]
extern crate stdweb;

use docopt::Docopt;
use rimd::SMF;
use std::fs::{read_dir, remove_file, File};
use std::io::Read;
use std::path::Path;
use stdweb::web::TypedArray;

use lasermidi::*;

const USAGE: &'static str = "
Usage:
    lasermidi [options]

All measurements are in mm.

Options:
    -t, --track-num <num>  Track number to process. [default: 1]
    -n, --notes <notes>  Comma-separated list of MIDI note numbers supported by your music box.
      [default: 40,42,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,\
      59,60,61,62,63,64,66,68,69,71,73,78,80].
    --tape-height <height>  Height of programming tape. [default: 68.6]
    --space-above-top-row <space>  Space between edge of tape and first row.  [default: 6]
    --space-below-bottom-row <space>  Space between last row and edge of tape.  [default: 5]
    --space-before-first-note <space>  Space between end of lead-in and first note.  [default: 20]
    --space-after-last-note <space>  Space between last note and end of tape.  [default: 20]
    --space-between-strips <space>  Vertical space between two strips cut from the same page.
        [default: 10]
    --hole-diameter <diameter>  Diameter of each hole. [default: 2.4]
    --page-width <width>  Width of the page. [default: 297]
    --page-height <height>  Height of the page. [default: 210]
    --margin-left <margin>  Left margin. [default: 10]
    --margin-right <margin>  Right margin. [default: 10]
    --margin-top <margin>  Top margin. [default: 10]
    --margin-bottom <margin>  Bottom margin. [default: 10]
    --cut-stroke-width <width>  Width of lines to be cut. Should equal the kerf. [default: 0.08]
    --cut-color <color>  SVG color of lines to be cut. [default: red]
    --engrave-color <color>  SVG color for engraving.  [default: black]
    --stretch <factor>  Horizontal stretch factor (mm / beat). [default: 16]
    --lead-in-width <width>  Width of diagonal edge at beginning of first page.  [default: 15]
    --lead-in-height <width>  Height of diagonal edge at beginning of first page.  [default: 35]
    --num-zig-zags <num>  Number of zig-zags in connecting edges.  [default: 5]
    --join-width <width>  Width of connecting edge join.  [default: 5]
    --join-style <style>  Straight, zigzag, or diagonal.  [default: zigzag]
    --title <title>  Name of song.
";

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct Args {
    flag_track_num: usize,
    flag_notes: String,
    flag_tape_height: f64,
    flag_space_above_top_row: f64,
    flag_space_below_bottom_row: f64,
    flag_space_before_first_note: f64,
    flag_space_after_last_note: f64,
    flag_space_between_strips: f64,
    flag_page_width: f64,
    flag_page_height: f64,
    flag_margin_left: f64,
    flag_margin_top: f64,
    flag_margin_right: f64,
    flag_margin_bottom: f64,
    flag_hole_diameter: f64,
    flag_cut_stroke_width: f64,
    flag_cut_color: String,
    flag_engrave_color: String,
    flag_stretch: f64,
    flag_lead_in_width: f64,
    flag_lead_in_height: f64,
    flag_num_zig_zags: u16,
    flag_join_width: f64,
    flag_join_style: JoinStyle,
    flag_title: String,
}

fn main() {
    stdweb::initialize();
    js! {
        Module.convert = @{convert};
        document.getElementById("usage").innerText = @{USAGE};
    }
    stdweb::event_loop();
}

pub fn convert(data: TypedArray<u8>, flags: String, file_name: String) {
    let args: Args = Docopt::new(USAGE)
        .unwrap()
        .argv(shlex::split(&flags).unwrap())
        .deserialize()
        .unwrap();

    let notes: Vec<u8> = args.flag_notes
        .split(',')
        .map(|x| x.parse().expect("Invalid note number"))
        .collect();

    let smf = SMF::from_reader(&mut &data.to_vec()[..]).expect("Failed to load MIDI file");
    let options = Options {
        track_num: args.flag_track_num,
        tape_height: args.flag_tape_height,
        interior_margin_top: args.flag_space_above_top_row,
        interior_margin_left: args.flag_space_before_first_note,
        interior_margin_right: args.flag_space_after_last_note,
        row_spacing: (args.flag_tape_height - args.flag_space_above_top_row -
                          args.flag_space_below_bottom_row) /
            (notes.len() as f64 - 1.0),
        notes: notes,
        page_width: args.flag_page_width,
        page_height: args.flag_page_height,
        margin_left: args.flag_margin_left,
        margin_top: args.flag_margin_top,
        margin_right: args.flag_margin_right,
        margin_bottom: args.flag_margin_bottom,
        gap: args.flag_space_between_strips,
        hole_radius: args.flag_hole_diameter / 2.0,
        cut_stroke_width: args.flag_cut_stroke_width,
        cut_color: args.flag_cut_color.parse().expect(
            "Failed to parse cut color",
        ),
        engrave_color: args.flag_engrave_color.parse().expect(
            "Failed to parse engrave color",
        ),
        stretch: args.flag_stretch,
        lead_in_width: args.flag_lead_in_width,
        lead_in_height: args.flag_lead_in_height,
        num_zig_zags: args.flag_num_zig_zags,
        join_width: args.flag_join_width,
        join_style: args.flag_join_style,
        title: args.flag_title,
        font_file: None,
    };
    let layout = options.layout(smf).unwrap();
    options
        .make_svg(&layout[..], &mut |page_num| {
            Box::new(
                File::create(Path::new(
                    &"/out_%.svg".replace("%", &format!("{:06}", page_num))[..],
                )).expect("Failed to open output file"),
            )
        })
        .expect("Failed to write SVG to output file");
    js!{
        Module.output = [];
        document.getElementById("output").innerHTML = "";
    }
    let mut paths: Vec<_> = read_dir("/")
        .unwrap()
        .map(|x| x.unwrap())
        .filter(|x| x.file_type().unwrap().is_file())
        .map(|x| x.path())
        .collect();
    paths.sort();
    for (i, path) in paths.iter().enumerate() {
        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        remove_file(&path).unwrap();
        js!{
            var output = document.getElementById("output");
            var url = "data:image/svg+xml;base64," + btoa(@{content});
            var i = @{i as u8} + 1;
            var link = document.createElement("a");
            link.href = url;
            link.download = @{&file_name}.replace(new RegExp("(\\.midi?)?$", "i"), "." + i + ".svg");
            link.innerText = "Page " + i;
            link.onmouseover = function(e) {
                preview(e.target.href);
            };
            output.appendChild(document.createTextNode(" "));
            output.appendChild(link);
            if (i == 1) {
                preview(url);
            }
        };
    }
    js!{
        document.body.scrollTop = document.body.scrollHeight;
    };
}
