extern crate rimd;

use rimd::{Event, SMF, SMFError, Status};
use std::env::{args, Args};
use std::path::Path;

fn main() {
    let mut args: Args = args();
    args.next();
    let pathstr = match args.next() {
        Some(s) => s,
        None => {
            panic!("Need a source path");
        }
    };
    match SMF::from_file(&Path::new(&pathstr[..])) {
        Ok(smf) => {
            make_svg(smf);
        }
        Err(e) => {
            match e {
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
            }
        }
    }
}

fn make_svg(smf: SMF) {
    let mut time = 0;
    let mut notes = Vec::new();
    let mut max = 0;
    let div = smf.division;
    if div <= 0 {
        panic!("Unsupported div {}", div);
    }

    // TODO: Don't only use track 2
    for event in &smf.tracks[2].events {
        time += event.vtime;
        match event.event {
            Event::Midi(ref msg) => {
                if msg.status() == Status::NoteOn {
                    if time > max {
                        max = time;
                    }
                    notes.push((time, 128 - msg.data(1)));
                }
            }
            Event::Meta(_) => {}
        };
    }
    println!(r#"<?xml version="1.0" encoding="UTF-8" ?>
<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{}.5" height="128">"#,
             (max as f64 * 16. / div as f64) as u64);
    for (time, note) in notes {
        println!(r#"<circle cx="{}.5" cy="{}" r="0.5" fill="black"/>"#,
                 (time as f64 * 16. / div as f64) as u64,
                 128 - note);
    }
    println!("</svg>");
}
