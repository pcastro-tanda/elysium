if x
  'x'
else
  if y then 'y' elsif z then 'z' else 'a' end
  ^^ Convert `if` nested inside `else` to `elsif`.
end
