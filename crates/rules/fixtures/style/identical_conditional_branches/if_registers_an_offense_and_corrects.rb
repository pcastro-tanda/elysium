x = if foo
  bar
  ^^^ Move `bar` out of the conditional.
else
  bar
  ^^^ Move `bar` out of the conditional.
end
