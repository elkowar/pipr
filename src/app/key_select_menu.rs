pub struct KeySelectMenu<T> {
    options: Vec<(char, String)>,
    pub menu_type: T,
}

impl<T> KeySelectMenu<T> {
    pub fn new(options: Vec<(char, String)>, menu_type: T) -> Self {
        Self { options, menu_type }
    }

    pub fn option_list_strings(&self) -> impl Iterator<Item = String> + '_ {
        self.options.iter().map(|(c, s)| format!("{}: {}", c, s))
    }
}
