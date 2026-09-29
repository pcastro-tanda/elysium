if x
  'x'
else
  if y then 'y' else 'z' end
  ^^ Convert `if` nested inside `else` to `elsif`.
end
