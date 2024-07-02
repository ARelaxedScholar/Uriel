/*I need a way to annotate files, epub files for my app.
This means I want to be able to take notes, to highlight, and extract snippets.
As I see it, the take notes is a side feature, so no need to focus on it.
The extract snippet feature is important for the card creation and would likely be a matter of copying bits of data.
the highlight might be something I build on top of the extract?
  */

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
    color: Color,
    type: HighlightType,
    comment: String,    
    start_cfi:String,
    end_cfi: String,
}

// Opting for  an external note approach.
