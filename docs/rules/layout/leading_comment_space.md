# Layout/LeadingCommentSpace

Comments should start with a space.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

A `#` that starts a comment should be followed by a space, so the comment
reads as prose rather than code.

```ruby
# bad
#Some comment

# good
# Some comment
```

The leading space is not required for RDoc's `#++`/`#--` block markers, a
`#:nodoc:`-style directive (unless `AllowRBSInlineAnnotation` exempts it
first), a shebang line (or its continuation lines, or a rackup `#\` options
line in `config.ru`), or a sprockets-style `#=` directive.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowDoxygenCommentStyle | false |  | Allow a comment starting with `#*` (a Doxygen block delimiter). |
| AllowGemfileRubyComment | false |  | In a file named `Gemfile`, allow a `#ruby`-prefixed comment (RVM's version pin syntax). |
| AllowRBSInlineAnnotation | false |  | Allow a comment starting with `#:`, `#|`, or `#[...]` (an RBS::Inline annotation). |
| AllowSteepAnnotation | false |  | Allow a comment starting with `#:` or `#$` (a Steep type annotation). |
| AllowYARDCommentBlockSeparator | false |  | Allow a comment that is exactly `#-` (a YARD comment block separator). |

## Blind spots

A comment's text is read from the file as-is; a `#`-run followed only by a
non-ASCII space character is not recognised as needing a space, matching
RuboCop's `\s`-based regular expressions.
