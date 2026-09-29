# Style/TrailingBodyOnMethodDefinition

Checks for trailing code after the method definition.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

NOTE: It always accepts endless method definitions that are basically on the same line.

# bad
def some_method; do_stuff
end

def f(x); b = foo
b[c: x]
end

# good
def some_method
do_stuff
end

def f(x)
b = foo
b[c: x]
end

def endless_method = do_stuff

## Options

This rule has no options.

## Blind spots

`Layout/IndentationWidth`'s `Width` is read as a peer option, matching upstream's own
`Alignment#configured_indentation_width` cross-cop read.
