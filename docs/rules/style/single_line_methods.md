# Style/SingleLineMethods

Avoid single-line methods.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for single-line method definitions that contain a body. It will accept single-line methods with no body.

Endless methods added in Ruby 3.0 are also accepted by this cop.

If `Style/EndlessMethod` is enabled with `EnforcedStyle: allow_single_line`, `allow_always`, `require_single_line`, or `require_always`, single-line methods will be autocorrected to endless methods if there is only one statement in the body.

```ruby
# bad
def some_method; body end
def link_to(url); {:name => url}; end
def @table.columns; super; end

# good
def self.resource_class=(klass); end
def @table.columns; end
def some_method() = body
```

With `AllowIfMethodIsEmpty: false`, a completely empty single-line method (`def no_op; end`) is also flagged; the default (`true`) accepts it.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowIfMethodIsEmpty | true |  | Whether a single-line method with no body (`def no_op; end`) is accepted. |

## Blind spots

None recorded.
