x = case something
in :a
  bar
  ^^^ Move `bar` out of the conditional.
in :b
  bar
  ^^^ Move `bar` out of the conditional.
else
  bar
  ^^^ Move `bar` out of the conditional.
end
