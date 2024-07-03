/*I need a way to annotate files, epub files for my app.
This means I want to be able to take notes, to highlight, and extract snippets.
As I see it, the take notes is a side feature, so no need to focus on it.
The extract snippet feature is important for the card creation and would likely be a matter of copying bits of data.
the highlight might be something I build on top of the extract?
  */
use crate::mod.rs::EpubDoc;

enum HighlightType{
    Actionable,
    Example,
    Important,
    Interesting,
    KeyConcept,
    Reread,
    Question,    
}

struct Highlight {
    highlight_type: HighlightType,
    comment: String,    
    start_cfi:String,
    end_cfi: String,
}

impl Highlight{
    //TODO: Define the functions to load the highlights from json, save them to Json, and add an highlight and remove one.
}

fn highlight(highlight_type:HighlightType, text_to_highlight:&str, comment:&str, start_cfi:&str, end_cfi:&str) -> Highlight{
    Highlight(
        highlight_type,
        comment: comment.to_string(),
        start_cfi: start_cfi.to_string(),
        end_cfi: end_cfi.to_string()
    )    
}

fn highlight_epub(){
    //TODO: Define this
}



