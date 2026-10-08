def foo
  return $MATCH unless /re/.match(foo, 1)
end
