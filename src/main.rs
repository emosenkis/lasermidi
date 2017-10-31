// TODO:
// - Support diagonal/straight join instead of teeth
// - Avoid panics
// - Draw grid
// - Make last strip as short as allowed by the last note
// - Separate into generating the layout and converting it to SVG

extern crate docopt;
extern crate rimd;
#[macro_use]
extern crate serde_derive;

use docopt::Docopt;
use rimd::{Event, SMFError, Status, SMF};
use std::fs::File;
use std::io::{stdout, Write};
use std::path::Path;

const USAGE: &'static str = "
Usage:
    lasermidi [options] INPUT [OUTPUT]
    lasermidi (--help | --version)

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
    --connecting-edge-num-teeth <num>  Number of zig-zags in connecting edges.  [default: 5]
    --connecting-edge-join-width <width>  Width of connecting edge join.  [default: 5]
    --title <title>  Name of song.
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
    flag_connecting_edge_num_teeth: u16,
    flag_connecting_edge_join_width: f64,
    flag_title: String,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let mut notes: Vec<u8> = args.flag_notes
        .split(',')
        .map(|x| x.parse().expect("Invalid note number"))
        .collect();
    notes.sort();

    match SMF::from_file(&Path::new(&args.arg_INPUT[..])) {
        Ok(smf) => {
            let output_pattern = args.arg_OUTPUT.clone();
            Options {
                track_num: args.flag_track_num,
                tape_height: args.flag_tape_height,
                interior_margin_top: args.flag_space_above_top_row,
                interior_margin_left: args.flag_space_before_first_note,
                interior_margin_right: args.flag_space_after_last_note,
                row_spacing: (args.flag_tape_height - args.flag_space_above_top_row
                    - args.flag_space_below_bottom_row)
                    / (notes.len() as f64 - 1.0),
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
                cut_color: args.flag_cut_color,
                engrave_color: args.flag_engrave_color,
                stretch: args.flag_stretch,
                lead_in_width: args.flag_lead_in_width,
                lead_in_height: args.flag_lead_in_height,
                num_teeth: args.flag_connecting_edge_num_teeth,
                join_width: args.flag_connecting_edge_join_width,
                title: args.flag_title,
            }.make_svg(smf, &mut |page_num| match output_pattern {
                Some(ref pattern) => Box::new(
                    File::create(Path::new(
                        &pattern.replace("%", &(page_num + 1).to_string())[..],
                    )).expect("Failed to open output file"),
                ),
                None => Box::new(stdout()),
            });
        }
        Err(e) => match e {
            SMFError::InvalidSMFFile(s) => {
                println!("{}", s);
            }
            SMFError::Error(e) => {
                println!("io: {}", e);
            }
            SMFError::MidiError(_) => {
                println!("Midi Error");
            }
            SMFError::MetaError(_) => {
                println!("Meta Error");
            }
        },
    }
}

struct Options {
    track_num: usize,
    notes: Vec<u8>,
    tape_height: f64,
    interior_margin_top: f64,
    interior_margin_left: f64,
    interior_margin_right: f64,
    row_spacing: f64,
    page_width: f64,
    page_height: f64,
    margin_left: f64,
    margin_top: f64,
    margin_right: f64,
    margin_bottom: f64,
    gap: f64,
    hole_radius: f64,
    cut_stroke_width: f64,
    cut_color: String,
    engrave_color: String,
    stretch: f64,
    lead_in_width: f64,
    lead_in_height: f64,
    num_teeth: u16,
    join_width: f64,
    title: String,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug)]
struct Note {
    time: u64,
    note: u8,
}

impl Options {
    pub fn make_svg(&self, smf: SMF, make_output_stream: &mut FnMut(u16) -> Box<Write>) {
        let div = smf.division;
        if div <= 0 {
            panic!("Unsupported div {}", div);
        }

        let mut time = 0;
        let mut max = 0;
        let notes = {
            let mut notes = Vec::new();
            for event in &smf.tracks[self.track_num].events {
                time += event.vtime;
                match event.event {
                    Event::Midi(ref msg) => if msg.status() == Status::NoteOn {
                        if time > max {
                            max = time;
                        }
                        notes.push(Note {
                            time: time,
                            note: 128 - msg.data(1),
                        });
                    },
                    Event::Meta(_) => {}
                };
            }
            notes.sort_unstable();
            let min_time = notes[0].time;
            for note in notes.iter_mut() {
                note.time -= min_time;
            }
            notes
        };
        let max_time = notes[notes.len() - 1].time;
        let total_width = self.time_to_width(div, max_time);
        let usable_width_first_strip = self.page_width - self.margin_left - self.margin_right
            - self.lead_in_width - self.interior_margin_left
            - self.hole_radius - self.join_width;
        let usable_width_middle_strip =
            self.page_width - self.margin_left - self.margin_right - self.join_width;
        let usable_width_last_strip = self.page_width - self.margin_left - self.margin_right
            - self.interior_margin_right - self.hole_radius;
        let usable_width_only_strip = self.page_width - self.margin_left - self.margin_right
            - self.lead_in_width - self.interior_margin_left
            - self.interior_margin_right
            - (2.0 * self.hole_radius);
        let num_strips = if total_width <= usable_width_only_strip {
            1
        } else {
            2
                + ((total_width - usable_width_first_strip - usable_width_last_strip)
                    / usable_width_middle_strip)
                    .ceil() as u16
        };
        let strips_per_page = 1
            + ((self.page_height - self.margin_top - self.margin_bottom - self.tape_height)
                / (self.gap + self.tape_height))
                .floor() as u16;
        let mut output: Box<Write> = Box::new(stdout());
        for strip_num in 0..num_strips {
            let strip_on_page = strip_num % strips_per_page;
            let page_num = strip_num / strips_per_page;
            let first_strip = strip_num == 0;
            let last_strip = strip_num + 1 == num_strips;
            let last_strip_on_page = last_strip || strip_on_page + 1 == strips_per_page;
            let top_edge = self.margin_top + strip_on_page as f64 * (self.tape_height + self.gap);
            let bottom_edge = top_edge + self.tape_height;
            let left_edge = self.margin_left;
            let right_edge = self.page_width - self.margin_right - if last_strip {
                0.0
            } else {
                self.join_width
            };
            let x_offset = if first_strip {
                self.lead_in_width + self.interior_margin_left + self.hole_radius
            } else {
                -(usable_width_first_strip + ((strip_num - 1) as f64 * usable_width_middle_strip))
            };
            let tape_border = {
                let mut points = Vec::new();
                if first_strip {
                    let lead_in_right = left_edge + self.lead_in_width;
                    let lead_in_top = top_edge + (self.tape_height - self.lead_in_height);
                    points.push((left_edge, top_edge));
                    points.push((left_edge, lead_in_top));
                    points.push((lead_in_right, bottom_edge));
                } else {
                    points.extend(self.make_teeth(left_edge, top_edge));
                }
                if last_strip {
                    points.push((right_edge, bottom_edge));
                    points.push((right_edge, top_edge));
                } else {
                    points.extend(self.make_teeth(right_edge, top_edge).iter().rev());
                }
                points
            };
            if strip_on_page == 0 {
                output = make_output_stream(page_num);
                writeln!(output, r#"<?xml version="1.0" encoding="UTF-8" ?>"#).unwrap();
                writeln!(
                    output,
                    r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{page_width:.2}mm" height="{page_height:.2}mm" viewBox="0 0 {page_width:.2} {page_height:.2}">"#,
                    page_width = self.page_width,
                    page_height = self.page_height
                ).unwrap();
                writeln!(
                    output,
                    r#"<g fill="none" stroke-width="{cut_stroke_width:.2}" stroke="{cut_color}">"#,
                    cut_stroke_width = self.cut_stroke_width,
                    cut_color = self.cut_color
                ).unwrap();
            }
            writeln!(
                output,
                r#"<defs><clipPath id="strip_{}_border">"#,
                strip_num
            ).unwrap();
            self.polygon(&tape_border[..], &mut output);
            writeln!(output, "</clipPath></defs>").unwrap();
            writeln!(
                output,
                r#"<g clip-path="url(#strip_{}_border)">"#,
                strip_num
            ).unwrap();

            // Draw the border with double the stroke width but allow half of that to be clipped,
            // producing exactly the requested strip size (assuming that stroke width is exactly
            // equal to the kerf.
            writeln!(
                output,
                r#"<g stroke-width="{:.2}">"#,
                self.cut_stroke_width * 2.0
            ).unwrap();
            self.polygon(&tape_border[..], &mut output);
            writeln!(output, "</g>").unwrap();

            if !self.title.is_empty() {
                writeln!(
                    output,
                    r#"<text x="{x:.2}" y="{y:.2}" font-size="{font_size:.2}" fill="{engrave_color}" stroke="none">{title} ({strip_num} of {num_strips})</text>"#,
                    x = left_edge + if first_strip {
                        self.lead_in_width
                    } else {
                        self.join_width + 1.0
                    },
                    y = top_edge + self.interior_margin_top / 2.0,
                    font_size = self.interior_margin_top / 2.0,
                    title = self.title,
                    strip_num = strip_num + 1,
                    num_strips = num_strips,
                    engrave_color = self.engrave_color,
                ).unwrap();
            }
            writeln!(
                output,
                r#"<svg x="{margin_left:.2}" y="{top_edge:.2}" width="{tape_width:.2}" height="{tape_height:.2}">"#,
                margin_left = self.margin_left,
                top_edge = top_edge,
                tape_width = self.page_width - self.margin_left - self.margin_right,
                tape_height = self.tape_height,
            ).unwrap();

            for note in &notes {
                let row = self.notes
                    .binary_search(&note.note)
                    .expect("Unsupported note");
                let note_x = self.time_to_width(div, note.time) + self.hole_radius + x_offset;
                if note_x + self.hole_radius < 0.0 {
                    // TODO: Binary search instead
                    continue;
                } else if note_x - self.hole_radius > self.page_width - self.margin_right {
                    break;
                }
                writeln!(
                    output,
                    "<!-- time={}, note={} row={} -->",
                    note.time,
                    note.note,
                    row + 1
                ).unwrap();
                writeln!(
                    output,
                    r#"<circle cx="{x:.2}" cy="{y:.2}" r="{hole_radius:.2}" />"#,
                    x = note_x,
                    y = row as f64 * self.row_spacing + self.interior_margin_top,
                    // Reduce the radius by 1/2 the kerf to create a resulting hole of the exact
                    // size requested.
                    hole_radius = self.hole_radius - (self.cut_stroke_width / 2.0),
                ).unwrap();
            }
            writeln!(output, "</svg>").unwrap();
            writeln!(output, "</g>").unwrap();
            if last_strip_on_page {
                writeln!(output, "</g>").unwrap();
                writeln!(output, "</svg>").unwrap();
            }
        }
    }

    fn time_to_width(&self, div: i16, time: u64) -> f64 {
        return time as f64 * self.stretch / div as f64;
    }

    fn polygon(&self, points: &[(f64, f64)], output: &mut Write) {
        write!(
            output,
            r#"<polygon points="{:2},{:2}"#,
            points[0].0,
            points[0].1
        ).unwrap();
        for i in 1..points.len() {
            write!(output, " {:.2},{:.2}", points[i].0, points[i].1).unwrap();
        }
        writeln!(output, r#""/>"#).unwrap();
    }

    #[allow(dead_code)]
    fn line(&self, start: (f64, f64), end: (f64, f64), output: &mut Write) {
        writeln!(
            output,
            r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" />"#,
            x1 = start.0,
            y1 = start.1,
            x2 = end.0,
            y2 = end.1,
        ).unwrap();
    }

    fn make_teeth(&self, x: f64, y: f64) -> Vec<(f64, f64)> {
        let tooth_height = self.tape_height / self.num_teeth as f64;
        let mut points = Vec::new();
        for i in 0..self.num_teeth {
            let left = x;
            let right = x + self.join_width;
            let top = y + (i as f64 * tooth_height);
            let middle = y + ((i * 2 + 1) as f64 * tooth_height / 2.0);
            points.push((left, top));
            points.push((right, middle));
        }
        points.push((x, y + self.tape_height));
        points
    }
}
