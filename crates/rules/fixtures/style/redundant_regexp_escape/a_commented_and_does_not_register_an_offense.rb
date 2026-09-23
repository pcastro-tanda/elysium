r = /
  foo # shouldn't start a char class: [
  \+  # escape is only redundant inside a char class, so not redundant here
  bar # shouldn't end a char class: ]
/x
