# Layout/SpaceAroundKeyword

Use a space around keywords if appropriate.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the spacing around the keywords.

# bad
something 'test'do|x|
end

while(something)
end

something = 123if test

return(foo + bar)

# good
something 'test' do |x|
end

while (something)
end

something = 123 if test

return (foo + bar)

## Options

This rule has no options.

## Blind spots

None recorded.
