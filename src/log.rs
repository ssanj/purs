use ansi_term::Colour;
use crate::model::PursError;

pub fn print_error(message: String) {
  let coloured_error = Colour::Red.paint(format!("Error: {}", message));
  println!("{}", coloured_error)
}

pub fn print_info(message: String) {
  let coloured_info = Colour::Green.paint(message);
  println!("{}", coloured_info)
}

pub fn print_errors(message: &str, errors: Vec<PursError>) {
  print_error(message.to_owned());
  errors.into_iter().for_each(|e| {
    print_error(format!("  {}", e))
  })
}
