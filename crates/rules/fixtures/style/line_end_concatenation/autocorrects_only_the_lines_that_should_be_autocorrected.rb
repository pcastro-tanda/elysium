top = "test#{x}" <<
                 ^^ Use `\` instead of `<<` to concatenate multiline strings.
"top" + # comment
"foo" +
      ^ Use `\` instead of `+` to concatenate multiline strings.
"bar" +
%(baz) +
"qux"
