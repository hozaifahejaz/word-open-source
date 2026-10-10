//! Original, supported paragraph/run templates; catalog IDs are stable API values.
use document_core::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateId {
    Blank,
    Letter,
    MeetingNotes,
    ProjectBrief,
}
pub const CATALOG: &[(TemplateId, &str, &str, &str)] = &[
    (
        TemplateId::Blank,
        "blank",
        "Blank",
        "A clean page for your own words.",
    ),
    (
        TemplateId::Letter,
        "letter",
        "Letter",
        "A thoughtful letter with a clear opening and close.",
    ),
    (
        TemplateId::MeetingNotes,
        "meeting_notes",
        "Meeting notes",
        "Capture decisions, owners and next steps.",
    ),
    (
        TemplateId::ProjectBrief,
        "project_brief",
        "Project brief",
        "Define the purpose, scope and measures of success.",
    ),
];
pub fn build(id: TemplateId) -> Result<Document, String> {
    if id == TemplateId::Blank {
        return Ok(Document::default());
    }
    let lines: &[(&str, bool)] = match id {
        TemplateId::Blank => unreachable!(),
        TemplateId::Letter => &[
            ("A personal letter", true),
            ("[Your name] · [Your address]", false),
            ("[Date]", false),
            ("Dear [recipient],", false),
            (
                "I am writing to [describe the purpose of your letter].",
                false,
            ),
            (
                "[Share the context, explain your request and suggest a next step.]",
                false,
            ),
            ("Thank you for your time and consideration.", false),
            ("Warm regards,", false),
            ("[Your name]", false),
        ],
        TemplateId::MeetingNotes => &[
            ("Meeting notes", true),
            ("[Meeting name] · [Date] · [Participants]", false),
            ("Purpose", true),
            ("[What do we need to decide today?]", false),
            ("Discussion", true),
            (
                "[Record the useful context and differing perspectives.]",
                false,
            ),
            ("Decisions", true),
            ("[Decision — reasoning — owner]", false),
            ("Next steps", true),
            ("[Action — owner — due date]", false),
        ],
        TemplateId::ProjectBrief => &[
            ("Project brief", true),
            ("[Project name] · [Owner] · [Date]", false),
            ("Purpose and audience", true),
            ("[What problem will this solve, and for whom?]", false),
            ("Scope", true),
            ("[Describe the deliverables and boundaries.]", false),
            ("Success measures", true),
            ("[How will we know the outcome helped?]", false),
            ("Milestones and risks", true),
            ("[Milestone — date — dependency or risk]", false),
            ("Next decision", true),
            ("[What needs agreement before work begins?]", false),
        ],
    };
    let doc = Document {
        blocks: lines
            .iter()
            .enumerate()
            .map(|(index, &(text, bold))| {
                let mut paragraph = Paragraph::plain(text);
                paragraph.runs[0].style.bold = bold;
                if index == 0 {
                    paragraph.runs[0].style.size_half_points = 40;
                }
                Block::Paragraph(paragraph)
            })
            .collect(),
        ..Default::default()
    };
    doc.validate().map_err(|e| e.to_string())?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_builders_are_valid_original_formatted_documents() {
        assert_eq!(
            CATALOG.iter().map(|entry| entry.1).collect::<Vec<_>>(),
            ["blank", "letter", "meeting_notes", "project_brief"]
        );
        for &(id, _, _, _) in CATALOG {
            let doc = build(id).unwrap();
            doc.validate().unwrap();
            if id == TemplateId::Blank {
                assert_eq!(doc, Document::default());
            } else {
                assert!(doc.blocks.len() >= 5);
                assert!(doc.paragraph(0).unwrap().runs[0].style.bold);
            }
        }
    }
}
