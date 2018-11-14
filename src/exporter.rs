use history::History;
use settings::Settings;

use csv::Writer;
use std::fs::File;
use history::Command;
use training_sample_generator::TrainingSampleGenerator;
use std::cell::RefCell;

#[derive(Debug)]
pub struct Exporter<'a> {
    settings: &'a Settings,
    history: &'a History,
    writer: RefCell<Writer<File>>
}

impl<'a> Exporter<'a> {
    pub fn new(settings: &'a Settings, history: &'a History) -> Exporter<'a> {
        let path = settings.file.clone().unwrap();
        let writer = RefCell::new(Writer::from_path(path)
            .expect("Expected to be able to write a CSV"));

        Exporter { settings, history, writer }
    }

    fn output_header(&self) {
        let mut writer = self.writer.borrow_mut();
        writer.write_record(&[
            "age_factor",
            "length_factor",
            "exit_factor",
            "recent_failure_factor",
            "selected_dir_factor",
            "dir_factor",
            "overlap_factor",
            "immediate_overlap_factor",
            "selected_occurrences_factor",
            "occurrences_factor",
            "correct"
        ]).expect("Expected to write to CSV");
        writer.flush().expect("Expected to flush CSV");
    }

    fn output_row(&self, winner: &Command, correct: bool) {
        let mut writer = self.writer.borrow_mut();
        writer.write_record(&[
            format!("{}", winner.age_factor),
            format!("{}", winner.length_factor),
            format!("{}", winner.exit_factor),
            format!("{}", winner.recent_failure_factor),
            format!("{}", winner.selected_dir_factor),
            format!("{}", winner.dir_factor),
            format!("{}", winner.overlap_factor),
            format!("{}", winner.immediate_overlap_factor),
            format!("{}", winner.selected_occurrences_factor),
            format!("{}", winner.occurrences_factor),
            if correct { String::from("1.0") } else { String::from("0.0") }
        ]).expect("Expected to write to CSV");
        writer.flush().expect("Expected to flush CSV");
    }

    pub fn export(&self) {
        self.output_header();

        let generator = TrainingSampleGenerator::new(self.settings, self.history);
        generator.generate(-1, |command: &Command, correct: bool| {
            self.output_row(command, correct);
        });
    }
}
