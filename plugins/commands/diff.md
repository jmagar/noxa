---
description: Compare a URL's current content against a saved snapshot
argument-hint: <snapshot.json> <url>
---

Output: !`noxa --diff-with $1 $2`

Report the diff result to the user: whether the content changed, the word delta, and a summary of what sections changed.
