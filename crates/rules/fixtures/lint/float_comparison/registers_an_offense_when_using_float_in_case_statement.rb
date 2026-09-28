case value
when 1.0
     ^^^ Avoid float literal comparisons in case statements as they are unreliable.
  foo
when 2.0
     ^^^ Avoid float literal comparisons in case statements as they are unreliable.
  bar
end
