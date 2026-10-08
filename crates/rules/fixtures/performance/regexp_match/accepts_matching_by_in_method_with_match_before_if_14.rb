def foo
  return $MATCH if re !~ "foo"
end
