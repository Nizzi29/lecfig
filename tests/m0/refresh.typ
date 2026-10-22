// M0 check: does the tinymist preview refresh when only the JSON file changes?
// Open this file in Helix, then edit refresh.json in another terminal and save.
#let data = json("refresh.json")
= JSON refresh check
Value: #data.value
