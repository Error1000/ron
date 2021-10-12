
trait Terminal{
    fn write(&mut self, s: &str);
    fn read(&self) -> char;
}

struct Shell<DisplayType: Terminal>{
    display: DisplayType,
}

impl<DisplayType: Terminal> Shell<DisplayType>{
    fn display_prompt(&mut self){
            self.display.write("#");
    }

    fn run_line(&mut self){
//        self.display.read();
    }
}