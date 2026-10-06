def foo
  return $MATCH unless "foo".match(re, 1)
end
