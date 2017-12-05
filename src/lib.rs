// TODO:
// - Use Failure and remove the last few calls to unwrap() and expect()
// - Draw grid
// - Warn or fail if output pattern doesn't contain % and num_pages > 1
// - Check that track contains at least one note
// - Support printing --notes value based on notes in a MIDI file
// - Feature gate PDF support (because it adds tons of deps)
// - Support DXF output
// - PDF: Expand outline to get precise size specified
// - Write tests
// - Support multi-page SVG if output pattern doesn't contain %
// - Add a user-friendly web interface

extern crate css_color_parser;
//extern crate dxf;
#[cfg(feature = "pdf")]
#[macro_use]
extern crate printpdf;
extern crate rimd;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use css_color_parser::Color;
use rimd::{Event, Status, SMF};
#[cfg(feature = "pdf")]
use std::fs::File;
use std::io::{self, Write};

#[derive(Debug, Deserialize, Eq, Copy, Clone, PartialEq)]
pub enum OutputFormat {
    SVG,
    PDF,
    JSON,
    // DXF,
}

#[derive(Debug, Deserialize, Eq, Copy, Clone, PartialEq)]
pub enum JoinStyle {
    ZigZag,
    Diagonal,
    Straight,
}

pub struct Options {
    pub track_num: usize,
    pub notes: Vec<u8>,
    pub tape_height: f64,
    pub interior_margin_top: f64,
    pub interior_margin_left: f64,
    pub interior_margin_right: f64,
    pub row_spacing: f64,
    pub page_width: f64,
    pub page_height: f64,
    pub margin_left: f64,
    pub margin_top: f64,
    pub margin_right: f64,
    pub margin_bottom: f64,
    pub gap: f64,
    pub hole_radius: f64,
    pub cut_stroke_width: f64,
    pub cut_color: Color,
    pub engrave_color: Color,
    pub stretch: f64,
    pub lead_in_width: f64,
    pub lead_in_height: f64,
    pub num_zig_zags: u16,
    pub join_width: f64,
    pub join_style: JoinStyle,
    pub title: String,
    pub font_file: Option<String>,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash, Debug)]
struct Note {
    time: u64,
    note: u8,
}

pub type Point = (f64, f64);

#[derive(PartialEq, Clone, Debug, Serialize)]
pub struct Text {
    position: Point,
    text: String,
    font_size: f64,
}

#[derive(PartialEq, Clone, Debug, Serialize)]
pub struct Strip {
    texts: Vec<Text>,
    outline: Vec<Point>,
    holes: Vec<Point>,
}

#[derive(PartialEq, Clone, Debug, Serialize)]
pub struct Page {
    strips: Vec<Strip>,
}

#[derive(Eq, PartialEq, Debug)]
pub enum Error {
    /// Time-code based MIDI files are not supported.
    UnsupportedDiv,
    /// The requested track does not exist in the file.
    TrackNotFound,
    /// The requested track has zero notes in it.
    EmptyTrack,
    /// A note was present in the track that does not appear in the notes list.
    InvalidNote(u8),
}

impl Options {
    pub fn layout(&self, smf: SMF) -> Result<Vec<Page>, Error> {
        use Error::*;
        let div = smf.division;
        if div <= 0 {
            return Err(UnsupportedDiv);
        } else if smf.tracks.len() <= self.track_num {
            return Err(TrackNotFound);
        }

        let mut time = 0;
        let mut max = 0;
        let notes = {
            let mut notes = Vec::new();
            for event in &smf.tracks[self.track_num].events {
                time += event.vtime;
                match event.event {
                    Event::Midi(ref msg) => {
                        if msg.status() == Status::NoteOn {
                            if time > max {
                                max = time;
                            }
                            notes.push(Note {
                                time: time,
                                note: 128 - msg.data(1),
                            });
                        }
                    }
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
        if notes.is_empty() {
            return Err(EmptyTrack);
        }
        let join_width = if self.join_style == JoinStyle::Straight {
            0.0
        } else {
            self.join_width
        };
        let max_time = notes[notes.len() - 1].time;
        let total_width = self.time_to_width(div, max_time);
        let usable_width_first_strip =
            self.page_width - self.margin_left - self.margin_right - self.lead_in_width -
                self.interior_margin_left - self.hole_radius - join_width;
        let usable_width_middle_strip = self.page_width - self.margin_left - self.margin_right -
            join_width;
        let usable_width_last_strip = self.page_width - self.margin_left - self.margin_right -
            self.interior_margin_right - self.hole_radius;
        let usable_width_only_strip = self.page_width - self.margin_left - self.margin_right -
            self.lead_in_width - self.interior_margin_left -
            self.interior_margin_right -
            (2.0 * self.hole_radius);
        let num_strips = if total_width <= usable_width_only_strip {
            1
        } else {
            2 +
                ((total_width - usable_width_first_strip - usable_width_last_strip) /
                     usable_width_middle_strip)
                    .ceil() as u16
        };
        let strips_per_page = 1 +
            ((self.page_height - self.margin_top - self.margin_bottom - self.tape_height) /
                 (self.gap + self.tape_height))
                .floor() as u16;
        let num_pages = (num_strips + strips_per_page - 1) / strips_per_page;
        let mut pages = Vec::new();
        for page_num in 0..num_pages {
            let mut strips = Vec::new();
            for strip_on_page in 0..strips_per_page {
                let strip_num = page_num * strips_per_page + strip_on_page;
                if strip_num == num_strips {
                    break;
                }
                let first_strip = strip_num == 0;
                let last_strip = strip_num + 1 == num_strips;
                let x_offset = if first_strip {
                    self.lead_in_width + self.interior_margin_left + self.hole_radius
                } else {
                    -(usable_width_first_strip +
                          ((strip_num - 1) as f64 * usable_width_middle_strip))
                };
                let top_edge = self.margin_top +
                    strip_on_page as f64 * (self.tape_height + self.gap);
                let bottom_edge = top_edge + self.tape_height;
                let left_edge = self.margin_left;
                let right_edge = if last_strip {
                    self.time_to_width(div, notes.last().unwrap().time) + x_offset +
                        self.interior_margin_right + self.hole_radius
                } else {
                    self.page_width - self.margin_right - join_width
                };
                let outline = {
                    let mut points = Vec::new();
                    if first_strip {
                        let lead_in_right = left_edge + self.lead_in_width;
                        let lead_in_top = top_edge + (self.tape_height - self.lead_in_height);
                        points.push((left_edge, top_edge));
                        points.push((left_edge, lead_in_top));
                        points.push((lead_in_right, bottom_edge));
                    } else {
                        points.extend(self.join(left_edge, top_edge));
                    }
                    if last_strip {
                        points.push((right_edge, bottom_edge));
                        points.push((right_edge, top_edge));
                    } else {
                        points.extend(self.join(right_edge, top_edge).iter().rev());
                    }
                    points
                };
                let texts = if self.title.is_empty() {
                    Vec::new()
                } else {
                    let x = left_edge +
                        if first_strip {
                            self.lead_in_width
                        } else {
                            join_width + 1.0
                        };
                    let y = top_edge + self.interior_margin_top / 2.0;
                    vec![
                        Text {
                            position: (x, y),
                            text: format!("{} ({} of {})", self.title, strip_num + 1, num_strips),
                            font_size: self.interior_margin_top / 2.0,
                        },
                    ]
                };
                let mut holes = Vec::new();
                for note in &notes {
                    let row = match self.notes.iter().position(|&n| n == note.note) {
                        Some(i) => i,
                        None => {
                            return Err(InvalidNote(note.note));
                        }
                    };
                    let x = self.time_to_width(div, note.time) + x_offset;
                    if x + self.hole_radius < 0.0 {
                        // TODO: Binary search instead
                        continue;
                    } else if x + left_edge - self.hole_radius >
                               self.page_width - self.margin_right
                    {
                        break;
                    }
                    let y = row as f64 * self.row_spacing + self.interior_margin_top;
                    holes.push((x + left_edge, y + top_edge));
                }
                strips.push(Strip {
                    texts: texts,
                    outline: outline,
                    holes: holes,
                });
            }
            pages.push(Page { strips: strips });
        }
        Ok(pages)
    }

    pub fn make_svg(
        &self,
        pages: &[Page],
        make_output_stream: &mut FnMut(usize) -> Box<Write>,
    ) -> io::Result<()> {
        let mut strip_num = 0;
        for (page_num, page) in pages.iter().enumerate() {
            let mut output = make_output_stream(page_num);
            writeln!(output, r#"<?xml version="1.0" encoding="UTF-8" ?>"#)?;
            writeln!(
                output,
                r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{page_width:.2}mm" height="{page_height:.2}mm" viewBox="0 0 {page_width:.2} {page_height:.2}">"#,
                page_width = self.page_width,
                page_height = self.page_height
            )?;
            writeln!(
                output,
                r#"<g fill="none" stroke-width="{cut_stroke_width:.2}" stroke="rgba({r},{g},{b},{a:.2})">"#,
                cut_stroke_width = self.cut_stroke_width,
                r = self.cut_color.r,
                g = self.cut_color.g,
                b = self.cut_color.b,
                a = self.cut_color.a,
            )?;
            for strip in &page.strips {
                writeln!(
                    output,
                    r#"<defs><clipPath id="strip_{}_border">"#,
                    strip_num
                )?;
                self.polygon(&strip.outline[..], &mut output);
                writeln!(output, "</clipPath></defs>")?;
                writeln!(
                    output,
                    r#"<g clip-path="url(#strip_{}_border)">"#,
                    strip_num
                )?;

                // Draw the border with double the stroke width then clip the half of that inside
                // the border producing exactly the requested strip size (assuming that stroke
                // width is exactly equal to the kerf.
                writeln!(
                    output,
                    r#"<g stroke-width="{:.2}">"#,
                    self.cut_stroke_width * 2.0
                )?;
                self.polygon(&strip.outline[..], &mut output);
                writeln!(output, "</g>")?;
                for text in &strip.texts {
                    writeln!(
                        output,
                        r#"<text x="{x:.2}" y="{y:.2}" font-size="{font_size:.2}" fill="rgba({r},{g},{b},{a:.2})" stroke="none">{text}</text>"#,
                        x = text.position.0,
                        y = text.position.1,
                        font_size = text.font_size,
                        text = text.text,
                        r = self.engrave_color.r,
                        g = self.engrave_color.g,
                        b = self.engrave_color.b,
                        a = self.engrave_color.a,
                    )?;
                }
                for hole in &strip.holes {
                    writeln!(
                        output,
                        r#"<circle cx="{x:.2}" cy="{y:.2}" r="{hole_radius:.2}" />"#,
                        x = hole.0,
                        y = hole.1,
                        // Reduce the radius by 1/2 the kerf to create a resulting hole of the exact
                        // size requested.
                        hole_radius = self.hole_radius - (self.cut_stroke_width / 2.0),
                    )?;
                }
                writeln!(output, "</g>")?;
                strip_num += 1;
            }
            writeln!(output, "</g>")?;
            writeln!(output, "</svg>")?;
        }
        Ok(())
    }

    #[cfg(feature = "pdf")]
    pub fn make_pdf(&self, pages: &[Page], output: &mut Write) -> io::Result<()> {
        use std::io::Cursor;
        use std::io::BufWriter;
        use printpdf::*;
        let (doc, page1, layer1) = PdfDocument::new(
            self.title.clone(),
            self.page_width,
            self.page_height,
            "Layer 1".to_string(),
        );
        let font = if pages.iter().flat_map(|p| p.strips.iter()).all(|s| {
            s.texts.is_empty()
        })
        {
            None
        } else if self.font_file.is_some() {
            Some(
                doc.add_external_font(File::open(self.font_file.as_ref().unwrap())?)
                    .expect("Failed to load font"),
            )
        } else {
            Some(doc.add_builtin_font(BuiltinFont::TimesRoman).expect(
                "Failed to load built-in font",
            ))
        };
        for (page_num, page) in pages.iter().enumerate() {
            let (page_idx, layer_idx) = if page_num == 0 {
                (page1, layer1)
            } else {
                doc.add_page(self.page_width, self.page_height, "Layer 1".to_string())
            };
            let cur_layer = doc.get_page(page_idx).get_layer(layer_idx);
            cur_layer.set_outline_color(printpdf::Color::Rgb(Rgb::new(
                self.cut_color.r as f64 / 255.0,
                self.cut_color.g as f64 / 255.0,
                self.cut_color.b as f64 / 255.0,
                None,
            )));
            cur_layer.set_outline_thickness(self.cut_stroke_width);
            for strip in &page.strips {
                cur_layer.add_shape(Line::new(
                    // The outline should be grown by 1/2 line thickness to achieve the desired
                    // size after cutting.
                    strip
                        .outline
                        .iter()
                        .map(|&(x, y)| (Point::new(x, self.page_height - y), false))
                        .collect(),
                    /* has_stroke*/
                    true,
                    /* is_closed */
                    true,
                    /* has_fill */
                    false,
                ));
                for text in &strip.texts {
                    cur_layer.use_text(
                        text.text.clone(),
                        mm_to_pt!(text.font_size) as i64,
                        text.position.0,
                        self.page_height - text.position.1,
                        font.as_ref().unwrap(),
                    );
                }
                for hole in &strip.holes {
                    let x = hole.0;
                    let y = self.page_height - hole.1;
                    let r = self.hole_radius - self.cut_stroke_width;
                    cur_layer.add_shape(Line::new(
                        vec![
                            (Point::new(x - r, y - r), false),
                            (Point::new(x + r, y - r), false),
                            (Point::new(x + r, y + r), false),
                            (Point::new(x - r, y + r), false),
                        ],
                        /* has_stroke*/
                        true,
                        /* is_closed */
                        true,
                        /* has_fill */
                        false,
                    ));
                }
            }
        }
        // Using a BufWriter to a Cursor is wasteful, but it allows this to work for any Write
        // without having to guarantee that output implements Seek.
        let mut buffer = Cursor::new(Vec::<u8>::new());
        doc.save(&mut BufWriter::new(&mut buffer)).expect(
            "Failed to generate PDF",
        );
        output.write_all(&buffer.into_inner())
    }

    fn time_to_width(&self, div: i16, time: u64) -> f64 {
        return time as f64 * self.stretch / div as f64;
    }

    fn polygon(&self, points: &[Point], output: &mut Write) {
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
    fn line(&self, start: Point, end: Point, output: &mut Write) {
        writeln!(
            output,
            r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" />"#,
            x1 = start.0,
            y1 = start.1,
            x2 = end.0,
            y2 = end.1,
        ).unwrap();
    }

    fn join(&self, x: f64, y: f64) -> Vec<Point> {
        use JoinStyle::*;
        match self.join_style {
            ZigZag => self.make_zig_zags(x, y),
            Diagonal => vec![(x, y), (x + self.join_width, y + self.tape_height)],
            Straight => vec![(x, y), (x, y + self.tape_height)],
        }
    }

    fn make_zig_zags(&self, x: f64, y: f64) -> Vec<Point> {
        let zig_zag_height = self.tape_height / self.num_zig_zags as f64;
        let mut points = Vec::new();
        for i in 0..self.num_zig_zags {
            let left = x;
            let right = x + self.join_width;
            let top = y + (i as f64 * zig_zag_height);
            let middle = y + ((i * 2 + 1) as f64 * zig_zag_height / 2.0);
            points.push((left, top));
            points.push((right, middle));
        }
        points.push((x, y + self.tape_height));
        points
    }
}
