// Copyright 2026 Andrew Moran <developer@moran.io>
// SPDX-License-Identifier: MPL-2.0

use crate::UserPlaces;
use quick_xml::writer::Writer;
use quick_xml::Error;
use std::io::Cursor;

pub fn custom_write(user_places: UserPlaces) -> Result<String, crate::Error> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    let _ = writer
        .create_element("xbel")
        .with_attributes(
            vec![
                (
                    "xmlns:mime",
                    "http://www.freedesktop.org/standards/shared-mime-info",
                ),
                (
                    "xmlns:bookmark",
                    "http://www.freedesktop.org/standards/desktop-bookmarks",
                ),
                ("version", "1.0"),
            ]
            .into_iter(),
        )
        .write_inner_content::<_, Error>(|writer| {
            for b in user_places.bookmarks {
                let mut attributes = vec![];
                let mut b_added = String::from("");
                if b.added.is_some() {
                    b_added += &b.added.unwrap();
                    attributes.push(("added", b_added.as_str()));
                }
                attributes.push(("href", b.href.as_str()));
                let mut b_modified = String::from("");
                if b.modified.is_some() {
                    b_modified += &b.modified.unwrap();
                    attributes.push(("modified", b_modified.as_str()));
                }
                let mut b_visited = String::from("");
                if b.visited.is_some() {
                    b_visited += &b.visited.unwrap();
                    attributes.push(("visited", b_visited.as_str()));
                }
                let _ = writer
                    .create_element("bookmark")
                    .with_attributes(attributes)
                    .write_inner_content::<_, Error>(|writer| {
                        if let Some(info) = b.info {
                            let _ = writer
                                .create_element("info")
                                .write_inner_content::<_, Error>(|writer| {
                                    for m in info.metadata {
                                        let _ = writer
                                            .create_element("metadata")
                                            .with_attributes([("owner", m.owner.as_str())])
                                            .write_inner_content::<_, Error>(|writer| {
                                                if let Some(mime) = m.mime_type {
                                                    let _ = writer
                                                        .create_element("mime:mime-type")
                                                        .with_attributes([(
                                                            "type",
                                                            mime.mime_type.as_str(),
                                                        )])
                                                        .write_empty();
                                                }
                                                let _ = writer
                                                    .create_element("bookmark:applications")
                                                    .write_inner_content::<_, Error>(|writer| {
                                                        let mut apps = vec![];
                                                        if m.applications.is_some() {
                                                            apps = m.applications.unwrap().applications
                                                        }
                                                        for app in apps {
                                                            let _ = writer
                                                                .create_element("bookmark:application")
                                                                .with_attributes([
                                                                    ("name", app.name.as_str()),
                                                                    ("exec", app.exec.as_str()),
                                                                    ("modified", app.modified.as_str()),
                                                                    (
                                                                        "count",
                                                                        app.count.to_string().as_str(),
                                                                    ),
                                                                ])
                                                                .write_empty();
                                                        }
                                                        Ok(())
                                                    });
                                                Ok(())
                                            });
                                    }
                                    Ok(())
                                });
                        }
                        Ok(())
                    });
            }
            Ok(())
        });

    let bytes = writer.into_inner().into_inner();
    match String::from_utf8(bytes) {
        Ok(string) => Ok(string),
        Err(_e) => Err(crate::Error::Serialization(None)),
    }
}
