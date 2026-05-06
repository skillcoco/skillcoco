---
	slug: bad
title: This frontmatter has a tab indent on the first key, which is invalid YAML.
steps:
  - id: anything
    title: This will not parse
---

# Malformed Frontmatter

The leading tab in front of `slug:` is not valid YAML indentation. A correct
gray_matter parse must surface this as Err(LabError::Spec(_)).
