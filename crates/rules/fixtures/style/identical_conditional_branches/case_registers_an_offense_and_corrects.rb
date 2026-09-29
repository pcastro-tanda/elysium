x = case something
when :a
  bar
  ^^^ Move `bar` out of the conditional.
when :b
  bar
  ^^^ Move `bar` out of the conditional.
else
  bar
  ^^^ Move `bar` out of the conditional.
end
