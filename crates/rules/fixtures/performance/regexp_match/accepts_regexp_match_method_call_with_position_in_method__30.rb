def foo
  return $MATCH if /re/.match(foo, 1)
end
