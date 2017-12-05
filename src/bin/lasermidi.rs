extern crate css_color_parser;
extern crate docopt;
//extern crate dxf;
extern crate lasermidi;
extern crate rimd;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use docopt::Docopt;
use rimd::SMF;
use std::fs::File;
use std::io::{stdout, Write};
use std::path::Path;

use lasermidi::*;

const USAGE: &'static str = "
Usage:
    lasermidi [options] INPUT [OUTPUT]
    lasermidi (--help | --version)

All measurements are in mm.

Options:
    -h, --help  Show this message and exit.
    --version  Print the version and exit.
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
    --font-file <ttf>  Font to use for PDF format.
    --output-format <format>  SVG, PDF, or JSON.
";

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct Args {
    arg_INPUT: String,
    arg_OUTPUT: Option<String>,
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
    flag_output_format: Option<OutputFormat>,
    flag_font_file: Option<String>,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| {
            d.help(true)
                .version(Some(
                    env!("CARGO_PKG_NAME").to_string() + " v" +
                        env!("CARGO_PKG_VERSION"),
                ))
                .deserialize()
        })
        .unwrap_or_else(|e| e.exit());

    let notes: Vec<u8> = args.flag_notes
        .split(',')
        .map(|x| x.parse().expect("Invalid note number"))
        .collect();

    let smf = SMF::from_file(&Path::new(&args.arg_INPUT[..])).expect("Failed to load MIDI file");
    let output_pattern = args.arg_OUTPUT.clone();
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
        font_file: args.flag_font_file,
    };
    let output_format = args.flag_output_format.unwrap_or_else(|| {
        output_pattern
            .as_ref()
            .map(|o| o.to_lowercase())
            .map(|o| {
                if o.ends_with(".json") {
                    OutputFormat::JSON
                } else if o.ends_with(".pdf") {
                    OutputFormat::PDF
                } else if o.ends_with(".svg") {
                    OutputFormat::SVG
                // } else if o.ends_with(".dxf") {
                // OutputFormat::DXF
                } else {
                    OutputFormat::JSON
                }
            })
            .unwrap_or(OutputFormat::JSON)
    });
    let layout = options.layout(smf).unwrap();
    match output_format {
        OutputFormat::SVG => {
            options
                .make_svg(&layout[..], &mut |page_num| match output_pattern {
                    Some(ref pattern) => Box::new(
                        File::create(Path::new(
                            &pattern.replace("%", &(page_num + 1).to_string())[..],
                        )).expect("Failed to open output file"),
                    ),
                    None => Box::new(stdout()),
                })
                .expect("Failed to write SVG to output file")
        }
        OutputFormat::JSON => {
            let output: Box<Write> = match output_pattern {
                Some(ref pattern) => {
                    Box::new(File::create(Path::new(pattern)).expect(
                        "Failed to open output file",
                    ))
                }
                None => Box::new(stdout()),
            };
            serde_json::to_writer_pretty(output, &layout).expect("Failed to write output");
        }
        #[cfg(feature = "pdf")]
        OutputFormat::PDF => {
            let mut output: Box<Write> = match output_pattern {
                Some(ref pattern) => {
                    Box::new(File::create(Path::new(pattern)).expect(
                        "Failed to open output file",
                    ))
                }
                None => Box::new(stdout()),
            };
            options.make_pdf(&layout[..], &mut output).unwrap();
        }
        #[cfg(not(feature = "pdf"))]
        OutputFormat::PDF => panic!("pdf support was disabled at compile time"),
    }
}
